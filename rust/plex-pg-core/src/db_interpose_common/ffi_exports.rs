use super::*;

#[no_mangle]
pub extern "C" fn rust_get_exception_tracker(
    type_name: *const c_char,
) -> *mut ExceptionTypeTracker {
    unsafe { get_exception_tracker_impl(type_name) }
}

#[no_mangle]
pub extern "C" fn rust_reset_exception_tracking() {
    reset_exception_tracking_impl();
}

#[no_mangle]
pub extern "C" fn get_exception_tracker(type_name: *const c_char) -> *mut ExceptionTypeTracker {
    rust_get_exception_tracker(type_name)
}

#[no_mangle]
pub extern "C" fn reset_exception_tracking() {
    rust_reset_exception_tracking();
}

#[no_mangle]
pub extern "C" fn reset_symbol_verification() {
    rust_reset_symbol_verification();
}

#[no_mangle]
pub extern "C" fn pg_check_fake_value(
    p_val: *mut crate::ffi_types::sqlite3_value,
) -> *mut PgFakeValue {
    rust_pg_check_fake_value(p_val)
}

#[no_mangle]
pub extern "C" fn is_library_db_path(path: *const c_char) -> c_int {
    crate::db_interpose_helpers::rust_is_library_db_path(path)
}

#[no_mangle]
pub extern "C" fn is_blobs_db_path(path: *const c_char) -> c_int {
    crate::db_interpose_helpers::rust_is_blobs_db_path(path)
}

#[no_mangle]
pub extern "C" fn common_load_sqlite_symbols(handle: *mut c_void) {
    rust_common_load_sqlite_symbols(handle);
}

#[no_mangle]
pub extern "C" fn shim_ensure_ready() -> c_int {
    rust_shim_ensure_ready()
}

#[no_mangle]
pub extern "C" fn worker_init() -> c_int {
    rust_worker_init()
}

#[no_mangle]
pub extern "C" fn worker_cleanup() {
    rust_worker_cleanup();
}

#[no_mangle]
pub extern "C" fn delegate_prepare_to_worker(
    db: *mut sqlite3,
    z_sql: *const c_char,
    n_byte: c_int,
    stmt: *mut *mut sqlite3_stmt,
    tail: *mut *const c_char,
) -> c_int {
    rust_delegate_prepare_to_worker(db, z_sql, n_byte, stmt, tail)
}

#[no_mangle]
pub extern "C" fn pg_exception_note_query(sql: *const c_char) {
    rust_pg_exception_note_query(sql);
}

#[no_mangle]
pub extern "C" fn pg_exception_dump_recent_queries() {
    rust_pg_exception_dump_recent_queries();
}

#[no_mangle]
pub extern "C" fn pg_exception_note_phase(
    phase: *const c_char,
    sql: *const c_char,
    stmt: *const c_void,
    db: *const c_void,
) {
    rust_pg_exception_note_phase(phase, sql, stmt, db);
}

#[no_mangle]
pub extern "C" fn pg_exception_dump_recent_phases() {
    rust_pg_exception_dump_recent_phases();
}
