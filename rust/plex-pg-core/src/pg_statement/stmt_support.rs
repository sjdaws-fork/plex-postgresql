use crate::ffi_types::{PgStmt, PARAM_BUF_LEN};
use std::sync::atomic::{AtomicI32, Ordering};

use super::{rust_stmt_ref, rust_stmt_unref};

pub(super) const MAX_CACHED_STMTS_PER_THREAD: usize = 64;

static LEAK_STMTS: std::sync::LazyLock<AtomicI32> = std::sync::LazyLock::new(|| AtomicI32::new(-1));
static DISABLE_STMT_CACHE: std::sync::LazyLock<AtomicI32> =
    std::sync::LazyLock::new(|| AtomicI32::new(-1));

fn env_truthy(name: &str) -> bool {
    crate::env_utils::env_truthy_str(name)
}

pub(super) fn leak_enabled() -> bool {
    let cached = LEAK_STMTS.load(Ordering::Relaxed);
    if cached >= 0 {
        return cached != 0;
    }
    let enabled = env_truthy("PLEX_PG_LEAK_STMTS");
    LEAK_STMTS.store(enabled as i32, Ordering::Relaxed);
    enabled
}

pub(super) fn stmt_cache_disabled() -> bool {
    let cached = DISABLE_STMT_CACHE.load(Ordering::Relaxed);
    if cached >= 0 {
        return cached != 0;
    }
    let enabled = env_truthy("PLEX_PG_DISABLE_STMT_CACHE");
    DISABLE_STMT_CACHE.store(enabled as i32, Ordering::Relaxed);
    enabled
}

pub(super) fn stmt_ref_ptr(pg_stmt: usize) {
    if pg_stmt == 0 {
        return;
    }
    rust_stmt_ref(pg_stmt as *mut PgStmt);
}

pub(super) fn stmt_unref_ptr(pg_stmt: usize) {
    if pg_stmt == 0 {
        return;
    }
    rust_stmt_unref(pg_stmt as *mut PgStmt);
}

#[allow(dead_code)]
pub(super) unsafe fn is_preallocated_buffer(stmt: &PgStmt, idx: usize) -> bool {
    let val = stmt.param_values[idx] as usize;
    if val == 0 {
        return false;
    }
    let buf_ptr = stmt.param_buffers[idx].as_ptr() as usize;
    val >= buf_ptr && val < buf_ptr + PARAM_BUF_LEN
}
