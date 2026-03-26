use std::os::raw::{c_char, c_int, c_void};

use crate::ffi_types::{sqlite3_stmt, PgConnection, PgStmt};

#[no_mangle]
pub extern "C" fn resolve_column_tables(pg_stmt: *mut PgStmt, pg_conn: *mut PgConnection) -> c_int {
    crate::db_interpose_column::rust_resolve_column_tables(pg_stmt, pg_conn)
}

#[no_mangle]
pub extern "C" fn pg_decode_bytea(
    pg_stmt: *mut PgStmt,
    row: c_int,
    col: c_int,
    out_length: *mut c_int,
) -> *const c_void {
    crate::db_interpose_column::rust_pg_decode_bytea_cached(pg_stmt, row, col, out_length)
}

#[no_mangle]
pub extern "C" fn pg_note_stmt_prepare(p_stmt: *mut sqlite3_stmt, sql: *const c_char) {
    crate::db_interpose_stmt_lifecycle::rust_pg_note_stmt_prepare(p_stmt, sql);
}

#[no_mangle]
pub extern "C" fn skip_leading_sql_noise(sql: *const c_char) -> *const c_char {
    crate::db_interpose_txn_utils::rust_skip_leading_sql_noise(sql)
}

#[no_mangle]
pub extern "C" fn is_txn_terminator_sql(sql: *const c_char) -> c_int {
    crate::db_interpose_txn_utils::rust_is_txn_terminator_sql(sql)
}

#[no_mangle]
pub extern "C" fn txn_terminator_should_noop(
    conn: *mut PgConnection,
    sql: *const c_char,
    txn_state_out: *mut c_int,
) -> c_int {
    crate::db_interpose_txn_utils::rust_txn_terminator_should_noop(conn, sql, txn_state_out)
}
