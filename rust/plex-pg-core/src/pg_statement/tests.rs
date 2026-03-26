use super::*;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::atomic::Ordering;

fn cs(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn make_stmt(sql: &str) -> *mut PgStmt {
    let csql = CString::new(sql).unwrap();
    let stmt = rust_stmt_create(std::ptr::null_mut(), csql.as_ptr(), std::ptr::null_mut());
    assert!(!stmt.is_null());
    stmt
}

fn ref_count(stmt: *mut PgStmt) -> i32 {
    unsafe { (*stmt).ref_count.load(Ordering::Relaxed) }
}

#[path = "tests/lifecycle.rs"]
mod lifecycle;
#[path = "tests/metadata.rs"]
mod metadata;
#[path = "tests/registry.rs"]
mod registry;
#[path = "tests/tls_cache.rs"]
mod tls_cache;
#[path = "tests/value_pool.rs"]
mod value_pool;
