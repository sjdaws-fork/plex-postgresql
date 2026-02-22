/// Module: upsert
///
/// Rewrites SQLite INSERT OR REPLACE / INSERT OR IGNORE / REPLACE INTO
/// to PostgreSQL ON CONFLICT syntax:
///
///   INSERT OR REPLACE INTO t (cols) VALUES (vals)
///     → INSERT INTO t (cols) VALUES (vals) ON CONFLICT DO UPDATE SET col1=EXCLUDED.col1, ...
///
///   INSERT OR IGNORE INTO t (cols) VALUES (vals)
///     → INSERT INTO t (cols) VALUES (vals) ON CONFLICT DO NOTHING
///
///   REPLACE INTO t (cols) VALUES (vals)
///     → same as OR REPLACE
use sqlparser::ast::*;

pub fn transform(stmt: &mut Statement) {
    if let Statement::Insert(insert) = stmt {
        transform_insert(insert);
    }
}

fn transform_insert(insert: &mut Insert) {
    // Skip if already has ON CONFLICT clause
    if insert.on.is_some() {
        return;
    }

    match insert.or {
        Some(SqliteOnConflict::Replace) => {
            // INSERT OR REPLACE → ON CONFLICT DO UPDATE SET col=EXCLUDED.col, ...
            let columns = insert.columns.clone();
            insert.on = Some(OnInsert::OnConflict(make_do_update(columns)));
            insert.or = None;
            // Also clear replace_into flag if set
            insert.replace_into = false;
        }
        Some(SqliteOnConflict::Ignore) => {
            // INSERT OR IGNORE → ON CONFLICT DO NOTHING
            insert.on = Some(OnInsert::OnConflict(OnConflict {
                conflict_target: None,
                action: OnConflictAction::DoNothing,
            }));
            insert.or = None;
        }
        _ => {
            // No SQLite conflict modifier – nothing to do
        }
    }
}

/// Build `ON CONFLICT DO UPDATE SET col1 = EXCLUDED.col1, col2 = EXCLUDED.col2, ...`
fn make_do_update(columns: Vec<Ident>) -> OnConflict {
    let assignments: Vec<Assignment> = columns
        .iter()
        .map(|col| Assignment {
            target: AssignmentTarget::ColumnName(ObjectName(vec![ObjectNamePart::Identifier(
                col.clone(),
            )])),
            value: Expr::CompoundIdentifier(vec![Ident::new("EXCLUDED"), col.clone()]),
        })
        .collect();

    OnConflict {
        conflict_target: None,
        action: OnConflictAction::DoUpdate(DoUpdate {
            assignments,
            selection: None,
        }),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::translate;

    #[test]
    fn upsert_insert_or_replace() {
        let r = translate("INSERT OR REPLACE INTO settings(id, value) VALUES(?, ?)").unwrap();
        assert!(r.sql.to_uppercase().contains("ON CONFLICT"));
        assert!(r.sql.to_uppercase().contains("DO UPDATE"));
        assert!(!r.sql.to_uppercase().contains("OR REPLACE"));
    }

    #[test]
    fn upsert_insert_or_ignore() {
        let r = translate("INSERT OR IGNORE INTO tags(tag) VALUES(?)").unwrap();
        assert!(r.sql.to_uppercase().contains("ON CONFLICT"));
        assert!(r.sql.to_uppercase().contains("DO NOTHING"));
        assert!(!r.sql.to_uppercase().contains("OR IGNORE"));
    }

    #[test]
    fn upsert_replace_into() {
        let r = translate("REPLACE INTO settings(id, value) VALUES(?, ?)").unwrap();
        assert!(r.sql.to_uppercase().contains("ON CONFLICT"));
        assert!(r.sql.to_uppercase().contains("DO UPDATE"));
        assert!(!r.sql.to_uppercase().contains("REPLACE INTO"));
    }

    #[test]
    fn upsert_normal_insert_unchanged() {
        let r = translate("INSERT INTO t (a, b) VALUES (1, 2)").unwrap();
        assert!(!r.sql.to_uppercase().contains("ON CONFLICT"));
    }

    #[test]
    fn upsert_on_conflict_already_present_unchanged() {
        let r = translate(
            "INSERT INTO settings(id, value) VALUES(?, ?) ON CONFLICT(id) DO UPDATE SET value = excluded.value",
        )
        .unwrap();
        assert!(r.sql.to_uppercase().contains("ON CONFLICT"));
        let count = r.sql.to_uppercase().matches("ON CONFLICT").count();
        assert_eq!(count, 1, "ON CONFLICT should appear exactly once");
    }
}
