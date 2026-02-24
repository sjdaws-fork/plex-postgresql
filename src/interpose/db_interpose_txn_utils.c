#include "db_interpose_txn_utils.h"
#include <strings.h>

const char *skip_leading_sql_noise(const char *sql) {
    if (!sql) return "";
    const char *p = sql;
    for (;;) {
        while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
        if (p[0] == '-' && p[1] == '-') {
            p += 2;
            while (*p && *p != '\n') p++;
            continue;
        }
        if (p[0] == '/' && p[1] == '*') {
            p += 2;
            while (p[0] && !(p[0] == '*' && p[1] == '/')) p++;
            if (p[0]) p += 2;
            continue;
        }
        break;
    }
    return p;
}

int is_txn_terminator_sql(const char *sql) {
    const char *s = skip_leading_sql_noise(sql);
    return strncasecmp(s, "commit", 6) == 0 ||
           strncasecmp(s, "rollback", 8) == 0 ||
           strncasecmp(s, "end", 3) == 0;
}

int txn_terminator_should_noop(pg_connection_t *conn, const char *sql, int *txn_state_out) {
    if (txn_state_out) *txn_state_out = PQTRANS_IDLE;
    if (!conn || !conn->conn || !is_txn_terminator_sql(sql)) return 0;

    int txn_state = PQTRANS_IDLE;
    pthread_mutex_lock(&conn->mutex);
    if (conn->conn) {
        txn_state = (int)PQtransactionStatus(conn->conn);
    }
    pthread_mutex_unlock(&conn->mutex);

    if (txn_state_out) *txn_state_out = txn_state;
    return txn_state != PQTRANS_INTRANS && txn_state != PQTRANS_INERROR;
}
