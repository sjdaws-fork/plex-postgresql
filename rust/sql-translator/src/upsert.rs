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

    // Look up conflict target (PK column) from table name
    let table_name = match &insert.table {
        TableObject::TableName(name) => name
            .0
            .last()
            .and_then(|p| match p {
                ObjectNamePart::Identifier(i) => Some(i.value.to_lowercase()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => String::new(),
    };
    let conflict_target =
        get_table_pk(&table_name).map(|pk| ConflictTarget::Columns(vec![Ident::new(pk)]));

    match insert.or {
        Some(SqliteOnConflict::Replace) => {
            // INSERT OR REPLACE → ON CONFLICT (pk) DO UPDATE SET col=EXCLUDED.col, ...
            let columns = insert.columns.clone();
            let pk_name = get_table_pk(&table_name).unwrap_or("id");
            insert.on = Some(OnInsert::OnConflict(make_do_update(
                columns,
                conflict_target,
                pk_name,
            )));
            insert.or = None;
            // Also clear replace_into flag if set
            insert.replace_into = false;
        }
        Some(SqliteOnConflict::Ignore) => {
            // INSERT OR IGNORE → ON CONFLICT DO NOTHING
            insert.on = Some(OnInsert::OnConflict(OnConflict {
                conflict_target,
                action: OnConflictAction::DoNothing,
            }));
            insert.or = None;
        }
        _ => {
            // No SQLite conflict modifier – nothing to do
        }
    }
}

/// Known Plex table primary key columns
fn get_table_pk(table_name: &str) -> Option<&'static str> {
    match table_name.to_lowercase().as_str() {
        "tags"
        | "taggings"
        | "metadata_items"
        | "media_items"
        | "media_parts"
        | "media_streams"
        | "settings"
        | "preferences"
        | "accounts"
        | "directories"
        | "library_sections"
        | "statistics_bandwidth"
        | "statistics_media"
        | "statistics_resources"
        | "devices"
        | "play_queue_items"
        | "play_queue_generators"
        | "versioned_metadata_items"
        | "external_metadata_items"
        | "external_metadata_sources"
        | "metadata_item_settings"
        | "metadata_item_views"
        | "metadata_item_accounts"
        | "metadata_item_clusterings"
        | "media_item_settings"
        | "media_provider_resources"
        | "media_subscriptions"
        | "metadata_relations"
        | "metadata_subscription_desired_items"
        | "sync_schema_versions"
        | "locatables"
        | "spellfix_metadata_titles"
        | "section_locations"
        | "hub_templates" => Some("id"),
        "schema_migrations" => Some("version"),
        _ => None,
    }
}

/// Build `ON CONFLICT (pk) DO UPDATE SET col1 = EXCLUDED.col1, col2 = EXCLUDED.col2, ...`
/// Excludes the PK column from the SET assignments.
fn make_do_update(
    columns: Vec<Ident>,
    conflict_target: Option<ConflictTarget>,
    pk_name: &str,
) -> OnConflict {
    let assignments: Vec<Assignment> = columns
        .iter()
        .filter(|col| col.value.to_lowercase() != pk_name.to_lowercase())
        .map(|col| Assignment {
            target: AssignmentTarget::ColumnName(ObjectName(vec![ObjectNamePart::Identifier(
                col.clone(),
            )])),
            value: Expr::CompoundIdentifier(vec![Ident::new("EXCLUDED"), col.clone()]),
        })
        .collect();

    OnConflict {
        conflict_target,
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
