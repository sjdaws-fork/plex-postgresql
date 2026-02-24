#include "db_interpose_step_write_utils.h"
#include "db_interpose_txn_utils.h"
#include "db_interpose_rust.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

pg_connection_t *step_pick_thread_connection(pg_connection_t *base_conn) {
    if (!base_conn) return NULL;
    if (!is_library_db_path(base_conn->db_path)) return base_conn;

    pg_connection_t *thread_conn = pg_get_thread_connection(base_conn->db_path);
    if (thread_conn && thread_conn->is_pg_active && thread_conn->conn) {
        return thread_conn;
    }
    return base_conn;
}

int step_cached_write_should_noop(pg_connection_t *base_conn, const char *sql, pg_connection_t **out_exec_conn) {
    pg_connection_t *exec_conn = step_pick_thread_connection(base_conn);
    if (out_exec_conn) *out_exec_conn = exec_conn;
    return txn_terminator_should_noop(exec_conn, sql, NULL);
}

int step_pg_write_should_noop(pg_connection_t *exec_conn, const char *pg_sql, int *txn_state_out) {
    return txn_terminator_should_noop(exec_conn, pg_sql, txn_state_out);
}

char *step_cached_write_build_exec_sql(const char *orig_sql, const char *translated_sql, const char **exec_sql_out) {
    if (exec_sql_out) *exec_sql_out = translated_sql;
    if (!translated_sql) return NULL;

    char *owned = convert_metadata_settings_insert_to_upsert(translated_sql);
    if (owned) {
        if (exec_sql_out) *exec_sql_out = owned;
        return owned;
    }

    if (orig_sql && strncasecmp(orig_sql, "INSERT", 6) == 0 &&
        strcasestr(translated_sql, "schema_migrations") &&
        !strcasestr(translated_sql, "ON CONFLICT")) {
        size_t len = strlen(translated_sql);
        owned = malloc(len + 40);
        if (owned) {
            snprintf(owned, len + 40, "%s ON CONFLICT DO NOTHING", translated_sql);
            if (exec_sql_out) *exec_sql_out = owned;
        }
        return owned;
    }

    if (orig_sql && strncasecmp(orig_sql, "INSERT", 6) == 0 &&
        !strstr(translated_sql, "RETURNING") &&
        !strcasestr(translated_sql, "schema_migrations")) {
        size_t len = strlen(translated_sql);
        owned = malloc(len + 20);
        if (owned) {
            snprintf(owned, len + 20, "%s RETURNING id", translated_sql);
            if (exec_sql_out) *exec_sql_out = owned;
        }
    }

    return owned;
}
