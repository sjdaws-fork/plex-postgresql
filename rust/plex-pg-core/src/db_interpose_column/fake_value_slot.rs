use super::*;

pub(super) unsafe fn allocate_fake_sqlite_value(
    pg_stmt: *mut PgStmt,
    idx: c_int,
    row: c_int,
) -> *mut sqlite3_value {
    let mut guard = crate::db_interpose_common::FAKE_VALUES
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let slot = {
        let cur = guard.next;
        guard.next = guard.next.wrapping_add(1);
        (cur as usize) & (MAX_FAKE_VALUES - 1)
    };
    let fake = &mut guard.pool[slot];
    fake.magic = PG_FAKE_VALUE_MAGIC;
    fake.pg_stmt = pg_stmt as *mut c_void;
    fake.col_idx = idx;
    fake.row_idx = row;
    fake.owner_thread = libc::pthread_self();
    fake as *mut PgFakeValue as *mut sqlite3_value
}
