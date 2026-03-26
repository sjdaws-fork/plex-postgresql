use std::sync::atomic::Ordering;

use super::pool;

/// Get global metadata ID (atomic).
#[no_mangle]
pub extern "C" fn rust_get_global_metadata_id() -> i64 {
    pool().global_metadata_id.load(Ordering::SeqCst)
}

/// Set global metadata ID (atomic).
#[no_mangle]
pub extern "C" fn rust_set_global_metadata_id(id: i64) {
    pool().global_metadata_id.store(id, Ordering::SeqCst);
}

/// Get global last_insert_rowid (atomic).
#[no_mangle]
pub extern "C" fn rust_get_global_last_insert_rowid() -> i64 {
    pool().global_last_insert_rowid.load(Ordering::SeqCst)
}

/// Set global last_insert_rowid (atomic).
#[no_mangle]
pub extern "C" fn rust_set_global_last_insert_rowid(id: i64) {
    pool().global_last_insert_rowid.store(id, Ordering::SeqCst)
}
