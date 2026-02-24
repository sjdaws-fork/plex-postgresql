/*
 * Plex PostgreSQL Interposing Shim - Step/Reset/Finalize Operations
 *
 * Handles sqlite3_step, sqlite3_reset, sqlite3_finalize, sqlite3_clear_bindings.
 * This is the main query execution module.
 */

#include "db_interpose.h"
#include "db_interpose_common.h"  // For platform_print_backtrace
#include "db_interpose_rust.h"
#include "db_interpose_step_cached_read_utils.h"
#include "db_interpose_step_read_utils.h"
#include "db_interpose_step_write_utils.h"
#include "pg_query_cache.h"
#include "shim_alloc.h"

// Forward declaration of the inner implementation
static int my_sqlite3_step_impl(sqlite3_stmt *pStmt);
static step_result_t step_handle_cached_stmt(sqlite3_stmt *pStmt);

// Thread-local flag: set to 1 by step_impl whenever SQLITE_ERROR is caused by
// a PG connection failure (not a SQL/logic error). Checked by the retry wrapper.
static __thread int step_pg_conn_error = 0;

// ============================================================================
// Step Function - Retry Wrapper (v0.9.34, fixes #8)
// ============================================================================
// When PG restarts, threads that already hold a pool connection see
// CONNECTION_BAD / PQsend failures. This wrapper catches SQLITE_ERROR,
// resets statement state, waits with exponential backoff, and retries.
// Every "return SQLITE_ERROR" inside step_impl releases all mutexes,
// so re-entry is safe and deadlock-free.

int my_sqlite3_step(sqlite3_stmt *pStmt) {
    static __thread int step_retry_count = 0;

    pg_stmt_t *dbg_stmt = pg_find_stmt(pStmt);
    const char *dbg_sql = NULL;
    sqlite3 *dbg_db = NULL;
    if (dbg_stmt) {
        dbg_sql = dbg_stmt->pg_sql ? dbg_stmt->pg_sql : dbg_stmt->sql;
    }
    if (!dbg_sql && orig_sqlite3_sql) {
        dbg_sql = orig_sqlite3_sql(pStmt);
    }
    if (orig_sqlite3_db_handle) {
        dbg_db = orig_sqlite3_db_handle(pStmt);
    }
    pg_exception_note_phase("step", dbg_sql, pStmt, dbg_db);

    int rc = my_sqlite3_step_impl(pStmt);

    // Backoff schedule from PLEX_PG_RETRY_DELAYS (default: 500,1000,2000,3000,4000 ms)
    int step_retry_delays_ms[PG_RETRY_MAX_DELAYS];
    int step_max_retries = 0;
    pg_get_retry_delays(step_retry_delays_ms, &step_max_retries);

    if (rc == SQLITE_ERROR && step_retry_count < step_max_retries) {
        pg_stmt_t *pg_stmt = pg_find_stmt(pStmt);
        if (pg_stmt && pg_stmt->is_pg && step_pg_conn_error) {
            // step_impl set step_pg_conn_error=1: error was connection-related
            step_pg_conn_error = 0;
            {
                int delay = step_retry_delays_ms[step_retry_count];
                step_retry_count++;
                LOG_ERROR("step: PG conn error, retry %d/%d in %dms (thread %p)",
                         step_retry_count, step_max_retries, delay, (void*)pthread_self());

                pthread_mutex_lock(&pg_stmt->mutex);
                pg_stmt_clear_result(pg_stmt);
                pthread_mutex_unlock(&pg_stmt->mutex);

                usleep(delay * 1000);
                step_pg_conn_error = 0;
                rc = my_sqlite3_step(pStmt);  // recursive retry

                if (step_retry_count > 0 && rc != SQLITE_ERROR) {
                    LOG_ERROR("step: retry succeeded after %d attempt(s)",
                             step_retry_count);
                }
                step_retry_count = 0;
                return rc;
            }
        }
    }

    if (step_retry_count > 0) {
        if (rc == SQLITE_ERROR) {
            LOG_ERROR("step: retries exhausted, returning SQLITE_ERROR");
        }
        // Success in recursive frame: reset counter silently
        step_retry_count = 0;
    }
    return rc;
}

// ============================================================================
// Step Function - Inner Implementation
// ============================================================================

static step_result_t step_handle_cached_stmt(sqlite3_stmt *pStmt) {
    sqlite3 *db = sqlite3_db_handle(pStmt);
    pg_connection_t *pg_conn = pg_find_connection(db);
    if (!pg_conn) pg_conn = pg_find_any_library_connection();

    if (!(pg_conn && pg_conn->is_pg_active && pg_conn->conn &&
          is_library_db_path(pg_conn->db_path))) {
        return STEP_RESULT_FALLBACK;
    }

    char *expanded_sql = sqlite3_expanded_sql(pStmt);
    const char *sql = expanded_sql ? expanded_sql : sqlite3_sql(pStmt);

    const char *orig_sql = sqlite3_sql(pStmt);
    if (sql && is_write_operation(sql) && !should_skip_sql(sql) && !should_skip_sql(orig_sql)) {
        if (sql && strcasestr(sql, "INSERT") && strcasestr(sql, "metadata_items")) {
            LOG_DEBUG("CACHED INSERT metadata_items:");
            LOG_DEBUG("  expanded_sql=%s", expanded_sql ? "YES" : "NO");
            LOG_DEBUG("  sql (first 300): %.300s", sql ? sql : "(null)");
        }
        if (sql && rust_is_junk_metadata_insert(sql)) {
            LOG_ERROR("GUARD: Blocked cached junk INSERT into metadata_items "
                      "(library_section_id=NULL, metadata_type=NULL)");
            if (expanded_sql) sqlite3_free(expanded_sql);
            return STEP_RESULT_DONE;
        }

        pg_stmt_t *cached = pg_find_cached_stmt(pStmt);
        if (cached && cached->write_executed) {
            if (expanded_sql) sqlite3_free(expanded_sql);
            return STEP_RESULT_DONE;
        }

        pg_connection_t *cached_exec_conn = NULL;
        if (step_cached_write_should_noop(pg_conn, sql, &cached_exec_conn)) {
            if (expanded_sql) sqlite3_free(expanded_sql);
            return STEP_RESULT_DONE;
        }

        sql_translation_t trans = sql_translate(sql);
        if (trans.success && trans.sql) {
            const char *exec_sql = trans.sql;
            char *insert_sql = step_cached_write_build_exec_sql(sql, trans.sql, &exec_sql);
            int cached_write_conn_error = 0;
            step_result_t cached_write_rc = step_cached_write_execute_and_finalize(
                &cached, pStmt, pg_conn, cached_exec_conn, sql, exec_sql, &cached_write_conn_error);
            if (insert_sql) free(insert_sql);
            if (cached_write_rc == STEP_RESULT_ERROR) {
                sql_translation_free(&trans);
                if (expanded_sql) sqlite3_free(expanded_sql);
                if (cached_write_conn_error) step_pg_conn_error = 1;
                return STEP_RESULT_ERROR;
            }
        }
        sql_translation_free(&trans);
        if (expanded_sql) sqlite3_free(expanded_sql);
        return STEP_RESULT_DONE;
    }

    if (sql && is_read_operation(sql) && !should_skip_sql(sql)) {
        pg_connection_t *cached_read_conn = step_pick_thread_connection(pg_conn);
        step_result_t cached_branch_rc = STEP_RESULT_FALLBACK;
        pg_stmt_t *cached = pg_find_cached_stmt(pStmt);
        int sqlite_result = orig_sqlite3_step ? orig_sqlite3_step(pStmt) : SQLITE_ERROR;

        if (sqlite_result == SQLITE_ROW || sqlite_result == SQLITE_DONE) {
            step_result_t cached_rc = STEP_RESULT_DONE;
            if (step_cached_read_finalize_advance(cached, expanded_sql, &cached_rc)) {
                return cached_rc;
            }

            sql_translation_t trans = sql_translate(sql);
            if (trans.success && trans.sql) {
                pg_stmt_t *new_stmt =
                    step_cached_read_prepare_stmt(cached, cached_read_conn, sql, pStmt, trans.sql);
                if (new_stmt) {
                    int conn_error = 0;
                    cached_branch_rc = step_cached_read_execute(
                        new_stmt, cached_read_conn, sql, trans.sql, &conn_error);
                    if (conn_error && cached_branch_rc == STEP_RESULT_ERROR) {
                        step_pg_conn_error = 1;
                    }
                }
            }
            sql_translation_free(&trans);
        }

        if (cached_branch_rc == STEP_RESULT_ROW ||
            cached_branch_rc == STEP_RESULT_DONE ||
            cached_branch_rc == STEP_RESULT_ERROR) {
            if (expanded_sql) sqlite3_free(expanded_sql);
            return cached_branch_rc;
        }

        if (expanded_sql) sqlite3_free(expanded_sql);
        return sqlite_result;
    }

    if (expanded_sql) sqlite3_free(expanded_sql);
    return STEP_RESULT_FALLBACK;
}

static int my_sqlite3_step_impl(sqlite3_stmt *pStmt) {
    // Periodic shim memory usage summary (every 60s, near-zero overhead)
    shim_alloc_maybe_log();

    // CRITICAL FIX v0.9.3: If we're inside resolve_column_tables, skip shim entirely
    // This prevents: resolve_column_tables → PQexec → (Plex hook) → my_sqlite3_step → recursion
    // Flag declared in db_interpose.h, defined in db_interpose_common.c
    if (in_resolve_tables) {
        // Pass through to original SQLite - no PostgreSQL involved
        return orig_sqlite3_step ? orig_sqlite3_step(pStmt) : SQLITE_ERROR;
    }
    
    pg_stmt_t *pg_stmt = pg_find_stmt(pStmt);

    // CRITICAL FIX v0.9.0: Set in_step flag to prevent concurrent bind
    if (pg_stmt) {
        atomic_store(&pg_stmt->in_step, 1);
    }

    // Skip statements
    if (pg_stmt && pg_stmt->is_pg == 3) {
        LOG_DEBUG("[RACE_DEBUG] STEP_END thread=%p stmt=%p rc=%d reason=skip", 
                  (void*)pthread_self(), (void*)pStmt, SQLITE_DONE);
        return SQLITE_DONE;
    }

    // Handle cached statements (prepared before our shim)
    if (!pg_stmt) {
        step_result_t cached_rc = step_handle_cached_stmt(pStmt);
        if (cached_rc != STEP_RESULT_FALLBACK) {
            return cached_rc;
        }
    }

    // Execute prepared statement on PostgreSQL
    // IMPORTANT: Use thread-local connection, not the one stored at prepare time
    // This ensures INSERT and SELECT on the same thread use the same connection
    // v0.9.4.5: Get connection from statement's db handle, not stored conn
    // This fixes the bug where stmt_conn was from wrong database (blobs.db vs library.db)
    pg_connection_t *exec_conn = NULL;

    if (pg_stmt && pg_stmt->shadow_stmt) {
        // Get the actual database handle from the statement
        sqlite3 *db = sqlite3_db_handle(pg_stmt->shadow_stmt);
        pg_connection_t *handle_conn = pg_find_connection(db);

        if (handle_conn && handle_conn->is_pg_active && is_library_db_path(handle_conn->db_path)) {
            // v0.9.5: Check if this is library.db (uses pool) or blobs.db (uses direct connection)
            // library.db has conn=NULL (pool-only), blobs.db has conn=PGconn* (direct)
            if (handle_conn->conn) {
                // Direct connection (blobs.db) - use it directly
                exec_conn = handle_conn;
            } else {
                // Pool connection (library.db) - get thread-local pool connection
                pg_connection_t *thread_conn = pg_get_thread_connection(handle_conn->db_path);
                if (thread_conn && thread_conn->is_pg_active && thread_conn->conn) {
                    exec_conn = thread_conn;
                    // CRITICAL FIX: Touch connection IMMEDIATELY after obtaining it
                    pg_pool_touch_connection(exec_conn);
                }
            }
        }
    }

    if (pg_stmt && pg_stmt->pg_sql && exec_conn && exec_conn->conn) {
        // Lock statement mutex to protect statement state
        // NOTE: exec_conn->mutex is NOT needed because each thread has its own
        // connection from the pool (per-thread connection model)
        pthread_mutex_lock(&pg_stmt->mutex);

        const char *paramValues[MAX_PARAMS] = {NULL};  // Initialize to prevent garbage access
        for (int i = 0; i < pg_stmt->param_count && i < MAX_PARAMS; i++) {
            paramValues[i] = pg_stmt->param_values[i];
        }

        if (pg_stmt->is_pg == 2) {  // READ
            // CRITICAL FIX: Prevent re-execution after SQLITE_DONE was returned
            // Without this, Plex calling step() after DONE would re-execute the query
            if (pg_stmt->read_done) {
                pthread_mutex_unlock(&pg_stmt->mutex);
                return SQLITE_DONE;
            }

            // CRITICAL FIX: Handle cached results FIRST, before checking pg_stmt->result
            // When using cache, result is NULL but cached_result is set.
            // The cache hit (below) sets current_row=0 and returns immediately.
            // On subsequent step() calls, we advance current_row here.
            if (pg_stmt->cached_result) {
                return step_read_advance_cached_result(pg_stmt);
            }

            step_read_log_debug_context(pg_stmt, exec_conn);
            step_read_prepare_reexecution_state(pg_stmt, exec_conn);

            // ================================================================
            // v0.9.28: SINGLE-ROW STREAMING MODE
            // Instead of fetching entire PGresult eagerly, we use
            // PQsetSingleRowMode to fetch one row at a time.
            // This matches SQLite's step() memory model: one row in memory.
            // ================================================================

             // === STREAMING: Subsequent step() — fetch next row ===
            if (pg_stmt->streaming_mode) {
                return step_read_streaming_next(pStmt, pg_stmt);
            }

            // === NON-STREAMING: Subsequent step() on eager result — advance row ===
            if (pg_stmt->result) {
                return step_read_eager_next(pg_stmt);
            }

            // === FIRST step() — send query and decide streaming vs eager ===
            if (!pg_stmt->result) {
                int conn_error = 0;
                step_result_t first_rc = step_read_first_execute(
                    pg_stmt, &exec_conn, paramValues, &conn_error);
                if (first_rc == STEP_RESULT_ERROR && conn_error) step_pg_conn_error = 1;
                return first_rc;
            }
        } else if (pg_stmt->is_pg == 1) {  // WRITE
            // CRITICAL FIX: Prevent duplicate execution of the same write
            // If Plex calls step() multiple times without reset(), only execute once
            if (pg_stmt->write_executed) {
                // Already executed this write, just return DONE
                // This prevents the statistics_media INSERT storm bug
                pthread_mutex_unlock(&pg_stmt->mutex);
                return SQLITE_DONE;
            }

            // Treat COMMIT/ROLLBACK/END as no-op when there is no active tx.
            // This matches SQLite behavior and avoids noisy PG warnings.
            int txn_state = PQTRANS_IDLE;
            if (step_pg_write_should_noop(exec_conn, pg_stmt ? pg_stmt->pg_sql : NULL, &txn_state)) {
                LOG_DEBUG("TXN_NOOP: skipping tx terminator in state=%d sql=%.120s",
                          txn_state, pg_stmt->pg_sql);
                pg_stmt->write_executed = 1;
                pthread_mutex_unlock(&pg_stmt->mutex);
                return SQLITE_DONE;
            }

            step_write_log_debug_context(pg_stmt, exec_conn, paramValues);

            if (step_write_should_skip_special_insert(pg_stmt, exec_conn, paramValues)) {
                pthread_mutex_unlock(&pg_stmt->mutex);
                return SQLITE_DONE;
            }

            int prep_conn_error = 0;
            step_result_t prep_rc = step_write_prepare_connection(pg_stmt, &exec_conn, &prep_conn_error);
            if (prep_rc == STEP_RESULT_ERROR) {
                if (prep_conn_error) {
                    do { step_pg_conn_error = 1; return SQLITE_ERROR; } while(0);
                }
                return SQLITE_ERROR;
            }

            int write_conn_error = 0;
            step_result_t write_rc = step_write_execute_and_finalize(
                pg_stmt, exec_conn, paramValues, &write_conn_error);
            if (write_rc == STEP_RESULT_ERROR) {
                pthread_mutex_unlock(&pg_stmt->mutex);
                if (write_conn_error) {
                    do { step_pg_conn_error = 1; return SQLITE_ERROR; } while(0);
                }
                return SQLITE_ERROR;
            }
        }

        pthread_mutex_unlock(&pg_stmt->mutex);
    }

    if (pg_stmt && pg_stmt->is_pg) {
        // v0.8.9.5: WRITE statements always return SQLITE_DONE
        // SOCI expects this and uses last_insert_rowid() to get the ID via lastval()
        // The RETURNING result is kept for debugging but not exposed as SQLITE_ROW
        if (pg_stmt->is_pg == 1) return SQLITE_DONE;
    
    step_log_step_exit_trace(pg_stmt);
    }

    // Fallback to SQLite for non-PostgreSQL statements
    int final_rc = orig_sqlite3_step ? orig_sqlite3_step(pStmt) : SQLITE_ERROR;
    return final_rc;
}
