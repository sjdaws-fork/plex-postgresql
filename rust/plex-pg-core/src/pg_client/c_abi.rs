use std::os::raw::{c_char, c_int, c_void};

use crate::ffi_types::{sqlite3, PgConnection};
use crate::libpq_helpers::{rust_pq_result_error_field, PGresult};

use super::{
    rust_get_global_last_insert_rowid, rust_get_global_metadata_id, rust_hash_sql,
    rust_is_duplicate_sqlstate, rust_is_stale_sqlstate, rust_pg_client_cleanup,
    rust_pg_client_init, rust_pg_close, rust_pg_connect, rust_pg_ensure_connection,
    rust_pg_find_any_library_connection, rust_pg_find_connection, rust_pg_find_handle_connection,
    rust_pg_register_connection, rust_pg_unregister_connection, rust_pool_check_health,
    rust_pool_cleanup_after_fork, rust_pool_clear_streaming_active, rust_pool_get_connection,
    rust_pool_get_connection_excluding, rust_pool_is_live_connection,
    rust_pool_is_tracked_connection, rust_pool_release_for_db, rust_pool_touch_connection,
    rust_pool_validate_connection, rust_set_global_last_insert_rowid, rust_set_global_metadata_id,
    rust_stmt_cache_add, rust_stmt_cache_clear, rust_stmt_cache_clear_local,
    rust_stmt_cache_lookup,
};

const PG_DIAG_SQLSTATE: c_int = b'C' as c_int;

#[no_mangle]
pub extern "C" fn pg_client_init() {
    rust_pg_client_init();
}

#[no_mangle]
pub extern "C" fn pg_client_cleanup() {
    rust_pg_client_cleanup();
}

#[no_mangle]
pub extern "C" fn pg_connect(db_path: *const c_char, shadow_db: *mut sqlite3) -> *mut PgConnection {
    rust_pg_connect(db_path, shadow_db)
}

#[no_mangle]
pub extern "C" fn pg_close(conn: *mut PgConnection) {
    rust_pg_close(conn);
}

#[no_mangle]
pub extern "C" fn pg_ensure_connection(conn: *mut PgConnection) -> c_int {
    rust_pg_ensure_connection(conn)
}

#[no_mangle]
pub extern "C" fn pg_register_connection(conn: *mut PgConnection) {
    rust_pg_register_connection(conn);
}

#[no_mangle]
pub extern "C" fn pg_unregister_connection(conn: *mut PgConnection) {
    rust_pg_unregister_connection(conn);
}

#[no_mangle]
pub extern "C" fn pg_find_connection(db: *mut sqlite3) -> *mut PgConnection {
    rust_pg_find_connection(db)
}

#[no_mangle]
pub extern "C" fn pg_find_handle_connection(db: *mut sqlite3) -> *mut PgConnection {
    rust_pg_find_handle_connection(db)
}

#[no_mangle]
pub extern "C" fn pg_find_any_library_connection() -> *mut PgConnection {
    rust_pg_find_any_library_connection()
}

#[no_mangle]
pub extern "C" fn pg_get_thread_connection(db_path: *const c_char) -> *mut PgConnection {
    unsafe { rust_pool_get_connection(db_path) as *mut PgConnection }
}

#[no_mangle]
pub extern "C" fn pg_get_thread_connection_excluding(
    db_path: *const c_char,
    exclude_conn: *const c_void,
) -> *mut PgConnection {
    unsafe { rust_pool_get_connection_excluding(db_path, exclude_conn) as *mut PgConnection }
}

#[no_mangle]
pub extern "C" fn pg_pool_validate_connection(conn: *mut PgConnection) -> c_int {
    rust_pool_validate_connection(conn as *const c_void)
}

#[no_mangle]
pub extern "C" fn pg_pool_touch_connection(conn: *mut PgConnection) {
    rust_pool_touch_connection(conn as *const c_void);
}

#[no_mangle]
pub extern "C" fn pg_pool_clear_streaming_active(conn: *mut PgConnection) -> c_int {
    rust_pool_clear_streaming_active(conn as *const c_void)
}

#[no_mangle]
pub extern "C" fn pg_pool_is_live_connection(conn: *mut PgConnection) -> c_int {
    rust_pool_is_live_connection(conn as *const c_void)
}

#[no_mangle]
pub extern "C" fn pg_pool_is_tracked_connection(conn: *mut PgConnection) -> c_int {
    rust_pool_is_tracked_connection(conn as *const c_void)
}

#[no_mangle]
pub extern "C" fn pg_pool_check_connection_health(conn: *mut PgConnection) -> c_int {
    rust_pool_check_health(conn as *mut c_void)
}

#[no_mangle]
pub extern "C" fn pg_close_pool_for_db(db: *mut sqlite3) {
    if db.is_null() {
        return;
    }
    rust_pool_release_for_db(db as *const c_void);
}

#[no_mangle]
pub extern "C" fn pg_get_global_metadata_id() -> i64 {
    rust_get_global_metadata_id()
}

#[no_mangle]
pub extern "C" fn pg_set_global_metadata_id(id: i64) {
    rust_set_global_metadata_id(id);
}

#[no_mangle]
pub extern "C" fn pg_get_global_last_insert_rowid() -> i64 {
    rust_get_global_last_insert_rowid()
}

#[no_mangle]
pub extern "C" fn pg_set_global_last_insert_rowid(id: i64) {
    rust_set_global_last_insert_rowid(id);
}

#[no_mangle]
pub extern "C" fn pg_hash_sql(sql: *const c_char) -> u64 {
    rust_hash_sql(sql)
}

#[no_mangle]
pub extern "C" fn pg_stmt_cache_lookup(
    conn: *mut PgConnection,
    sql_hash: u64,
    stmt_name_out: *mut *const c_char,
) -> c_int {
    rust_stmt_cache_lookup(conn as *mut c_void, sql_hash, stmt_name_out)
}

#[no_mangle]
pub extern "C" fn pg_stmt_cache_add(
    conn: *mut PgConnection,
    sql_hash: u64,
    stmt_name: *const c_char,
    param_count: c_int,
) -> c_int {
    rust_stmt_cache_add(conn as *mut c_void, sql_hash, stmt_name, param_count)
}

#[no_mangle]
pub extern "C" fn pg_stmt_cache_clear(conn: *mut PgConnection) {
    rust_stmt_cache_clear(conn as *mut c_void);
}

#[no_mangle]
pub extern "C" fn pg_stmt_cache_clear_local(conn: *mut PgConnection) {
    rust_stmt_cache_clear_local(conn as *mut c_void);
}

#[no_mangle]
pub extern "C" fn pg_is_stale_prepared_stmt(res: *mut PGresult) -> c_int {
    if res.is_null() {
        return 0;
    }
    let sqlstate = rust_pq_result_error_field(res, PG_DIAG_SQLSTATE);
    rust_is_stale_sqlstate(sqlstate)
}

#[no_mangle]
pub extern "C" fn pg_is_duplicate_prepared_stmt(res: *mut PGresult) -> c_int {
    if res.is_null() {
        return 0;
    }
    let sqlstate = rust_pq_result_error_field(res, PG_DIAG_SQLSTATE);
    rust_is_duplicate_sqlstate(sqlstate)
}

#[no_mangle]
pub extern "C" fn pg_pool_cleanup_after_fork() {
    rust_pool_cleanup_after_fork();
}
