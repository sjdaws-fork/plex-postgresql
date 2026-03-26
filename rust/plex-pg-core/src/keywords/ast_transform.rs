use super::*;

pub(super) fn transform_query(q: &mut Query) {
    if let Some(with) = &mut q.with {
        for cte in &mut with.cte_tables {
            transform_query(&mut cte.query);
        }
    }

    // Detect GROUP BY NULL before transforming (need Query-level access for ORDER BY)
    let had_group_by_null = if let SetExpr::Select(sel) = q.body.as_ref() {
        if let GroupByExpr::Expressions(exprs, _) = &sel.group_by {
            !exprs.is_empty() && exprs.iter().all(is_null_expr)
        } else {
            false
        }
    } else {
        false
    };

    transform_set_expr(&mut q.body);

    // GROUP BY NULL was removed: add ORDER BY 1 NULLS FIRST if no ORDER BY exists
    if had_group_by_null && q.order_by.is_none() {
        q.order_by = Some(OrderBy {
            kind: OrderByKind::Expressions(vec![OrderByExpr {
                expr: Expr::Value(ValueWithSpan {
                    value: Value::Number("1".to_string(), false),
                    span: Span::empty(),
                }),
                options: OrderByOptions {
                    asc: None,
                    nulls_first: Some(true),
                },
                with_fill: None,
            }]),
            interpolate: None,
        });
    }
}

fn transform_set_expr(se: &mut SetExpr) {
    match se {
        SetExpr::Select(s) => transform_select(s),
        SetExpr::Query(q) => transform_query(q),
        SetExpr::SetOperation { left, right, .. } => {
            transform_set_expr(left);
            transform_set_expr(right);
        }
        _ => {}
    }
}

fn transform_select(sel: &mut Select) {
    // Rewrite FROM tables (sqlite_master → information_schema subquery)
    for twj in &mut sel.from {
        transform_table_with_joins(twj);
    }

    // WHERE
    if let Some(w) = &mut sel.selection {
        transform_expr(w);
    }

    // GROUP BY NULL → remove
    if let GroupByExpr::Expressions(exprs, _) = &mut sel.group_by {
        exprs.retain(|e| !is_null_expr(e));
        // If all expressions were NULL, replace with empty group-by
    }

    // Projection subqueries
    for item in &mut sel.projection {
        match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => {
                transform_expr(e);
            }
            _ => {}
        }
    }

    // HAVING
    if let Some(h) = &mut sel.having {
        transform_expr(h);
    }
}

fn is_null_expr(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Value(ValueWithSpan {
            value: Value::Null,
            ..
        })
    )
}

fn transform_table_with_joins(twj: &mut TableWithJoins) {
    transform_table_factor(&mut twj.relation);
    for join in &mut twj.joins {
        transform_table_factor(&mut join.relation);
    }
}

fn transform_table_factor(tf: &mut TableFactor) {
    match tf {
        TableFactor::Table { name, .. } => {
            // Check the last part of the name (handles "main".sqlite_master etc.)
            let bare_name = table_name_last_part(name);
            if bare_name == "sqlite_master" || bare_name == "sqlite_schema" {
                *tf = make_sqlite_master_subquery();
            }
        }
        TableFactor::Derived { subquery, .. } => {
            transform_query(subquery);
        }
        _ => {}
    }
}

/// Get the last (bare) part of a table name, e.g. "main".sqlite_master → sqlite_master
fn table_name_last_part(name: &ObjectName) -> String {
    name.0
        .last()
        .and_then(|p| match p {
            ObjectNamePart::Identifier(i) => Some(i.value.to_lowercase()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Build the subquery that replaces `sqlite_master`:
///
/// ```sql
/// (SELECT tablename AS name, 'table' AS type
///  FROM information_schema.tables WHERE table_schema = 'public'
///  UNION
///  SELECT indexname AS name, 'index' AS type
///  FROM pg_indexes WHERE schemaname = 'public') AS sqlite_master
/// ```
fn make_sqlite_master_subquery() -> TableFactor {
    let make_str = |s: &str| -> Expr {
        Expr::Value(ValueWithSpan {
            value: Value::SingleQuotedString(s.to_string()),
            span: Span::empty(),
        })
    };
    let make_ident = |s: &str| -> Expr { Expr::Identifier(Ident::new(s)) };
    let make_alias_item = |expr: Expr, alias: &str| -> SelectItem {
        SelectItem::ExprWithAlias {
            expr,
            alias: Ident::new(alias),
        }
    };

    // SELECT tablename AS name, 'table' AS type
    // FROM information_schema.tables
    // WHERE table_schema = 'public'
    let tables_select = Box::new(SetExpr::Select(Box::new(Select {
        select_token: AttachedToken::empty(),
        optimizer_hint: None,
        distinct: None,
        select_modifiers: None,
        top: None,
        top_before_distinct: false,
        projection: vec![
            make_alias_item(make_ident("tablename"), "name"),
            make_alias_item(make_str("table"), "type"),
        ],
        exclude: None,
        into: None,
        from: vec![TableWithJoins {
            relation: TableFactor::Table {
                name: ObjectName(vec![
                    ObjectNamePart::Identifier(Ident::new("information_schema")),
                    ObjectNamePart::Identifier(Ident::new("tables")),
                ]),
                alias: None,
                args: None,
                with_hints: vec![],
                version: None,
                with_ordinality: false,
                partitions: vec![],
                json_path: None,
                sample: None,
                index_hints: vec![],
            },
            joins: vec![],
        }],
        lateral_views: vec![],
        prewhere: None,
        selection: Some(Expr::BinaryOp {
            left: Box::new(make_ident("table_schema")),
            op: BinaryOperator::Eq,
            right: Box::new(make_str("public")),
        }),
        connect_by: vec![],
        group_by: GroupByExpr::Expressions(vec![], vec![]),
        cluster_by: vec![],
        distribute_by: vec![],
        sort_by: vec![],
        having: None,
        named_window: vec![],
        qualify: None,
        window_before_qualify: false,
        value_table_mode: None,
        flavor: SelectFlavor::Standard,
    })));

    // SELECT indexname AS name, 'index' AS type
    // FROM pg_indexes
    // WHERE schemaname = 'public'
    let indexes_select = Box::new(SetExpr::Select(Box::new(Select {
        select_token: AttachedToken::empty(),
        optimizer_hint: None,
        distinct: None,
        select_modifiers: None,
        top: None,
        top_before_distinct: false,
        projection: vec![
            make_alias_item(make_ident("indexname"), "name"),
            make_alias_item(make_str("index"), "type"),
        ],
        exclude: None,
        into: None,
        from: vec![TableWithJoins {
            relation: TableFactor::Table {
                name: ObjectName(vec![ObjectNamePart::Identifier(Ident::new("pg_indexes"))]),
                alias: None,
                args: None,
                with_hints: vec![],
                version: None,
                with_ordinality: false,
                partitions: vec![],
                json_path: None,
                sample: None,
                index_hints: vec![],
            },
            joins: vec![],
        }],
        lateral_views: vec![],
        prewhere: None,
        selection: Some(Expr::BinaryOp {
            left: Box::new(make_ident("schemaname")),
            op: BinaryOperator::Eq,
            right: Box::new(make_str("public")),
        }),
        connect_by: vec![],
        group_by: GroupByExpr::Expressions(vec![], vec![]),
        cluster_by: vec![],
        distribute_by: vec![],
        sort_by: vec![],
        having: None,
        named_window: vec![],
        qualify: None,
        window_before_qualify: false,
        value_table_mode: None,
        flavor: SelectFlavor::Standard,
    })));

    let union_query = Box::new(Query {
        with: None,
        body: Box::new(SetExpr::SetOperation {
            op: SetOperator::Union,
            set_quantifier: SetQuantifier::None,
            left: tables_select,
            right: indexes_select,
        }),
        order_by: None,
        limit_clause: None,
        fetch: None,
        locks: vec![],
        for_clause: None,
        settings: None,
        format_clause: None,
        pipe_operators: vec![],
    });

    TableFactor::Derived {
        lateral: false,
        subquery: union_query,
        alias: Some(TableAlias {
            explicit: true,
            name: Ident::new("pg_schema_objects"),
            columns: vec![],
        }),
        sample: None,
    }
}

/// Recursively transform an expression in place, rewriting SQLite keyword
/// constructs to their PostgreSQL equivalents.
pub(super) fn transform_expr(expr: &mut Expr) {
    match expr {
        // IN () → IN (SELECT -1 WHERE FALSE)
        Expr::InList { ref list, .. } if list.is_empty() => {
            // Build: SELECT -1 WHERE FALSE
            let false_select = Box::new(Query {
                with: None,
                body: Box::new(SetExpr::Select(Box::new(Select {
                    select_token: AttachedToken::empty(),
                    optimizer_hint: None,
                    distinct: None,
                    select_modifiers: None,
                    top: None,
                    top_before_distinct: false,
                    projection: vec![SelectItem::UnnamedExpr(Expr::UnaryOp {
                        op: UnaryOperator::Minus,
                        expr: Box::new(Expr::Value(ValueWithSpan {
                            value: Value::Number("1".to_string(), false),
                            span: Span::empty(),
                        })),
                    })],
                    exclude: None,
                    into: None,
                    from: vec![],
                    lateral_views: vec![],
                    prewhere: None,
                    selection: Some(Expr::Value(ValueWithSpan {
                        value: Value::Boolean(false),
                        span: Span::empty(),
                    })),
                    connect_by: vec![],
                    group_by: GroupByExpr::Expressions(vec![], vec![]),
                    cluster_by: vec![],
                    distribute_by: vec![],
                    sort_by: vec![],
                    having: None,
                    named_window: vec![],
                    qualify: None,
                    window_before_qualify: false,
                    value_table_mode: None,
                    flavor: SelectFlavor::Standard,
                }))),
                order_by: None,
                limit_clause: None,
                fetch: None,
                locks: vec![],
                for_clause: None,
                settings: None,
                format_clause: None,
                pipe_operators: vec![],
            });
            // Extract the inner expr, then replace the whole node
            let inner = match std::mem::replace(
                expr,
                Expr::Value(ValueWithSpan {
                    value: Value::Null,
                    span: Span::empty(),
                }),
            ) {
                Expr::InList {
                    expr: inner,
                    negated,
                    ..
                } => (inner, negated),
                _ => unreachable!(),
            };
            *expr = Expr::InSubquery {
                expr: inner.0,
                subquery: false_select,
                negated: inner.1,
            };
        }
        // Recurse
        Expr::BinaryOp { left, right, .. } => {
            transform_expr(left);
            transform_expr(right);
        }
        Expr::UnaryOp { expr: inner, .. } => transform_expr(inner),
        Expr::Nested(inner) => transform_expr(inner),
        Expr::InList {
            expr: inner, list, ..
        } => {
            transform_expr(inner);
            for e in list {
                transform_expr(e);
            }
        }
        Expr::InSubquery { expr: inner, .. } => {
            transform_expr(inner);
        }
        Expr::Between {
            expr: inner,
            low,
            high,
            ..
        } => {
            transform_expr(inner);
            transform_expr(low);
            transform_expr(high);
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(op) = operand {
                transform_expr(op);
            }
            for cw in conditions {
                transform_expr(&mut cw.condition);
                transform_expr(&mut cw.result);
            }
            if let Some(er) = else_result {
                transform_expr(er);
            }
        }
        _ => {}
    }
}
