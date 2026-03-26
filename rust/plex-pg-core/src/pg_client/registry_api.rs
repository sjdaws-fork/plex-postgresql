use std::ffi::CString;
use std::os::raw::c_void;

use crate::ffi_types::{sqlite3, PgConnection};

use super::config::env_nonzero;
use super::connection_helpers::{conn_db_path, conn_is_pg_active};
use super::pool_lookup::{is_library_db, pool_find_connection_for_db};
use super::{log_debug, pool, rust_find_any_library_connection, rust_find_registered_connection};

/// Register a non-pooled connection using its shadow DB handle.
#[no_mangle]
pub extern "C" fn rust_pg_register_connection(conn: *mut PgConnection) {
    if conn.is_null() {
        return;
    }
    unsafe {
        let db = (*conn).shadow_db;
        if db.is_null() {
            return;
        }
        pool().registry.register(db as usize, conn as usize);
    }
}

/// Unregister a non-pooled connection using its shadow DB handle.
#[no_mangle]
pub extern "C" fn rust_pg_unregister_connection(conn: *mut PgConnection) {
    if conn.is_null() {
        return;
    }
    unsafe {
        let db = (*conn).shadow_db;
        if db.is_null() {
            return;
        }
        pool().registry.unregister(db as usize);
    }
}

/// Find the handle connection for a sqlite3* handle.
#[no_mangle]
pub extern "C" fn rust_pg_find_handle_connection(db_handle: *const sqlite3) -> *mut PgConnection {
    if db_handle.is_null() {
        return std::ptr::null_mut();
    }
    rust_find_registered_connection(db_handle as *const c_void) as *mut PgConnection
}

/// Find the active connection for a sqlite3* handle, including pool logic.
#[no_mangle]
pub extern "C" fn rust_pg_find_connection(db_handle: *const sqlite3) -> *mut PgConnection {
    if db_handle.is_null() {
        return std::ptr::null_mut();
    }

    let _ = super::rust_pool_check_fork();

    let handle_conn =
        rust_find_registered_connection(db_handle as *const c_void) as *mut PgConnection;
    if handle_conn.is_null() {
        return std::ptr::null_mut();
    }

    let path = conn_db_path(handle_conn);

    if is_library_db(&path) {
        if env_nonzero("PLEX_PG_FORCE_SQLITE_LIBRARY") {
            return std::ptr::null_mut();
        }

        if env_nonzero("PLEX_PG_DISABLE_POOL") {
            if conn_is_pg_active(handle_conn) {
                return handle_conn;
            }
            return std::ptr::null_mut();
        }

        if let Ok(cs) = CString::new(path) {
            let pool_conn = pool_find_connection_for_db(db_handle as usize, cs.as_ptr());
            if !pool_conn.is_null() && conn_is_pg_active(pool_conn as *mut PgConnection) {
                return pool_conn as *mut PgConnection;
            }
        }

        log_debug("Pool full for library.db, falling back to SQLite");
        return std::ptr::null_mut();
    }

    if conn_is_pg_active(handle_conn) {
        handle_conn
    } else {
        std::ptr::null_mut()
    }
}

/// Find any active library connection (pool or handle).
#[no_mangle]
pub extern "C" fn rust_pg_find_any_library_connection() -> *mut PgConnection {
    rust_find_any_library_connection() as *mut PgConnection
}
