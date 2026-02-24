#ifndef DB_INTERPOSE_STEP_READ_UTILS_H
#define DB_INTERPOSE_STEP_READ_UTILS_H

#include "db_interpose.h"
#include "db_interpose_step_result.h"

/*
 * Contract:
 * - Caller holds stmt->mutex on entry.
 * - Helpers may unlock stmt->mutex before returning.
 * - STEP_RESULT_ERROR + conn_error_out=1 means connection-level failure (retry-eligible).
 */

/* Advance cached result row and finalize when exhausted. May unlock stmt->mutex. */
step_result_t step_read_advance_cached_result(pg_stmt_t *stmt);

/* Fetch next row in streaming mode. May unlock stmt->mutex. */
step_result_t step_read_streaming_next(sqlite3_stmt *pStmt, pg_stmt_t *stmt);

/* Advance eager result rowset. May unlock stmt->mutex. */
step_result_t step_read_eager_next(pg_stmt_t *stmt);

/* First read execution path: acquire/recover conn + execute + initialize result mode. */
step_result_t step_read_first_execute(pg_stmt_t *stmt,
                                      pg_connection_t **exec_conn_io,
                                      const char *paramValues[MAX_PARAMS],
                                      int *pg_conn_error_out);

#endif
