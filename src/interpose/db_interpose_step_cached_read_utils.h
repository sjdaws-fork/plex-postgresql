#ifndef DB_INTERPOSE_STEP_CACHED_READ_UTILS_H
#define DB_INTERPOSE_STEP_CACHED_READ_UTILS_H

#include "db_interpose.h"

#define STEP_CACHED_READ_UNHANDLED (-1)

int step_cached_read_maybe_advance(pg_stmt_t *cached, char *expanded_sql, int *sqlite_rc_out);
pg_stmt_t *step_cached_read_get_or_create_stmt(pg_stmt_t *cached,
                                               pg_connection_t *conn,
                                               const char *sql,
                                               sqlite3_stmt *pStmt,
                                               const char *translated_sql);
int step_cached_read_execute_translated(pg_stmt_t *stmt,
                                        pg_connection_t *conn,
                                        const char *orig_sql,
                                        const char *translated_sql,
                                        int *pg_conn_error_out);

#endif
