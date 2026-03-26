use std::os::raw::{c_char, c_int, c_void};

use crate::ffi_types::{sqlite3_stmt, sqlite3_value, PgConnection, PgStmt};

use super::{
    rust_cached_stmt_clear, rust_cached_stmt_clear_weak, rust_cached_stmt_find,
    rust_cached_stmt_register, rust_create_column_value, rust_decltype_special_case,
    rust_is_our_value, rust_oid_to_sqlite_decltype, rust_oid_to_sqlite_type,
    rust_stmt_clear_result, rust_stmt_create, rust_stmt_find, rust_stmt_find_any,
    rust_stmt_is_ours, rust_stmt_ref, rust_stmt_register, rust_stmt_registry_cleanup,
    rust_stmt_registry_init, rust_stmt_unref, rust_stmt_unregister, PgValue, SQLITE_NULL,
};

#[no_mangle]
pub extern "C" fn pg_statement_init() {
    rust_stmt_registry_init();
}

#[no_mangle]
pub extern "C" fn pg_statement_cleanup() {
    rust_stmt_registry_cleanup();
}

#[no_mangle]
pub extern "C" fn pg_register_stmt(sqlite_stmt: *mut sqlite3_stmt, pg_stmt: *mut PgStmt) {
    rust_stmt_register(sqlite_stmt as usize, pg_stmt as usize);
}

#[no_mangle]
pub extern "C" fn pg_unregister_stmt(sqlite_stmt: *mut sqlite3_stmt) {
    rust_stmt_unregister(sqlite_stmt as usize);
}

#[no_mangle]
pub extern "C" fn pg_find_stmt(stmt: *mut sqlite3_stmt) -> *mut PgStmt {
    rust_stmt_find(stmt as usize) as *mut PgStmt
}

#[no_mangle]
pub extern "C" fn pg_find_any_stmt(stmt: *mut sqlite3_stmt) -> *mut PgStmt {
    rust_stmt_find_any(stmt as usize) as *mut PgStmt
}

#[no_mangle]
pub extern "C" fn pg_is_our_stmt(ptr: *mut c_void) -> c_int {
    rust_stmt_is_ours(ptr as usize)
}

#[no_mangle]
pub extern "C" fn pg_register_cached_stmt(sqlite_stmt: *mut sqlite3_stmt, pg_stmt: *mut PgStmt) {
    rust_cached_stmt_register(sqlite_stmt as usize, pg_stmt as usize);
}

#[no_mangle]
pub extern "C" fn pg_find_cached_stmt(sqlite_stmt: *mut sqlite3_stmt) -> *mut PgStmt {
    rust_cached_stmt_find(sqlite_stmt as usize) as *mut PgStmt
}

#[no_mangle]
pub extern "C" fn pg_clear_cached_stmt(sqlite_stmt: *mut sqlite3_stmt) {
    rust_cached_stmt_clear(sqlite_stmt as usize);
}

#[no_mangle]
pub extern "C" fn pg_clear_cached_stmt_weak(sqlite_stmt: *mut sqlite3_stmt) {
    rust_cached_stmt_clear_weak(sqlite_stmt as usize);
}

#[no_mangle]
pub extern "C" fn pg_stmt_create(
    conn: *mut PgConnection,
    sql: *const c_char,
    shadow_stmt: *mut sqlite3_stmt,
) -> *mut PgStmt {
    rust_stmt_create(conn, sql, shadow_stmt)
}

#[no_mangle]
pub extern "C" fn pg_stmt_free(stmt: *mut PgStmt) {
    super::rust_stmt_free(stmt);
}

#[no_mangle]
pub extern "C" fn pg_stmt_ref(stmt: *mut PgStmt) {
    rust_stmt_ref(stmt);
}

#[no_mangle]
pub extern "C" fn pg_stmt_unref(stmt: *mut PgStmt) {
    rust_stmt_unref(stmt);
}

#[no_mangle]
pub extern "C" fn pg_stmt_clear_result(stmt: *mut PgStmt) {
    rust_stmt_clear_result(stmt);
}

#[no_mangle]
pub extern "C" fn pg_oid_to_sqlite_type(oid: u32) -> c_int {
    rust_oid_to_sqlite_type(oid)
}

#[no_mangle]
pub extern "C" fn pg_oid_to_sqlite_decltype(oid: u32) -> *const c_char {
    rust_oid_to_sqlite_decltype(oid)
}

#[no_mangle]
pub extern "C" fn pg_decltype_special_case(
    oid: u32,
    col_name: *const c_char,
    pg_sql: *const c_char,
    table_oid: u32,
) -> c_int {
    rust_decltype_special_case(oid, col_name, pg_sql, table_oid)
}

#[no_mangle]
pub extern "C" fn pg_create_column_value(
    pg_stmt: *mut PgStmt,
    col_idx: c_int,
) -> *mut sqlite3_value {
    if pg_stmt.is_null() || unsafe { (*pg_stmt).result.is_null() } {
        return rust_create_column_value(pg_stmt as usize, col_idx, SQLITE_NULL)
            as *mut sqlite3_value;
    }
    let sqlite_type = unsafe {
        crate::db_interpose_helpers::rust_pg_create_column_value(
            (*pg_stmt).result,
            (*pg_stmt).current_row,
            (*pg_stmt).num_rows,
            col_idx,
        )
    };
    rust_create_column_value(pg_stmt as usize, col_idx, sqlite_type) as *mut sqlite3_value
}

#[no_mangle]
pub extern "C" fn pg_is_our_value(val: *mut sqlite3_value) -> c_int {
    rust_is_our_value(val as *const PgValue)
}
