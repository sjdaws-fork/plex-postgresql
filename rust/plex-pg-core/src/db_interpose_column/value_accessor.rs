use super::*;

unsafe fn column_value_debug_sql(dbg_stmt: *mut PgStmt) -> *const c_char {
    if dbg_stmt.is_null() {
        return ptr::null();
    }

    if !(*dbg_stmt).pg_sql.is_null() {
        (*dbg_stmt).pg_sql
    } else {
        (*dbg_stmt).sql
    }
}

unsafe fn passthrough_column_value(p_stmt: *mut sqlite3_stmt, idx: c_int) -> *mut sqlite3_value {
    orig_sqlite3_column_value
        .map(|f| f(p_stmt, idx))
        .unwrap_or(ptr::null_mut())
}

pub(super) fn column_value_impl(p_stmt: *mut sqlite3_stmt, idx: c_int) -> *mut sqlite3_value {
    let dbg_stmt = unsafe { pg_find_any_stmt(p_stmt) };
    let dbg_sql = unsafe { column_value_debug_sql(dbg_stmt) };
    let dbg_db = unsafe {
        orig_sqlite3_db_handle
            .map(|f| f(p_stmt))
            .unwrap_or(ptr::null_mut())
    };
    unsafe {
        pg_exception_note_phase(
            b"column_value\0".as_ptr() as *const c_char,
            dbg_sql,
            p_stmt,
            dbg_db,
        );
    }

    let pg_stmt = dbg_stmt;
    if pg_stmt.is_null() || unsafe { (*pg_stmt).is_pg == 0 } {
        return unsafe { passthrough_column_value(p_stmt, idx) };
    }

    if env_utils::env_truthy_str("PLEX_PG_DISABLE_COLUMN_VALUE") {
        return ptr::null_mut();
    }

    // Call ensure_pg_result_for_metadata BEFORE acquiring stmt mutex to avoid
    // ABBA deadlock (stmt mutex -> conn mutex).
    let needs_metadata = unsafe {
        (*pg_stmt).result.is_null()
            && (*pg_stmt).cached_result.is_null()
            && !(*pg_stmt).pg_sql.is_null()
    };
    if needs_metadata {
        if !ensure_pg_result_for_metadata(pg_stmt) {
            return unsafe { passthrough_column_value(p_stmt, idx) };
        }
    }

    // Hold mutex only for data reads — no logging inside this block
    // to avoid ABBA deadlock between stmt mutex and LOGGER mutex.
    // Extract row value, then release mutex before calling allocate_fake_sqlite_value
    // (which acquires fake_value_mutex).
    let row = {
        let _guard = unsafe { PthreadMutexGuard::lock(&mut (*pg_stmt).mutex as *mut _) };

        if unsafe { (*pg_stmt).result.is_null() } {
            return unsafe { passthrough_column_value(p_stmt, idx) };
        }

        if idx < 0 || idx >= unsafe { (*pg_stmt).num_cols } {
            return ptr::null_mut();
        }

        unsafe { (*pg_stmt).current_row }
    };
    // Mutex released here.

    // allocate_fake_sqlite_value acquires fake_value_mutex — called outside stmt mutex.
    unsafe { allocate_fake_sqlite_value(pg_stmt, idx, row) }
}
