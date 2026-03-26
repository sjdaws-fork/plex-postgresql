#ifndef DB_INTERPOSE_TXN_UTILS_H
#define DB_INTERPOSE_TXN_UTILS_H

#include "db_interpose.h"

const char *skip_leading_sql_noise(const char *sql);
int is_txn_terminator_sql(const char *sql);
int txn_terminator_should_noop(pg_connection_t *conn, const char *sql, int *txn_state_out);

#endif
