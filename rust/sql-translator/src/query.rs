/// Module: query
///
/// Miscellaneous query fixups for SQLite → PostgreSQL translation:
///   1. Subquery alias      — FROM (SELECT …) without alias → AS _subq
///   2. NULLS LAST          — `ORDER BY col IS NULL, col ASC` → `ORDER BY col NULLS LAST`
///   3. DISTINCT + agg ORDER BY — `SELECT DISTINCT … ORDER BY count(*)` → remove DISTINCT
///   4. CASE booleans       — `CASE WHEN x THEN 1 ELSE 0 END` → TRUE/FALSE
///   5. WHERE 1 / WHERE 0   — → WHERE TRUE / WHERE FALSE
///   6. max(a,b) multi-arg  — → GREATEST(a,b)
///   7. min(a,b) multi-arg  — → LEAST(a,b)
///   8. COLLATE ICU strip   — remove icu_* collations
///   9. COLLATE NOCASE      — `x COLLATE NOCASE = y` → `LOWER(x) = LOWER(y)`
use sqlparser::ast::*;
use sqlparser::tokenizer::Span;

pub fn transform(stmt: &mut Statement) {
    match stmt {
        Statement::Query(q) => transform_query(q),
        Statement::Insert(ins) => {
            if let Some(src) = &mut ins.source {
                transform_query(src);
            }
        }
        Statement::Update(u) => {
            if let Some(sel) = &mut u.selection {
                transform_expr_inplace(sel);
            }
        }
        Statement::Delete(d) => {
            if let Some(sel) = &mut d.selection {
                transform_expr_inplace(sel);
            }
        }
        _ => {}
    }
}

fn transform_query(q: &mut Query) {
    if let Some(with) = &mut q.with {
        for cte in &mut with.cte_tables {
            transform_query(&mut cte.query);
        }
    }

    // Check for DISTINCT + aggregate in ORDER BY → remove DISTINCT
    fix_distinct_with_agg_orderby(q);

    transform_set_expr(&mut q.body);

    // Transform expressions in ORDER BY (collate stripping, etc.)
    if let Some(ob) = &mut q.order_by {
        if let OrderByKind::Expressions(exprs) = &mut ob.kind {
            for oe in exprs.iter_mut() {
                transform_expr_inplace(&mut oe.expr);
            }
        }
    }

    // Fix ORDER BY IS NULL pattern → NULLS LAST (must run after expr transform)
    fix_order_by_is_null(q);
}

fn transform_set_expr(se: &mut SetExpr) {
    match se {
        SetExpr::Select(s) => transform_select(s),
        SetExpr::Query(q) => transform_query(q),
        SetExpr::SetOperation { left, right, .. } => {
            transform_set_expr(left);
            transform_set_expr(right);
        }
        SetExpr::Values(vals) => {
            for row in &mut vals.rows {
                for e in row {
                    transform_expr_inplace(e);
                }
            }
        }
        _ => {}
    }
}

fn transform_select(sel: &mut Select) {
    // Fix FROM subqueries without alias
    for twj in &mut sel.from {
        fix_table_with_joins(twj);
    }

    // WHERE fixups
    if let Some(w) = &mut sel.selection {
        fix_where_numeric(w);
        transform_expr_inplace(w);
    }

    // Projection
    for item in &mut sel.projection {
        match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => {
                transform_expr_inplace(e);
            }
            _ => {}
        }
    }

    // GROUP BY
    if let GroupByExpr::Expressions(ref mut exprs, _) = sel.group_by {
        for e in exprs {
            transform_expr_inplace(e);
        }
    }

    // HAVING
    if let Some(h) = &mut sel.having {
        transform_expr_inplace(h);
    }
}

// ─── Fix 1: Subquery alias ────────────────────────────────────────────────────

fn fix_table_with_joins(twj: &mut TableWithJoins) {
    fix_table_factor(&mut twj.relation);
    for join in &mut twj.joins {
        fix_table_factor(&mut join.relation);
    }
}

fn fix_table_factor(tf: &mut TableFactor) {
    if let TableFactor::Derived {
        subquery, alias, ..
    } = tf
    {
        transform_query(subquery);
        // Add alias if missing
        if alias.is_none() {
            *alias = Some(TableAlias {
                explicit: true,
                name: Ident::new("_subq"),
                columns: vec![],
            });
        }
    }
}

// ─── Fix 2: NULLS LAST from IS NULL pattern ───────────────────────────────────

/// Detect: `ORDER BY expr IS NULL, expr [ASC|DESC]`
/// Replace with: `ORDER BY expr [ASC|DESC] NULLS LAST`
fn fix_order_by_is_null(q: &mut Query) {
    let Some(ob) = &mut q.order_by else { return };
    let OrderByKind::Expressions(exprs) = &mut ob.kind else {
        return;
    };

    // Find pairs: (i, j) where exprs[i] is `x IS NULL` and exprs[j] is `x [ASC|DESC]`
    // Strategy: collect indices of IS NULL exprs, match with following expr
    let len = exprs.len();
    if len < 2 {
        return;
    }

    let mut to_remove: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < len {
        if let Expr::IsNull(ref inner) = exprs[i].expr {
            let col = *inner.clone();
            // Look for the next expr that matches `col`
            if i + 1 < len {
                let next = &mut exprs[i + 1];
                if exprs_display_eq(&next.expr, &col) {
                    // Set NULLS LAST on the real ordering expr
                    next.options.nulls_first = Some(false);
                    to_remove.push(i);
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }

    // Remove IS NULL entries in reverse order
    for idx in to_remove.into_iter().rev() {
        exprs.remove(idx);
    }
}

fn exprs_display_eq(a: &Expr, b: &Expr) -> bool {
    format!("{}", a).to_lowercase() == format!("{}", b).to_lowercase()
}

// ─── Fix 3: DISTINCT + aggregate ORDER BY ─────────────────────────────────────

fn fix_distinct_with_agg_orderby(q: &mut Query) {
    let has_distinct = if let SetExpr::Select(sel) = q.body.as_ref() {
        matches!(sel.distinct, Some(Distinct::Distinct))
    } else {
        false
    };

    if !has_distinct {
        return;
    }

    let has_agg_in_order = if let Some(ob) = &q.order_by {
        if let OrderByKind::Expressions(exprs) = &ob.kind {
            exprs.iter().any(|oe| is_aggregate_expr(&oe.expr))
        } else {
            false
        }
    } else {
        false
    };

    if has_agg_in_order {
        if let SetExpr::Select(sel) = q.body.as_mut() {
            sel.distinct = None;
        }
    }
}

// ─── Fix 5: WHERE 1 / WHERE 0 ─────────────────────────────────────────────────

fn fix_where_numeric(expr: &mut Expr) {
    if let Expr::Value(ValueWithSpan {
        value: Value::Number(ref s, _),
        ref span,
    }) = *expr
    {
        if s == "1" {
            *expr = Expr::Value(ValueWithSpan {
                value: Value::Boolean(true),
                span: *span,
            });
        } else if s == "0" {
            *expr = Expr::Value(ValueWithSpan {
                value: Value::Boolean(false),
                span: *span,
            });
        }
    }
}

// ─── Main expression transformer ─────────────────────────────────────────────

fn transform_expr(expr: Expr) -> Expr {
    match expr {
        // Fix 4: CASE WHEN x THEN 1 ELSE 0 END → TRUE/FALSE
        Expr::Case {
            case_token,
            end_token,
            operand,
            conditions,
            else_result,
        } => {
            let conditions: Vec<CaseWhen> = conditions
                .into_iter()
                .map(|cw| CaseWhen {
                    condition: transform_expr(cw.condition),
                    result: maybe_bool_value(transform_expr(cw.result)),
                })
                .collect();
            let else_result = else_result.map(|e| Box::new(maybe_bool_value(transform_expr(*e))));
            Expr::Case {
                case_token,
                end_token,
                operand: operand.map(|o| Box::new(transform_expr(*o))),
                conditions,
                else_result,
            }
        }

        // Fix 6 & 7: max(a,b) → GREATEST, min(a,b) → LEAST
        // Fix 8 & 9: COLLATE handling
        Expr::Function(mut func) => {
            // Get name and arg count before mutable borrow
            let fn_name = func_name_str(&func);
            let arg_count = if let FunctionArguments::List(ref al) = func.args {
                al.args.len()
            } else {
                0
            };

            // Recurse into args
            if let FunctionArguments::List(ref mut al) = func.args {
                for arg in &mut al.args {
                    match arg {
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => {
                            transform_expr_inplace(e);
                        }
                        FunctionArg::Named {
                            arg: FunctionArgExpr::Expr(e),
                            ..
                        } => {
                            transform_expr_inplace(e);
                        }
                        _ => {}
                    }
                }
            }

            if fn_name == "max" && arg_count >= 2 {
                rename_function(&mut func, "GREATEST");
            } else if fn_name == "min" && arg_count >= 2 {
                rename_function(&mut func, "LEAST");
            }

            Expr::Function(func)
        }

        // Fix 9: COLLATE NOCASE on equality → LOWER(x) = LOWER(y)
        // Fix general BinaryOp recursion
        Expr::BinaryOp { left, op, right } => {
            // Before recursing, check for COLLATE NOCASE at this level
            let is_eq_op = matches!(op, BinaryOperator::Eq | BinaryOperator::NotEq);
            let (left_stripped, left_nocase) = strip_collate_nocase_shallow(*left);
            let (right_stripped, right_nocase) = strip_collate_nocase_shallow(*right);

            if is_eq_op && (left_nocase || right_nocase) {
                // Recurse into the stripped inner expressions
                let left_t = transform_expr(left_stripped);
                let right_t = transform_expr(right_stripped);
                Expr::BinaryOp {
                    left: Box::new(wrap_lower(left_t)),
                    op,
                    right: Box::new(wrap_lower(right_t)),
                }
            } else {
                // Put back what we stripped (if nothing was NOCASE, strip_collate returns original)
                let left_t = transform_expr(left_stripped);
                let right_t = transform_expr(right_stripped);
                Expr::BinaryOp {
                    left: Box::new(left_t),
                    op,
                    right: Box::new(right_t),
                }
            }
        }
        Expr::UnaryOp { op, expr } => Expr::UnaryOp {
            op,
            expr: Box::new(transform_expr(*expr)),
        },
        Expr::Nested(inner) => Expr::Nested(Box::new(transform_expr(*inner))),
        Expr::Cast {
            kind,
            expr,
            data_type,
            array,
            format,
        } => Expr::Cast {
            kind,
            expr: Box::new(transform_expr(*expr)),
            data_type,
            array,
            format,
        },
        Expr::InList {
            expr,
            list,
            negated,
        } => Expr::InList {
            expr: Box::new(transform_expr(*expr)),
            list: list.into_iter().map(transform_expr).collect(),
            negated,
        },
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => Expr::Between {
            expr: Box::new(transform_expr(*expr)),
            negated,
            low: Box::new(transform_expr(*low)),
            high: Box::new(transform_expr(*high)),
        },
        Expr::Collate { expr, collation } => {
            // Fix 8: strip ICU collations entirely
            // Note: NOCASE on equality is handled in the BinaryOp arm above
            let collation_name = collation
                .0
                .first()
                .and_then(|p| match p {
                    ObjectNamePart::Identifier(i) => Some(i.value.to_lowercase()),
                    _ => None,
                })
                .unwrap_or_default();

            let inner = transform_expr(*expr);

            if collation_name.starts_with("icu") || collation_name == "unicode" {
                // Strip the collation entirely
                inner
            } else {
                // Keep other collations (including NOCASE when not inside equality)
                Expr::Collate {
                    expr: Box::new(inner),
                    collation,
                }
            }
        }
        Expr::IsNull(inner) => Expr::IsNull(Box::new(transform_expr(*inner))),
        Expr::IsNotNull(inner) => Expr::IsNotNull(Box::new(transform_expr(*inner))),
        other => other,
    }
}

fn transform_expr_inplace(expr: &mut Expr) {
    let taken = std::mem::replace(
        expr,
        Expr::Value(ValueWithSpan {
            value: Value::Null,
            span: Span::empty(),
        }),
    );
    *expr = transform_expr(taken);
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn func_name_str(func: &Function) -> String {
    func.name
        .0
        .first()
        .and_then(|p| match p {
            ObjectNamePart::Identifier(i) => Some(i.value.to_lowercase()),
            _ => None,
        })
        .unwrap_or_default()
}

fn rename_function(func: &mut Function, new_name: &str) {
    func.name = ObjectName(vec![ObjectNamePart::Identifier(Ident::new(new_name))]);
}

fn is_aggregate_expr(expr: &Expr) -> bool {
    const AGGREGATES: &[&str] = &[
        "count",
        "sum",
        "avg",
        "min",
        "max",
        "group_concat",
        "string_agg",
        "array_agg",
        "bool_and",
        "bool_or",
    ];
    if let Expr::Function(func) = expr {
        let name = func_name_str(func);
        AGGREGATES.contains(&name.as_str())
    } else {
        false
    }
}

/// If the expression is a numeric literal 1 or 0, replace with TRUE/FALSE.
fn maybe_bool_value(expr: Expr) -> Expr {
    if let Expr::Value(ValueWithSpan {
        value: Value::Number(ref s, _),
        ref span,
    }) = expr
    {
        if s == "1" {
            return Expr::Value(ValueWithSpan {
                value: Value::Boolean(true),
                span: *span,
            });
        } else if s == "0" {
            return Expr::Value(ValueWithSpan {
                value: Value::Boolean(false),
                span: *span,
            });
        }
    }
    expr
}

/// Strip COLLATE NOCASE from an expression (shallow — does not recurse).
/// Returns (inner_expr, was_nocase).
fn strip_collate_nocase_shallow(expr: Expr) -> (Expr, bool) {
    if let Expr::Collate {
        expr: inner,
        collation,
    } = expr
    {
        let collation_name = collation
            .0
            .first()
            .and_then(|p| match p {
                ObjectNamePart::Identifier(i) => Some(i.value.to_lowercase()),
                _ => None,
            })
            .unwrap_or_default();

        if collation_name == "nocase" {
            return (*inner, true);
        } else if collation_name.starts_with("icu") || collation_name == "unicode" {
            // Strip ICU collation, not NOCASE
            return (*inner, false);
        }
        return (
            Expr::Collate {
                expr: inner,
                collation,
            },
            false,
        );
    }
    (expr, false)
}

/// Wrap an expression in LOWER(…)
fn wrap_lower(expr: Expr) -> Expr {
    Expr::Function(Function {
        name: ObjectName(vec![ObjectNamePart::Identifier(Ident::new("LOWER"))]),
        uses_odbc_syntax: false,
        parameters: FunctionArguments::None,
        args: FunctionArguments::List(FunctionArgumentList {
            duplicate_treatment: None,
            args: vec![FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))],
            clauses: vec![],
        }),
        filter: None,
        null_treatment: None,
        over: None,
        within_group: vec![],
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::translate;

    #[test]
    fn query_subquery_gets_alias() {
        let r = translate("SELECT * FROM (SELECT id FROM t) WHERE id > 0").unwrap();
        let sql_up = r.sql.to_uppercase();
        assert!(
            sql_up.contains("AS ") || sql_up.contains(" AS"),
            "Subquery should have alias, got: {}",
            r.sql
        );
    }

    #[test]
    fn query_nulls_last_from_is_null_pattern() {
        let r =
            translate(r#"SELECT * FROM t ORDER BY parents."index" IS NULL, parents."index" asc"#)
                .unwrap();
        let sql = r.sql.to_uppercase();
        assert!(
            sql.contains("NULLS LAST"),
            "Expected NULLS LAST, got: {}",
            r.sql
        );
        assert!(
            !sql.contains("IS NULL"),
            "IS NULL pattern should be replaced, got: {}",
            r.sql
        );
    }

    #[test]
    fn query_case_then_1_else_0_to_booleans() {
        let r = translate("SELECT (CASE WHEN a THEN 1 ELSE 0 END) FROM t").unwrap();
        let sql = r.sql.to_lowercase();
        assert!(
            sql.contains("true") || sql.contains("false"),
            "Expected boolean values, got: {}",
            r.sql
        );
    }

    #[test]
    fn query_where_0_to_false() {
        let r = translate("SELECT * FROM t WHERE 0").unwrap();
        assert!(r.sql.to_uppercase().contains("FALSE"), "Got: {}", r.sql);
    }

    #[test]
    fn query_where_1_to_true() {
        let r = translate("SELECT * FROM t WHERE 1").unwrap();
        assert!(r.sql.to_uppercase().contains("TRUE"), "Got: {}", r.sql);
    }

    #[test]
    fn query_max_two_args_to_greatest() {
        let r = translate("SELECT max(x, y) FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("GREATEST"), "Got: {}", r.sql);
    }

    #[test]
    fn query_max_one_arg_preserved() {
        let r = translate("SELECT max(x) FROM t").unwrap();
        assert!(!r.sql.to_uppercase().contains("GREATEST"), "Got: {}", r.sql);
        assert!(r.sql.to_lowercase().contains("max("), "Got: {}", r.sql);
    }

    #[test]
    fn query_min_two_args_to_least() {
        let r = translate("SELECT min(x, y) FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("LEAST"), "Got: {}", r.sql);
    }

    #[test]
    fn query_strip_icu_collation() {
        let r = translate("SELECT * FROM t ORDER BY name COLLATE icu_root").unwrap();
        assert!(!r.sql.to_lowercase().contains("icu_root"), "Got: {}", r.sql);
    }

    #[test]
    fn query_collate_nocase_to_lower() {
        let r = translate("SELECT * FROM t WHERE name COLLATE NOCASE = 'Test'").unwrap();
        let sql = r.sql.to_lowercase();
        assert!(sql.contains("lower("), "Expected LOWER(), got: {}", r.sql);
        assert!(
            !sql.contains("collate nocase"),
            "COLLATE NOCASE should be removed, got: {}",
            r.sql
        );
    }

    #[test]
    fn query_orderby_unchanged_without_isnull() {
        let r = translate("SELECT * FROM t ORDER BY id").unwrap();
        assert!(
            !r.sql.to_uppercase().contains("NULLS LAST"),
            "Should not add NULLS LAST, got: {}",
            r.sql
        );
    }
}
