use std::os::raw::c_void;
use std::sync::atomic::Ordering;

use crate::ffi_types::PgConnection;

use super::threading::{current_thread_id, threads_equal};
use super::{pool, PoolManager, SLOT_READY};

pub(super) fn conn_db_path(conn: &PgConnection) -> String {
    super::cbuf_to_string(&conn.db_path)
}

pub(super) fn conn_is_pg_active(conn: &PgConnection) -> bool {
    conn.is_pg_active != 0
}

pub(super) fn conn_is_streaming_active(conn: &PgConnection) -> bool {
    conn.streaming_active.load(Ordering::Acquire) != 0
}

/// Raw-pointer wrapper for callers that still hold `*mut PgConnection`.
/// Returns `false` when the pointer is null.
pub(super) fn conn_is_streaming_active_ptr(conn: *mut PgConnection) -> bool {
    if conn.is_null() {
        return false;
    }
    conn_is_streaming_active(unsafe { &*conn })
}

/// Raw-pointer wrapper for callers that still hold `*mut PgConnection`.
/// Returns `false` when the pointer is null.
pub(super) fn conn_is_pg_active_ptr(conn: *mut PgConnection) -> bool {
    if conn.is_null() {
        return false;
    }
    conn_is_pg_active(unsafe { &*conn })
}

pub(super) fn thread_streaming_connection_count(
    pm: &PoolManager,
    thread_id: u64,
    exclude_conn: *const c_void,
) -> usize {
    if thread_id == 0 {
        return 0;
    }

    let pool_size = pm.pool_size();
    let mut count = 0usize;
    for i in 0..pool_size {
        let slot = &pm.slots[i];
        if slot.state.load(Ordering::Acquire) != SLOT_READY {
            continue;
        }
        if !threads_equal(slot.owner_thread.load(Ordering::Acquire), thread_id) {
            continue;
        }

        let conn = slot.conn.load(Ordering::Acquire);
        if conn.is_null() || conn == exclude_conn as *mut c_void {
            continue;
        }
        if conn_is_streaming_active_ptr(conn as *mut PgConnection) {
            count += 1;
        }
    }
    count
}

pub(crate) fn current_thread_has_other_streaming_connection(exclude_conn: *const c_void) -> bool {
    let current_thread = current_thread_id();
    thread_streaming_connection_count(pool(), current_thread, exclude_conn) != 0
}
