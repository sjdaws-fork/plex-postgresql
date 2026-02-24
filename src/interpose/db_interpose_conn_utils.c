#include "db_interpose_conn_utils.h"

void step_conn_cancel_and_drain(pg_connection_t *conn, const char *scope_tag) {
    if (!conn || !conn->conn) return;

    if (atomic_load(&conn->streaming_active)) return;

    PQsetnonblocking(conn->conn, 0);
    while (PQisBusy(conn->conn)) {
        PQconsumeInput(conn->conn);
    }

    PGcancel *cancel = PQgetCancel(conn->conn);
    if (cancel) {
        char errbuf[256];
        PQcancel(cancel, errbuf, sizeof(errbuf));
        PQfreeCancel(cancel);
    }

    PGresult *pending;
    int drain_count = 0;
    while ((pending = PQgetResult(conn->conn)) != NULL) {
        drain_count++;
        if (drain_count <= 3) {
            LOG_INFO("%s: Drained orphaned result from connection %p (status=%d: %s)",
                     scope_tag ? scope_tag : "STEP",
                     (void *)conn,
                     PQresultStatus(pending),
                     PQresStatus(PQresultStatus(pending)));
        }
        PQclear(pending);
        if (drain_count > 1000) {
            LOG_INFO("%s: Drain loop exceeded 1000 on %p - aborting drain",
                     scope_tag ? scope_tag : "STEP", (void *)conn);
            break;
        }
    }
    if (drain_count > 3) {
        LOG_INFO("%s: Drained %d orphaned results total from connection %p",
                 scope_tag ? scope_tag : "STEP", drain_count, (void *)conn);
    }
}
