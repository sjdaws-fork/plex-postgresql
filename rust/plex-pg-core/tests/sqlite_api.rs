use rusqlite::ffi;
use rusqlite::{Connection, Result};
use std::ffi::{CStr, CString};
use std::ptr;

/// Helper: prepare via FFI and return raw stmt pointer (caller must finalize).
unsafe fn prepare_raw(conn: &Connection, sql: &str) -> *mut ffi::sqlite3_stmt {
    let c_sql = CString::new(sql).unwrap();
    let mut raw: *mut ffi::sqlite3_stmt = ptr::null_mut();
    let mut tail: *const std::os::raw::c_char = ptr::null();
    let rc = ffi::sqlite3_prepare_v2(conn.handle(), c_sql.as_ptr(), -1, &mut raw, &mut tail);
    assert_eq!(rc, ffi::SQLITE_OK);
    raw
}

#[test]
fn sqlite3_free_null_is_ok() {
    unsafe {
        ffi::sqlite3_free(std::ptr::null_mut());
    }
}

#[test]
fn sqlite3_free_allocated_is_ok() {
    unsafe {
        let ptr = ffi::sqlite3_malloc(100);
        assert!(!ptr.is_null());
        ffi::sqlite3_free(ptr);
    }
}

#[test]
fn sqlite3_db_handle_returns_parent_db() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let raw_stmt = unsafe { prepare_raw(&conn, "SELECT 1") };
    let db_handle = unsafe { ffi::sqlite3_db_handle(raw_stmt) };
    assert_eq!(db_handle, unsafe { conn.handle() });
    unsafe { ffi::sqlite3_finalize(raw_stmt) };
    Ok(())
}

#[test]
fn sqlite3_db_handle_null_is_null() {
    let handle = unsafe { ffi::sqlite3_db_handle(std::ptr::null_mut()) };
    assert!(handle.is_null());
}

#[test]
fn sqlite3_sql_returns_statement_sql() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let raw_stmt = unsafe { prepare_raw(&conn, "SELECT * FROM sqlite_master") };
    let sql_ptr = unsafe { ffi::sqlite3_sql(raw_stmt) };
    assert!(!sql_ptr.is_null());
    let sql = unsafe { CStr::from_ptr(sql_ptr) }.to_string_lossy();
    assert!(sql.to_ascii_lowercase().contains("select"));
    unsafe { ffi::sqlite3_finalize(raw_stmt) };
    Ok(())
}

#[test]
fn sqlite3_sql_null_is_null() {
    let sql_ptr = unsafe { ffi::sqlite3_sql(std::ptr::null_mut()) };
    assert!(sql_ptr.is_null());
}

#[test]
fn sqlite3_bind_parameter_count_none() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let raw_stmt = unsafe { prepare_raw(&conn, "SELECT 1") };
    let count = unsafe { ffi::sqlite3_bind_parameter_count(raw_stmt) };
    assert_eq!(count, 0);
    unsafe { ffi::sqlite3_finalize(raw_stmt) };
    Ok(())
}

#[test]
fn sqlite3_bind_parameter_count_multiple() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let raw_stmt = unsafe { prepare_raw(&conn, "SELECT ? + ? + ?") };
    let count = unsafe { ffi::sqlite3_bind_parameter_count(raw_stmt) };
    assert_eq!(count, 3);
    unsafe { ffi::sqlite3_finalize(raw_stmt) };
    Ok(())
}

#[test]
fn sqlite3_bind_parameter_count_null_stmt_is_zero() {
    let count = unsafe { ffi::sqlite3_bind_parameter_count(std::ptr::null_mut()) };
    assert_eq!(count, 0);
}

#[test]
fn sqlite3_stmt_readonly_select_is_true() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let raw_stmt = unsafe { prepare_raw(&conn, "SELECT 1") };
    let readonly = unsafe { ffi::sqlite3_stmt_readonly(raw_stmt) };
    assert_ne!(readonly, 0);
    unsafe { ffi::sqlite3_finalize(raw_stmt) };
    Ok(())
}

#[test]
fn sqlite3_stmt_readonly_insert_is_false() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute("CREATE TABLE t(id INTEGER)", [])?;
    let raw_stmt = unsafe { prepare_raw(&conn, "INSERT INTO t VALUES (1)") };
    let readonly = unsafe { ffi::sqlite3_stmt_readonly(raw_stmt) };
    assert_eq!(readonly, 0);
    unsafe { ffi::sqlite3_finalize(raw_stmt) };
    Ok(())
}

#[test]
fn prepare_v3_with_persistent_flag_works() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute("CREATE TABLE test (id INTEGER, name TEXT)", [])?;
    let sql = CString::new("INSERT INTO test VALUES (?, ?)").unwrap();

    let mut stmt: *mut ffi::sqlite3_stmt = std::ptr::null_mut();
    let mut tail: *const std::os::raw::c_char = std::ptr::null();
    let rc = unsafe {
        ffi::sqlite3_prepare_v3(
            conn.handle(),
            sql.as_ptr(),
            -1,
            ffi::SQLITE_PREPARE_PERSISTENT,
            &mut stmt,
            &mut tail,
        )
    };
    assert_eq!(rc, ffi::SQLITE_OK);
    if !stmt.is_null() {
        unsafe { ffi::sqlite3_finalize(stmt) };
    }
    Ok(())
}

#[test]
fn drop_index_if_exists_is_safe() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let mut err: *mut std::os::raw::c_char = std::ptr::null_mut();
    let sql = CString::new("DROP INDEX IF EXISTS index_title_sort_icu").unwrap();
    let rc = unsafe {
        ffi::sqlite3_exec(
            conn.handle(),
            sql.as_ptr(),
            None,
            std::ptr::null_mut(),
            &mut err,
        )
    };
    if !err.is_null() {
        unsafe { ffi::sqlite3_free(err as *mut std::os::raw::c_void) };
    }
    assert_eq!(rc, ffi::SQLITE_OK);
    Ok(())
}
