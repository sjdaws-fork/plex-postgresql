#include "db_interpose_step_read_utils.h"
#include "pg_query_cache.h"

static void step_read_clear_row_caches(pg_stmt_t *stmt) {
    for (int i = 0; i < MAX_PARAMS; i++) {
        if (stmt->cached_text[i]) {
            free(stmt->cached_text[i]);
            stmt->cached_text[i] = NULL;
        }
        if (stmt->cached_blob[i]) {
            free(stmt->cached_blob[i]);
            stmt->cached_blob[i] = NULL;
            stmt->cached_blob_len[i] = 0;
        }
        if (stmt->decoded_blobs[i]) {
            free(stmt->decoded_blobs[i]);
            stmt->decoded_blobs[i] = NULL;
            stmt->decoded_blob_lens[i] = 0;
        }
    }
    stmt->cached_row = -1;
    stmt->decoded_blob_row = -1;
}

int step_read_advance_cached_result(pg_stmt_t *stmt) {
    if (!stmt || !stmt->cached_result) return SQLITE_ERROR;

    stmt->current_row++;
    if (stmt->current_row >= stmt->num_rows) {
        pg_query_cache_release(stmt->cached_result);
        stmt->cached_result = NULL;
        stmt->read_done = 1;
        pthread_mutex_unlock(&stmt->mutex);
        return SQLITE_DONE;
    }
    pthread_mutex_unlock(&stmt->mutex);
    return SQLITE_ROW;
}

int step_read_streaming_next(sqlite3_stmt *pStmt, pg_stmt_t *stmt) {
    if (!stmt || !stmt->streaming_mode || !stmt->streaming_conn || !stmt->streaming_conn->conn) {
        return SQLITE_ERROR;
    }

    if (stmt->result) {
        PQclear(stmt->result);
        stmt->result = NULL;
    }
    step_read_clear_row_caches(stmt);

    PGresult *row_res = PQgetResult(stmt->streaming_conn->conn);
    if (!row_res) {
        LOG_ERROR("STREAM: NULL result (unexpected!) stmt=%p sql=%.100s streaming_conn=%p",
                  (void *)pStmt, stmt->pg_sql ? stmt->pg_sql : "?",
                  (void *)stmt->streaming_conn);
        stmt->streaming_mode = 0;
        if (stmt->streaming_conn) atomic_store(&stmt->streaming_conn->streaming_active, 0);
        stmt->streaming_conn = NULL;
        stmt->read_done = 1;
        pthread_mutex_unlock(&stmt->mutex);
        return SQLITE_DONE;
    }

    ExecStatusType row_status = PQresultStatus(row_res);
    if (row_status == PGRES_SINGLE_TUPLE) {
        stmt->result = row_res;
        stmt->current_row = 0;
        stmt->num_rows = 1;
        stmt->num_cols = PQnfields(row_res);
        pthread_mutex_unlock(&stmt->mutex);
        return SQLITE_ROW;
    }
    if (row_status == PGRES_TUPLES_OK) {
        PQclear(row_res);
        PGresult *final_null = PQgetResult(stmt->streaming_conn->conn);
        if (final_null) PQclear(final_null);
        stmt->streaming_mode = 0;
        if (stmt->streaming_conn) atomic_store(&stmt->streaming_conn->streaming_active, 0);
        stmt->streaming_conn = NULL;
        stmt->read_done = 1;
        pthread_mutex_unlock(&stmt->mutex);
        return SQLITE_DONE;
    }

    const char *err = PQerrorMessage(stmt->streaming_conn->conn);
    LOG_ERROR("STREAM ERROR: %s (status=%d) sql=%.100s",
              err ? err : "(null)", (int)row_status, stmt->pg_sql ? stmt->pg_sql : "?");
    PQclear(row_res);
    PGresult *drain;
    while ((drain = PQgetResult(stmt->streaming_conn->conn)) != NULL) PQclear(drain);
    stmt->streaming_mode = 0;
    if (stmt->streaming_conn) atomic_store(&stmt->streaming_conn->streaming_active, 0);
    stmt->streaming_conn = NULL;
    stmt->read_done = 1;
    pthread_mutex_unlock(&stmt->mutex);
    return SQLITE_DONE;
}

int step_read_eager_next(pg_stmt_t *stmt) {
    if (!stmt || !stmt->result) return SQLITE_ERROR;

    stmt->current_row++;
    if (stmt->current_row >= stmt->num_rows) {
        PQclear(stmt->result);
        stmt->result = NULL;
        stmt->result_conn = NULL;
        stmt->read_done = 1;
        pthread_mutex_unlock(&stmt->mutex);
        return SQLITE_DONE;
    }
    pthread_mutex_unlock(&stmt->mutex);
    return SQLITE_ROW;
}
