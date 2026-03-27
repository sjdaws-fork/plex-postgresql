use super::{stmt_cache_disabled, with_tls_cache};

/// Register a cached statement in the TLS cache.
/// Increments ref_count.
pub fn rust_cached_stmt_register(sqlite_stmt: usize, pg_stmt: usize) {
    if sqlite_stmt == 0 || pg_stmt == 0 {
        return;
    }
    if stmt_cache_disabled() {
        return;
    }
    with_tls_cache(|cache| {
        cache.register(sqlite_stmt, pg_stmt);
    });
}

/// Find a cached statement in the TLS cache.
/// Returns 0 if not found.
pub fn rust_cached_stmt_find(sqlite_stmt: usize) -> usize {
    if sqlite_stmt == 0 {
        return 0;
    }
    if stmt_cache_disabled() {
        return 0;
    }
    with_tls_cache(|cache| cache.find(sqlite_stmt).unwrap_or(0)).unwrap_or(0)
}

/// Remove a cached statement from the TLS cache with unref.
pub fn rust_cached_stmt_clear(sqlite_stmt: usize) {
    if sqlite_stmt == 0 {
        return;
    }
    if stmt_cache_disabled() {
        return;
    }
    with_tls_cache(|cache| {
        cache.clear(sqlite_stmt);
    });
}

/// Remove a cached statement from the TLS cache WITHOUT unref (weak clear).
/// Used by finalize() because the global registry owns the reference.
pub fn rust_cached_stmt_clear_weak(sqlite_stmt: usize) {
    if sqlite_stmt == 0 {
        return;
    }
    if stmt_cache_disabled() {
        return;
    }
    with_tls_cache(|cache| {
        cache.clear_weak(sqlite_stmt);
    });
}

/// Drain all TLS cached statements (for thread exit cleanup).
/// Returns the pg_stmt pointers that need unreffing. The C shim calls this
/// from the TLS destructor.
///
/// # Safety
/// The returned array is heap-allocated and must be freed by the caller.
/// `count_out` must point to a valid i32.
pub fn rust_cached_stmt_drain_all(count_out: *mut i32) -> *mut usize {
    if stmt_cache_disabled() {
        if !count_out.is_null() {
            unsafe {
                *count_out = 0;
            }
        }
        return std::ptr::null_mut();
    }
    let stmts = with_tls_cache(|cache| cache.drain_all());
    let stmts = stmts.unwrap_or_default();
    let count = stmts.len();

    if !count_out.is_null() {
        unsafe {
            *count_out = count as i32;
        }
    }

    if stmts.is_empty() {
        return std::ptr::null_mut();
    }

    unsafe {
        let ptr = libc::malloc(count * std::mem::size_of::<usize>()) as *mut usize;
        if ptr.is_null() {
            return std::ptr::null_mut();
        }
        std::ptr::copy_nonoverlapping(stmts.as_ptr(), ptr, count);
        ptr
    }
}
