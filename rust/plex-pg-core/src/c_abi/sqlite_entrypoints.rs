use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use crate::ffi_types::{sqlite3, sqlite3_stmt, sqlite3_value};

type CollationCompare =
    Option<unsafe extern "C" fn(*mut c_void, c_int, *const c_void, c_int, *const c_void) -> c_int>;
type CollationDestroy = Option<unsafe extern "C" fn(*mut c_void)>;

#[no_mangle]
pub extern "C" fn my_sqlite3_open(filename: *const c_char, pp_db: *mut *mut sqlite3) -> c_int {
    crate::db_interpose_open::rust_my_sqlite3_open(filename, pp_db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_open_v2(
    filename: *const c_char,
    pp_db: *mut *mut sqlite3,
    flags: c_int,
    z_vfs: *const c_char,
) -> c_int {
    crate::db_interpose_open::rust_my_sqlite3_open_v2(filename, pp_db, flags, z_vfs)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_close(db: *mut sqlite3) -> c_int {
    crate::db_interpose_open::rust_my_sqlite3_close(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_close_v2(db: *mut sqlite3) -> c_int {
    crate::db_interpose_open::rust_my_sqlite3_close_v2(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_exec(
    db: *mut sqlite3,
    sql: *const c_char,
    callback: Option<
        unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
    >,
    arg: *mut c_void,
    errmsg: *mut *mut c_char,
) -> c_int {
    crate::db_interpose_exec::rust_my_sqlite3_exec(db, sql, callback, arg, errmsg)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_prepare_v2_internal(
    db: *mut sqlite3,
    z_sql: *const c_char,
    n_byte: c_int,
    pp_stmt: *mut *mut sqlite3_stmt,
    pz_tail: *mut *const c_char,
    from_worker: c_int,
) -> c_int {
    crate::db_interpose_prepare::rust_my_sqlite3_prepare_v2_internal(
        db,
        z_sql,
        n_byte,
        pp_stmt,
        pz_tail,
        from_worker,
    )
}

#[no_mangle]
pub extern "C" fn my_sqlite3_prepare(
    db: *mut sqlite3,
    z_sql: *const c_char,
    n_byte: c_int,
    pp_stmt: *mut *mut sqlite3_stmt,
    pz_tail: *mut *const c_char,
) -> c_int {
    crate::db_interpose_prepare::rust_my_sqlite3_prepare(db, z_sql, n_byte, pp_stmt, pz_tail)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_prepare_v2(
    db: *mut sqlite3,
    z_sql: *const c_char,
    n_byte: c_int,
    pp_stmt: *mut *mut sqlite3_stmt,
    pz_tail: *mut *const c_char,
) -> c_int {
    crate::db_interpose_prepare::rust_my_sqlite3_prepare_v2(db, z_sql, n_byte, pp_stmt, pz_tail)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_prepare_v3(
    db: *mut sqlite3,
    z_sql: *const c_char,
    n_byte: c_int,
    prep_flags: c_uint,
    pp_stmt: *mut *mut sqlite3_stmt,
    pz_tail: *mut *const c_char,
) -> c_int {
    crate::db_interpose_prepare::rust_my_sqlite3_prepare_v3(
        db, z_sql, n_byte, prep_flags, pp_stmt, pz_tail,
    )
}

#[no_mangle]
pub extern "C" fn my_sqlite3_prepare16_v2(
    db: *mut sqlite3,
    z_sql: *const c_void,
    n_byte: c_int,
    pp_stmt: *mut *mut sqlite3_stmt,
    pz_tail: *mut *const c_void,
) -> c_int {
    crate::db_interpose_prepare::rust_my_sqlite3_prepare16_v2(db, z_sql, n_byte, pp_stmt, pz_tail)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_int(p_stmt: *mut sqlite3_stmt, idx: c_int, val: c_int) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_int(p_stmt, idx, val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_int64(p_stmt: *mut sqlite3_stmt, idx: c_int, val: i64) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_int64(p_stmt, idx, val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_double(p_stmt: *mut sqlite3_stmt, idx: c_int, val: f64) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_double(p_stmt, idx, val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_text(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
    val: *const c_char,
    n_bytes: c_int,
    destructor: *mut c_void,
) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_text(p_stmt, idx, val, n_bytes, destructor)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_text64(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
    val: *const c_char,
    n_bytes: u64,
    destructor: *mut c_void,
    encoding: c_uchar,
) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_text64(
        p_stmt, idx, val, n_bytes, destructor, encoding,
    )
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_blob(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
    val: *const c_void,
    n_bytes: c_int,
    destructor: *mut c_void,
) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_blob(p_stmt, idx, val, n_bytes, destructor)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_blob64(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
    val: *const c_void,
    n_bytes: u64,
    destructor: *mut c_void,
) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_blob64(p_stmt, idx, val, n_bytes, destructor)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_value(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
    value: *const sqlite3_value,
) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_value(p_stmt, idx, value)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_null(p_stmt: *mut sqlite3_stmt, idx: c_int) -> c_int {
    crate::db_interpose_bind::rust_my_sqlite3_bind_null(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_step(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_step::rust_my_sqlite3_step(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_reset(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_stmt_lifecycle::rust_my_sqlite3_reset(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_finalize(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_stmt_lifecycle::rust_my_sqlite3_finalize(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_clear_bindings(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_stmt_lifecycle::rust_my_sqlite3_clear_bindings(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_count(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_column::rust_my_sqlite3_column_count(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_type(p_stmt: *mut sqlite3_stmt, idx: c_int) -> c_int {
    crate::db_interpose_column::rust_my_sqlite3_column_type(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_int(p_stmt: *mut sqlite3_stmt, idx: c_int) -> c_int {
    crate::db_interpose_column::rust_my_sqlite3_column_int(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_int64(p_stmt: *mut sqlite3_stmt, idx: c_int) -> i64 {
    crate::db_interpose_column::rust_my_sqlite3_column_int64(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_double(p_stmt: *mut sqlite3_stmt, idx: c_int) -> f64 {
    crate::db_interpose_column::rust_my_sqlite3_column_double(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_text(p_stmt: *mut sqlite3_stmt, idx: c_int) -> *const c_uchar {
    crate::db_interpose_column::rust_my_sqlite3_column_text(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_blob(p_stmt: *mut sqlite3_stmt, idx: c_int) -> *const c_void {
    crate::db_interpose_column::rust_my_sqlite3_column_blob(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_bytes(p_stmt: *mut sqlite3_stmt, idx: c_int) -> c_int {
    crate::db_interpose_column::rust_my_sqlite3_column_bytes(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_name(p_stmt: *mut sqlite3_stmt, idx: c_int) -> *const c_char {
    crate::db_interpose_column::rust_my_sqlite3_column_name(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_decltype(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
) -> *const c_char {
    crate::db_interpose_column::rust_my_sqlite3_column_decltype(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_column_value(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
) -> *mut sqlite3_value {
    crate::db_interpose_column::rust_my_sqlite3_column_value(p_stmt, idx)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_data_count(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_column::rust_my_sqlite3_data_count(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_value_type(p_val: *mut sqlite3_value) -> c_int {
    crate::db_interpose_value::rust_my_sqlite3_value_type(p_val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_value_text(p_val: *mut sqlite3_value) -> *const c_uchar {
    crate::db_interpose_value::rust_my_sqlite3_value_text(p_val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_value_int(p_val: *mut sqlite3_value) -> c_int {
    crate::db_interpose_value::rust_my_sqlite3_value_int(p_val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_value_int64(p_val: *mut sqlite3_value) -> i64 {
    crate::db_interpose_value::rust_my_sqlite3_value_int64(p_val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_value_double(p_val: *mut sqlite3_value) -> f64 {
    crate::db_interpose_value::rust_my_sqlite3_value_double(p_val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_value_bytes(p_val: *mut sqlite3_value) -> c_int {
    crate::db_interpose_value::rust_my_sqlite3_value_bytes(p_val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_value_blob(p_val: *mut sqlite3_value) -> *const c_void {
    crate::db_interpose_value::rust_my_sqlite3_value_blob(p_val)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_changes(db: *mut sqlite3) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_changes(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_changes64(db: *mut sqlite3) -> i64 {
    crate::db_interpose_metadata::rust_my_sqlite3_changes64(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_last_insert_rowid(db: *mut sqlite3) -> i64 {
    crate::db_interpose_metadata::rust_my_sqlite3_last_insert_rowid(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_errmsg(db: *mut sqlite3) -> *const c_char {
    crate::db_interpose_metadata::rust_my_sqlite3_errmsg(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_errcode(db: *mut sqlite3) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_errcode(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_extended_errcode(db: *mut sqlite3) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_extended_errcode(db)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_get_table(
    db: *mut sqlite3,
    sql: *const c_char,
    paz_result: *mut *mut *mut c_char,
    pn_row: *mut c_int,
    pn_col: *mut c_int,
    pz_err_msg: *mut *mut c_char,
) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_get_table(
        db, sql, paz_result, pn_row, pn_col, pz_err_msg,
    )
}

#[no_mangle]
pub extern "C" fn my_sqlite3_create_collation(
    db: *mut sqlite3,
    name: *const c_char,
    text_rep: c_int,
    arg: *mut c_void,
    compare: CollationCompare,
) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_create_collation(db, name, text_rep, arg, compare)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_create_collation_v2(
    db: *mut sqlite3,
    name: *const c_char,
    text_rep: c_int,
    arg: *mut c_void,
    compare: CollationCompare,
    destroy: CollationDestroy,
) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_create_collation_v2(
        db, name, text_rep, arg, compare, destroy,
    )
}

#[no_mangle]
pub extern "C" fn my_sqlite3_free(ptr: *mut c_void) {
    crate::db_interpose_metadata::rust_my_sqlite3_free(ptr);
}

#[no_mangle]
pub extern "C" fn my_sqlite3_malloc(n: c_int) -> *mut c_void {
    crate::db_interpose_metadata::rust_my_sqlite3_malloc(n)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_db_handle(p_stmt: *mut sqlite3_stmt) -> *mut sqlite3 {
    crate::db_interpose_metadata::rust_my_sqlite3_db_handle(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_sql(p_stmt: *mut sqlite3_stmt) -> *const c_char {
    crate::db_interpose_metadata::rust_my_sqlite3_sql(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_expanded_sql(p_stmt: *mut sqlite3_stmt) -> *mut c_char {
    crate::db_interpose_metadata::rust_my_sqlite3_expanded_sql(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_parameter_count(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_bind_parameter_count(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_parameter_index(
    p_stmt: *mut sqlite3_stmt,
    name: *const c_char,
) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_bind_parameter_index(p_stmt, name)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_stmt_readonly(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_stmt_readonly(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_stmt_busy(p_stmt: *mut sqlite3_stmt) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_stmt_busy(p_stmt)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_stmt_status(
    p_stmt: *mut sqlite3_stmt,
    op: c_int,
    reset_flag: c_int,
) -> c_int {
    crate::db_interpose_metadata::rust_my_sqlite3_stmt_status(p_stmt, op, reset_flag)
}

#[no_mangle]
pub extern "C" fn my_sqlite3_bind_parameter_name(
    p_stmt: *mut sqlite3_stmt,
    idx: c_int,
) -> *const c_char {
    crate::db_interpose_metadata::rust_my_sqlite3_bind_parameter_name(p_stmt, idx)
}
