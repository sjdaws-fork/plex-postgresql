#ifndef DB_INTERPOSE_STEP_READ_UTILS_H
#define DB_INTERPOSE_STEP_READ_UTILS_H

#include "db_interpose.h"

int step_read_advance_cached_result(pg_stmt_t *stmt);
int step_read_streaming_next(sqlite3_stmt *pStmt, pg_stmt_t *stmt);
int step_read_eager_next(pg_stmt_t *stmt);

#endif
