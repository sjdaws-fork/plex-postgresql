#ifndef DB_INTERPOSE_CONN_UTILS_H
#define DB_INTERPOSE_CONN_UTILS_H

#include "db_interpose.h"

void step_conn_cancel_and_drain(pg_connection_t *conn, const char *scope_tag);

#endif
