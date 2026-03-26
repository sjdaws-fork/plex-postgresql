use std::os::raw::{c_char, c_void};
use std::sync::atomic::Ordering;

use crate::db_interpose_conn_utils::log_error;
use crate::ffi_types::{sqlite3_stmt, PgConnection, PgStmt};

#[no_mangle]
pub extern "C" fn rust_stmt_create(
    conn: *mut PgConnection,
    sql: *const c_char,
    shadow_stmt: *mut sqlite3_stmt,
) -> *mut PgStmt {
    unsafe {
        let stmt_ptr = libc::calloc(1, std::mem::size_of::<PgStmt>()) as *mut PgStmt;
        if stmt_ptr.is_null() {
            log_error("pg_stmt_create: calloc failed");
            return std::ptr::null_mut();
        }

        let mut attr: libc::pthread_mutexattr_t = std::mem::zeroed();
        if libc::pthread_mutexattr_init(&mut attr as *mut _) != 0 {
            log_error("pg_stmt_create: pthread_mutexattr_init failed");
            libc::free(stmt_ptr as *mut c_void);
            return std::ptr::null_mut();
        }
        libc::pthread_mutexattr_settype(&mut attr as *mut _, libc::PTHREAD_MUTEX_RECURSIVE);
        libc::pthread_mutex_init(&mut (*stmt_ptr).mutex as *mut _, &attr as *const _);
        libc::pthread_mutexattr_destroy(&mut attr as *mut _);

        (*stmt_ptr).ref_count.store(1, Ordering::Release);
        (*stmt_ptr).conn = conn;
        (*stmt_ptr).shadow_stmt = shadow_stmt;
        (*stmt_ptr).sql = if sql.is_null() {
            std::ptr::null_mut()
        } else {
            libc::strdup(sql)
        };
        (*stmt_ptr).current_row = -1;
        (*stmt_ptr).cached_row = -1;
        (*stmt_ptr).decoded_blob_row = -1;
        (*stmt_ptr).write_executed = 0;
        (*stmt_ptr).read_done = 0;

        stmt_ptr
    }
}
