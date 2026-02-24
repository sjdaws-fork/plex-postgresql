/*
 * Plex PostgreSQL Interposing Shim - Statement Lifecycle Operations
 *
 * Handles sqlite3_reset, sqlite3_finalize, sqlite3_clear_bindings.
 */

#include "db_interpose.h"

static int reset_pg_stmt_locked(sqlite3_stmt *pStmt, pg_stmt_t *stmt) {
    pthread_mutex_lock(&stmt->mutex);

    atomic_store(&stmt->in_step, 0);

    for (int i = 0; i < MAX_PARAMS; i++) {
        if (stmt->param_values[i] && !is_preallocated_buffer(stmt, i)) {
            free(stmt->param_values[i]);
            stmt->param_values[i] = NULL;
        }
    }
    pg_stmt_clear_result(stmt);

    int rc = SQLITE_OK;
    if (stmt->is_pg != 2) {
        rc = orig_sqlite3_reset ? orig_sqlite3_reset(pStmt) : SQLITE_ERROR;
    }

    pthread_mutex_unlock(&stmt->mutex);
    return rc;
}

int my_sqlite3_reset(sqlite3_stmt *pStmt) {
    pg_stmt_t *pg_stmt = pg_find_any_stmt(pStmt);
    if (pg_stmt) {
        return reset_pg_stmt_locked(pStmt, pg_stmt);
    }

    pg_stmt_t *cached = pg_find_cached_stmt(pStmt);
    if (cached) {
        return reset_pg_stmt_locked(pStmt, cached);
    }

    return orig_sqlite3_reset ? orig_sqlite3_reset(pStmt) : SQLITE_ERROR;
}

int my_sqlite3_finalize(sqlite3_stmt *pStmt) {
    pg_stmt_t *pg_stmt = pg_find_stmt(pStmt);
    int is_pg_only = 0;

    if (pg_stmt) {
        is_pg_only = (pg_stmt->is_pg == 2);

        pg_stmt_t *cached = pg_find_cached_stmt(pStmt);
        if (cached == pg_stmt) {
            LOG_DEBUG("finalize: stmt in both global and TLS, clearing TLS ref");
            pg_clear_cached_stmt(pStmt);
        } else if (cached) {
            LOG_INFO("finalize: different pg_stmt in global vs TLS for same sqlite_stmt (cross-thread re-prepare)");
            pg_clear_cached_stmt(pStmt);
        }

        pg_unregister_stmt(pStmt);
        pg_stmt_unref(pg_stmt);
    } else {
        pg_stmt_t *cached = pg_find_cached_stmt(pStmt);
        if (cached) {
            is_pg_only = (cached->is_pg == 2);
            LOG_DEBUG("finalize: stmt only in TLS (ref_count=%d), clearing",
                      atomic_load(&cached->ref_count));
            pg_clear_cached_stmt(pStmt);
            pg_stmt_unref(cached);
        }
    }

    if (is_pg_only) {
        return SQLITE_OK;
    }

    return orig_sqlite3_finalize ? orig_sqlite3_finalize(pStmt) : SQLITE_ERROR;
}

int my_sqlite3_clear_bindings(sqlite3_stmt *pStmt) {
    pg_stmt_t *pg_stmt = pg_find_stmt(pStmt);
    if (pg_stmt) {
        pthread_mutex_lock(&pg_stmt->mutex);
        for (int i = 0; i < MAX_PARAMS; i++) {
            if (pg_stmt->param_values[i] && !is_preallocated_buffer(pg_stmt, i)) {
                free(pg_stmt->param_values[i]);
                pg_stmt->param_values[i] = NULL;
            }
        }
        int rc = orig_sqlite3_clear_bindings ? orig_sqlite3_clear_bindings(pStmt) : SQLITE_ERROR;
        pthread_mutex_unlock(&pg_stmt->mutex);
        return rc;
    }
    return orig_sqlite3_clear_bindings ? orig_sqlite3_clear_bindings(pStmt) : SQLITE_ERROR;
}
