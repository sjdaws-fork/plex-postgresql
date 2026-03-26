use std::os::raw::{c_char, c_int};

use crate::ffi_types::{sqlite3_stmt, PgConnection, PgStmt};

#[no_mangle]
pub extern "C" fn step_conn_cancel_and_drain(conn: *mut PgConnection, scope_tag: *const c_char) {
    crate::db_interpose_conn_utils::rust_step_conn_cancel_and_drain(conn, scope_tag);
}

#[no_mangle]
pub extern "C" fn step_pick_thread_connection(base_conn: *mut PgConnection) -> *mut PgConnection {
    crate::db_interpose_step_write_utils::rust_step_pick_thread_connection(base_conn)
}

#[no_mangle]
pub extern "C" fn step_cached_write_should_noop(
    base_conn: *mut PgConnection,
    sql: *const c_char,
    out_exec_conn: *mut *mut PgConnection,
) -> c_int {
    crate::db_interpose_step_write_utils::rust_step_cached_write_should_noop(
        base_conn,
        sql,
        out_exec_conn,
    )
}

#[no_mangle]
pub extern "C" fn step_pg_write_should_noop(
    exec_conn: *mut PgConnection,
    pg_sql: *const c_char,
    txn_state_out: *mut c_int,
) -> c_int {
    crate::db_interpose_step_write_utils::rust_step_pg_write_should_noop(
        exec_conn,
        pg_sql,
        txn_state_out,
    )
}

#[no_mangle]
pub extern "C" fn step_cached_write_build_exec_sql(
    orig_sql: *const c_char,
    translated_sql: *const c_char,
    exec_sql_out: *mut *const c_char,
) -> *mut c_char {
    crate::db_interpose_step_write_utils::rust_step_cached_write_build_exec_sql(
        orig_sql,
        translated_sql,
        exec_sql_out,
    )
}

#[no_mangle]
pub extern "C" fn step_write_should_skip_special_insert(
    pg_stmt: *mut PgStmt,
    exec_conn: *mut PgConnection,
    param_values: *const *const c_char,
) -> c_int {
    crate::db_interpose_step_write_utils::rust_step_write_should_skip_special_insert(
        pg_stmt,
        exec_conn,
        param_values,
    )
}

#[no_mangle]
pub extern "C" fn step_write_prepare_connection(
    pg_stmt: *mut PgStmt,
    exec_conn_io: *mut *mut PgConnection,
    pg_conn_error_out: *mut c_int,
) -> c_int {
    crate::db_interpose_step_write_utils::rust_step_write_prepare_connection(
        pg_stmt,
        exec_conn_io,
        pg_conn_error_out,
    )
}

#[no_mangle]
pub extern "C" fn step_write_execute_and_finalize(
    pg_stmt: *mut PgStmt,
    exec_conn: *mut PgConnection,
    param_values: *const *const c_char,
    pg_conn_error_out: *mut c_int,
) -> c_int {
    crate::db_interpose_step_write_utils::rust_step_write_execute_and_finalize(
        pg_stmt,
        exec_conn,
        param_values,
        pg_conn_error_out,
    )
}

#[no_mangle]
pub extern "C" fn step_cached_write_execute_and_finalize(
    cached_io: *mut *mut PgStmt,
    p_stmt: *mut sqlite3_stmt,
    changes_conn: *mut PgConnection,
    exec_conn: *mut PgConnection,
    orig_sql: *const c_char,
    exec_sql: *const c_char,
    pg_conn_error_out: *mut c_int,
) -> c_int {
    crate::db_interpose_step_write_utils::rust_step_cached_write_execute_and_finalize(
        cached_io,
        p_stmt,
        changes_conn,
        exec_conn,
        orig_sql,
        exec_sql,
        pg_conn_error_out,
    )
}

#[no_mangle]
pub extern "C" fn step_write_log_debug_context(
    pg_stmt: *mut PgStmt,
    exec_conn: *mut PgConnection,
    param_values: *const *const c_char,
) {
    crate::db_interpose_step_write_utils::rust_step_write_log_debug_context(
        pg_stmt,
        exec_conn,
        param_values,
    );
}

#[no_mangle]
pub extern "C" fn step_log_step_exit_trace(pg_stmt: *mut PgStmt) {
    crate::db_interpose_step_write_utils::rust_step_log_step_exit_trace(pg_stmt);
}

#[no_mangle]
pub extern "C" fn step_cached_read_finalize_advance(
    cached: *mut PgStmt,
    expanded_sql: *mut c_char,
    step_rc_out: *mut c_int,
) -> c_int {
    crate::db_interpose_step_cached_read_utils::rust_step_cached_read_finalize_advance(
        cached,
        expanded_sql,
        step_rc_out,
    )
}

#[no_mangle]
pub extern "C" fn step_cached_read_prepare_stmt(
    cached: *mut PgStmt,
    conn: *mut PgConnection,
    sql: *const c_char,
    p_stmt: *mut sqlite3_stmt,
    translated_sql: *const c_char,
) -> *mut PgStmt {
    crate::db_interpose_step_cached_read_utils::rust_step_cached_read_prepare_stmt(
        cached,
        conn,
        sql,
        p_stmt,
        translated_sql,
    )
}

#[no_mangle]
pub extern "C" fn step_cached_read_execute(
    stmt: *mut PgStmt,
    conn: *mut PgConnection,
    orig_sql: *const c_char,
    translated_sql: *const c_char,
    pg_conn_error_out: *mut c_int,
) -> c_int {
    crate::db_interpose_step_cached_read_utils::rust_step_cached_read_execute(
        stmt,
        conn,
        orig_sql,
        translated_sql,
        pg_conn_error_out,
    )
}

#[no_mangle]
pub extern "C" fn step_read_advance_cached_result(stmt: *mut PgStmt) -> c_int {
    crate::db_interpose_step_read_utils::rust_step_read_advance_cached_result(stmt)
}

#[no_mangle]
pub extern "C" fn step_read_streaming_next(p_stmt: *mut sqlite3_stmt, stmt: *mut PgStmt) -> c_int {
    crate::db_interpose_step_read_utils::rust_step_read_streaming_next(p_stmt, stmt)
}

#[no_mangle]
pub extern "C" fn step_read_eager_next(stmt: *mut PgStmt) -> c_int {
    crate::db_interpose_step_read_utils::rust_step_read_eager_next(stmt)
}

#[no_mangle]
pub extern "C" fn step_read_first_execute(
    stmt: *mut PgStmt,
    exec_conn_io: *mut *mut PgConnection,
    param_values: *const *const c_char,
    pg_conn_error_out: *mut c_int,
) -> c_int {
    crate::db_interpose_step_read_utils::rust_step_read_first_execute(
        stmt,
        exec_conn_io,
        param_values,
        pg_conn_error_out,
    )
}

#[no_mangle]
pub extern "C" fn step_read_log_debug_context(stmt: *mut PgStmt, exec_conn: *mut PgConnection) {
    crate::db_interpose_step_read_utils::rust_step_read_log_debug_context(stmt, exec_conn);
}

#[no_mangle]
pub extern "C" fn step_read_prepare_reexecution_state(
    stmt: *mut PgStmt,
    exec_conn: *mut PgConnection,
) {
    crate::db_interpose_step_read_utils::rust_step_read_prepare_reexecution_state(stmt, exec_conn);
}
