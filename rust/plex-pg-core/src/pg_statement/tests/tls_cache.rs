use super::*;

#[test]
fn tls_cache_register_and_find() {
    let mut cache = ThreadCachedStmts::new();
    let stmt = make_stmt("SELECT 1");
    cache.register(0x100, stmt as usize);
    assert_eq!(cache.find(0x100), Some(stmt as usize));
    assert_eq!(ref_count(stmt), 2);

    cache.clear(0x100);
    assert_eq!(cache.find(0x100), None);
    assert_eq!(ref_count(stmt), 1);

    rust_stmt_unref(stmt);
}

#[test]
fn tls_cache_find_missing_returns_none() {
    let cache = ThreadCachedStmts::new();
    assert_eq!(cache.find(0x999), None);
}

#[test]
fn tls_cache_replace_unrefs_old() {
    let mut cache = ThreadCachedStmts::new();
    let stmt_a = make_stmt("SELECT 1");
    let stmt_b = make_stmt("SELECT 2");

    cache.register(0x100, stmt_a as usize);
    assert_eq!(ref_count(stmt_a), 2);

    cache.register(0x100, stmt_b as usize);

    assert_eq!(ref_count(stmt_a), 1);
    assert_eq!(ref_count(stmt_b), 2);
    assert_eq!(cache.find(0x100), Some(stmt_b as usize));

    cache.clear(0x100);
    assert_eq!(cache.find(0x100), None);
    assert_eq!(ref_count(stmt_b), 1);

    rust_stmt_unref(stmt_a);
    rust_stmt_unref(stmt_b);
}

#[test]
fn tls_cache_clear_unrefs() {
    let mut cache = ThreadCachedStmts::new();
    let stmt = make_stmt("SELECT 1");
    cache.register(0x100, stmt as usize);
    assert_eq!(ref_count(stmt), 2);

    cache.clear(0x100);
    assert_eq!(ref_count(stmt), 1);
    assert_eq!(cache.find(0x100), None);

    rust_stmt_unref(stmt);
}

#[test]
fn tls_cache_clear_weak_does_not_unref() {
    let mut cache = ThreadCachedStmts::new();
    let stmt = make_stmt("SELECT 1");
    cache.register(0x100, stmt as usize);
    assert_eq!(ref_count(stmt), 2);

    cache.clear_weak(0x100);
    assert_eq!(ref_count(stmt), 2);
    assert_eq!(cache.find(0x100), None);

    rust_stmt_unref(stmt);
    rust_stmt_unref(stmt);
}

#[test]
fn tls_cache_fifo_eviction() {
    let mut cache = ThreadCachedStmts::new();
    let mut stmts: Vec<*mut PgStmt> = Vec::new();

    for i in 0..MAX_CACHED_STMTS_PER_THREAD {
        let stmt = make_stmt("SELECT 1");
        cache.register(0x1000 + i, stmt as usize);
        stmts.push(stmt);
    }
    assert_eq!(cache.entries.len(), MAX_CACHED_STMTS_PER_THREAD);

    let extra = make_stmt("SELECT 2");
    cache.register(0x9999, extra as usize);
    stmts.push(extra);
    assert_eq!(cache.find(0x1000), None);
    assert_eq!(cache.find(0x9999), Some(extra as usize));

    let cached = cache.drain_all();
    for pg_stmt in cached {
        rust_stmt_unref(pg_stmt as *mut PgStmt);
    }
    for stmt in stmts {
        rust_stmt_unref(stmt);
    }
}

#[test]
fn tls_cache_drain_all_returns_all_pg_stmts() {
    let mut cache = ThreadCachedStmts::new();
    let stmt_a = make_stmt("SELECT 1");
    let stmt_b = make_stmt("SELECT 2");
    cache.register(0x100, stmt_a as usize);
    cache.register(0x300, stmt_b as usize);

    let drained = cache.drain_all();
    assert_eq!(drained.len(), 2);
    assert!(drained.contains(&(stmt_a as usize)));
    assert!(drained.contains(&(stmt_b as usize)));
    assert!(cache.entries.is_empty());

    for pg_stmt in drained {
        rust_stmt_unref(pg_stmt as *mut PgStmt);
    }
    rust_stmt_unref(stmt_a);
    rust_stmt_unref(stmt_b);
}

#[test]
fn tls_cache_is_thread_local() {
    let stmt = make_stmt("SELECT 1");

    with_tls_cache(|cache| {
        cache.register(0xAAAA, stmt as usize);
    });

    let handle =
        std::thread::spawn(|| with_tls_cache(|cache| cache.find(0xAAAA).is_none()).unwrap_or(true));

    assert!(handle.join().unwrap());

    with_tls_cache(|cache| {
        cache.clear(0xAAAA);
    });

    rust_stmt_unref(stmt);
}

#[test]
fn ffi_find_any_falls_back_to_tls() {
    let s = 0x50000_usize;
    let stmt = make_stmt("SELECT 1");

    assert_eq!(rust_stmt_find(s), 0);

    with_tls_cache(|cache| {
        cache.register(s, stmt as usize);
    });

    assert_eq!(rust_stmt_find_any(s), stmt as usize);

    with_tls_cache(|cache| {
        cache.clear(s);
    });

    rust_stmt_unref(stmt);
}
