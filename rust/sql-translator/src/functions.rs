use sqlparser::ast::helpers::attached_token::AttachedToken;
/// Module: functions
///
/// Rewrites SQLite function calls to their PostgreSQL equivalents:
///   IFNULL(a,b)           → COALESCE(a,b)
///   iif(cond,then,else)   → CASE WHEN cond THEN then ELSE else END
///   typeof(x)             → pg_typeof(x)::text
///   last_insert_rowid()   → lastval()
///   json_each(x)          → json_array_elements(x)
///   SUBSTR(a,b,c)         → SUBSTRING(a,b,c)
///   instr(a,b)            → STRPOS(b,a)   (arg order swapped)
///   strftime('%s',…)      → EXTRACT(EPOCH FROM …)::bigint
///   strftime('%Y-%m-%d',…)→ TO_CHAR(…, 'YYYY-MM-DD')
///   unixepoch(…)          → EXTRACT(EPOCH FROM …)::bigint
use sqlparser::ast::*;

pub fn transform(stmt: &mut Statement) {
    transform_stmt(stmt);
}

fn transform_stmt(stmt: &mut Statement) {
    match stmt {
        Statement::Query(q) => transform_query(q),
        Statement::Insert(ins) => {
            if let Some(src) = &mut ins.source {
                transform_query(src);
            }
        }
        Statement::Update(update) => {
            for assign in &mut update.assignments {
                transform_expr_inplace(&mut assign.value);
            }
            if let Some(sel) = &mut update.selection {
                transform_expr_inplace(sel);
            }
        }
        Statement::Delete(del) => {
            if let Some(sel) = &mut del.selection {
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
    transform_set_expr(&mut q.body);
    if let Some(ob) = &mut q.order_by {
        if let OrderByKind::Expressions(exprs) = &mut ob.kind {
            for oe in exprs {
                transform_expr_inplace(&mut oe.expr);
            }
        }
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
    for item in &mut sel.projection {
        match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => {
                transform_expr_inplace(e);
            }
            _ => {}
        }
    }
    for table in &mut sel.from {
        transform_table_with_joins(table);
    }
    if let Some(sel_where) = &mut sel.selection {
        transform_expr_inplace(sel_where);
    }
    match &mut sel.group_by {
        GroupByExpr::Expressions(exprs, _) => {
            for e in exprs {
                transform_expr_inplace(e);
            }
        }
        _ => {}
    }
    if let Some(having) = &mut sel.having {
        transform_expr_inplace(having);
    }
}

fn transform_table_with_joins(t: &mut TableWithJoins) {
    transform_table_factor(&mut t.relation);
    for join in &mut t.joins {
        transform_table_factor(&mut join.relation);
        match &mut join.join_operator {
            JoinOperator::Inner(JoinConstraint::On(e))
            | JoinOperator::LeftOuter(JoinConstraint::On(e))
            | JoinOperator::RightOuter(JoinConstraint::On(e))
            | JoinOperator::FullOuter(JoinConstraint::On(e)) => {
                transform_expr_inplace(e);
            }
            _ => {}
        }
    }
}

fn transform_table_factor(f: &mut TableFactor) {
    if let TableFactor::Derived { subquery, .. } = f {
        transform_query(subquery);
    }
}

/// The main recursive transformer.  Returns the expression that should
/// replace the input (usually `None` → keep self, but for `iif` we
/// swap the whole node).
fn transform_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Function(mut func) => {
            // Get the lowercase function name for matching
            let fn_name = func
                .name
                .0
                .first()
                .and_then(|p| match p {
                    ObjectNamePart::Identifier(i) => Some(i.value.to_lowercase()),
                    _ => None,
                })
                .unwrap_or_default();

            // First recurse into the args
            if let FunctionArguments::List(ref mut arg_list) = func.args {
                for arg in &mut arg_list.args {
                    match arg {
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => {
                            *e = transform_expr(std::mem::replace(
                                e,
                                Expr::Value(ValueWithSpan {
                                    value: Value::Null,
                                    span: sqlparser::tokenizer::Span::empty(),
                                }),
                            ));
                        }
                        FunctionArg::Named {
                            arg: FunctionArgExpr::Expr(e),
                            ..
                        } => {
                            *e = transform_expr(std::mem::replace(
                                e,
                                Expr::Value(ValueWithSpan {
                                    value: Value::Null,
                                    span: sqlparser::tokenizer::Span::empty(),
                                }),
                            ));
                        }
                        _ => {}
                    }
                }
            }

            match fn_name.as_str() {
                // IFNULL → COALESCE
                "ifnull" => {
                    rename_function(&mut func, "COALESCE");
                    Expr::Function(func)
                }

                // iif(cond, then, else) → CASE WHEN cond THEN then ELSE else END
                "iif" => {
                    let args = extract_unnamed_args(&mut func);
                    if args.len() >= 3 {
                        let mut iter = args.into_iter();
                        let cond = iter.next().unwrap();
                        let then = iter.next().unwrap();
                        let else_e = iter.next().unwrap();
                        Expr::Case {
                            case_token: AttachedToken::empty(),
                            end_token: AttachedToken::empty(),
                            operand: None,
                            conditions: vec![CaseWhen {
                                condition: cond,
                                result: then,
                            }],
                            else_result: Some(Box::new(else_e)),
                        }
                    } else {
                        Expr::Function(func)
                    }
                }

                // typeof(x) → pg_typeof(x)::text
                "typeof" => {
                    rename_function(&mut func, "pg_typeof");
                    Expr::Cast {
                        kind: CastKind::DoubleColon,
                        expr: Box::new(Expr::Function(func)),
                        data_type: DataType::Text,
                        array: false,
                        format: None,
                    }
                }

                // last_insert_rowid() → lastval()
                "last_insert_rowid" => {
                    rename_function(&mut func, "lastval");
                    Expr::Function(func)
                }

                // json_each(x) → json_array_elements(x)
                "json_each" => {
                    rename_function(&mut func, "json_array_elements");
                    Expr::Function(func)
                }

                // SUBSTR → SUBSTRING
                "substr" => {
                    rename_function(&mut func, "SUBSTRING");
                    Expr::Function(func)
                }

                // instr(a, b) → STRPOS(b, a)  (args swapped)
                "instr" => {
                    rename_function(&mut func, "STRPOS");
                    // swap argument order
                    if let FunctionArguments::List(ref mut al) = func.args {
                        if al.args.len() == 2 {
                            al.args.swap(0, 1);
                        }
                    }
                    Expr::Function(func)
                }

                // strftime('%s', expr) → EXTRACT(EPOCH FROM expr)::bigint
                // strftime('%Y-%m-%d', expr) → TO_CHAR(expr, 'YYYY-MM-DD')
                "strftime" => transform_strftime(func),

                // unixepoch(expr) → EXTRACT(EPOCH FROM expr)::bigint
                "unixepoch" => {
                    let args = extract_unnamed_args(&mut func);
                    let inner_expr = args.into_iter().next().unwrap_or_else(|| make_now_call());
                    let source = if is_now_literal(&inner_expr) {
                        make_now_call()
                    } else {
                        inner_expr
                    };
                    Expr::Cast {
                        kind: CastKind::DoubleColon,
                        expr: Box::new(Expr::Extract {
                            field: DateTimeField::Epoch,
                            syntax: ExtractSyntax::From,
                            expr: Box::new(source),
                        }),
                        data_type: DataType::BigInt(None),
                        array: false,
                        format: None,
                    }
                }

                _ => Expr::Function(func),
            }
        }

        // Recurse into other expression forms
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(transform_expr(*left)),
            op,
            right: Box::new(transform_expr(*right)),
        },
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
        Expr::Case {
            case_token,
            end_token,
            operand,
            conditions,
            else_result,
        } => {
            let operand = operand.map(|o| Box::new(transform_expr(*o)));
            let conditions = conditions
                .into_iter()
                .map(|cw| CaseWhen {
                    condition: transform_expr(cw.condition),
                    result: transform_expr(cw.result),
                })
                .collect();
            let else_result = else_result.map(|e| Box::new(transform_expr(*e)));
            Expr::Case {
                case_token,
                end_token,
                operand,
                conditions,
                else_result,
            }
        }
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
        // SUBSTR(a,b,c) is parsed as Expr::Substring { shorthand: true }
        // We convert it to SUBSTRING by clearing the shorthand flag
        Expr::Substring {
            expr,
            substring_from,
            substring_for,
            special,
            shorthand: true,
        } => Expr::Substring {
            expr: Box::new(transform_expr(*expr)),
            substring_from: substring_from.map(|e| Box::new(transform_expr(*e))),
            substring_for: substring_for.map(|e| Box::new(transform_expr(*e))),
            special,
            shorthand: false, // force SUBSTRING output
        },
        other => other,
    }
}

/// In-place wrapper for `transform_expr`.
fn transform_expr_inplace(expr: &mut Expr) {
    let taken = std::mem::replace(
        expr,
        Expr::Value(ValueWithSpan {
            value: Value::Null,
            span: sqlparser::tokenizer::Span::empty(),
        }),
    );
    *expr = transform_expr(taken);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn rename_function(func: &mut Function, new_name: &str) {
    func.name = ObjectName(vec![ObjectNamePart::Identifier(Ident::new(new_name))]);
}

/// Extract all `FunctionArg::Unnamed(FunctionArgExpr::Expr(…))` args.
fn extract_unnamed_args(func: &mut Function) -> Vec<Expr> {
    let mut result = Vec::new();
    if let FunctionArguments::List(ref mut al) = func.args {
        for arg in al.args.drain(..) {
            if let FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) = arg {
                result.push(e);
            }
        }
    }
    result
}

fn is_now_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Value(ValueWithSpan { value: Value::SingleQuotedString(s), .. }) if s.to_lowercase() == "now")
}

fn make_now_call() -> Expr {
    Expr::Function(Function {
        name: ObjectName(vec![ObjectNamePart::Identifier(Ident::new("NOW"))]),
        uses_odbc_syntax: false,
        parameters: FunctionArguments::None,
        args: FunctionArguments::List(FunctionArgumentList {
            duplicate_treatment: None,
            args: vec![],
            clauses: vec![],
        }),
        filter: None,
        null_treatment: None,
        over: None,
        within_group: vec![],
    })
}

fn transform_strftime(mut func: Function) -> Expr {
    let args = extract_unnamed_args(&mut func);
    if args.len() < 2 {
        return Expr::Function(func);
    }
    let mut iter = args.into_iter();
    let fmt_arg = iter.next().unwrap();
    let time_arg = iter.next().unwrap();

    let fmt_str = match &fmt_arg {
        Expr::Value(ValueWithSpan {
            value: Value::SingleQuotedString(s),
            ..
        }) => s.clone(),
        _ => return Expr::Function(func),
    };

    let source = if is_now_literal(&time_arg) {
        make_now_call()
    } else {
        time_arg
    };

    match fmt_str.as_str() {
        "%s" => {
            // → EXTRACT(EPOCH FROM source)::bigint
            Expr::Cast {
                kind: CastKind::DoubleColon,
                expr: Box::new(Expr::Extract {
                    field: DateTimeField::Epoch,
                    syntax: ExtractSyntax::From,
                    expr: Box::new(source),
                }),
                data_type: DataType::BigInt(None),
                array: false,
                format: None,
            }
        }
        "%Y-%m-%d" => {
            // → TO_CHAR(source, 'YYYY-MM-DD')
            let pg_fmt = Expr::Value(ValueWithSpan {
                value: Value::SingleQuotedString("YYYY-MM-DD".to_string()),
                span: sqlparser::tokenizer::Span::empty(),
            });
            Expr::Function(Function {
                name: ObjectName(vec![ObjectNamePart::Identifier(Ident::new("TO_CHAR"))]),
                uses_odbc_syntax: false,
                parameters: FunctionArguments::None,
                args: FunctionArguments::List(FunctionArgumentList {
                    duplicate_treatment: None,
                    args: vec![
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(source)),
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(pg_fmt)),
                    ],
                    clauses: vec![],
                }),
                filter: None,
                null_treatment: None,
                over: None,
                within_group: vec![],
            })
        }
        _ => Expr::Function(func),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::translate;

    #[test]
    fn function_ifnull_to_coalesce() {
        let r = translate("SELECT IFNULL(a, 0) FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("COALESCE"));
        assert!(!r.sql.to_uppercase().contains("IFNULL"));
    }

    #[test]
    fn function_iif_to_case() {
        let r = translate("SELECT iif(a > 0, 'yes', 'no') FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("CASE WHEN"));
        assert!(r.sql.to_uppercase().contains("THEN"));
        assert!(r.sql.to_uppercase().contains("ELSE"));
        assert!(!r.sql.to_lowercase().contains("iif"));
    }

    #[test]
    fn function_typeof_to_pg_typeof() {
        let r = translate("SELECT typeof(x) FROM t").unwrap();
        assert!(r.sql.contains("pg_typeof") || r.sql.contains("PG_TYPEOF"));
    }

    #[test]
    fn function_last_insert_rowid_to_lastval() {
        let r = translate("SELECT last_insert_rowid()").unwrap();
        assert!(r.sql.to_lowercase().contains("lastval()"));
        assert!(!r.sql.to_lowercase().contains("last_insert_rowid"));
    }

    #[test]
    fn function_substr_to_substring() {
        let r = translate("SELECT SUBSTR(a, 1, 5) FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("SUBSTRING"));
    }

    #[test]
    fn function_length_preserved() {
        let r = translate("SELECT LENGTH(name) FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("LENGTH"));
    }

    #[test]
    fn function_instr_to_strpos() {
        let r = translate("SELECT instr(haystack, needle) FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("STRPOS"));
        assert!(!r.sql.to_lowercase().contains("instr"));
    }

    #[test]
    fn function_strftime_epoch() {
        let r = translate("SELECT strftime('%s', 'now') FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("EXTRACT"));
        assert!(r.sql.to_uppercase().contains("EPOCH"));
    }

    #[test]
    fn function_unixepoch_now() {
        let r = translate("SELECT unixepoch('now') FROM t").unwrap();
        assert!(r.sql.to_uppercase().contains("EXTRACT"));
        assert!(r.sql.to_uppercase().contains("EPOCH"));
    }
}
