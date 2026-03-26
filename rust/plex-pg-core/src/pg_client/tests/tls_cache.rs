use super::super::tls_cache::{tls_pool_cache_clear, tls_pool_cache_get, tls_pool_cache_set};
use super::*;

// ═════════════════════════════════════════════════════════════════════════
// TLS Pool Cache
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn tls_pool_cache_initially_empty() {
    tls_pool_cache_clear();
    assert!(tls_pool_cache_get(0x1234).is_none());
}

#[test]
fn tls_pool_cache_set_and_get() {
    tls_pool_cache_set(0xABCD, 5, 42);
    let result = tls_pool_cache_get(0xABCD);
    assert_eq!(result, Some((5, 42)));
}

#[test]
fn tls_pool_cache_miss_for_different_db() {
    tls_pool_cache_set(0xABCD, 5, 42);
    assert!(tls_pool_cache_get(0x9999).is_none());
}

#[test]
fn tls_pool_cache_clear_makes_miss() {
    tls_pool_cache_set(0xABCD, 5, 42);
    tls_pool_cache_clear();
    assert!(tls_pool_cache_get(0xABCD).is_none());
}

#[test]
fn tls_pool_cache_overwrite() {
    tls_pool_cache_set(0xABCD, 5, 42);
    tls_pool_cache_set(0xABCD, 7, 99);
    assert_eq!(tls_pool_cache_get(0xABCD), Some((7, 99)));
}

#[test]
fn tls_pool_cache_is_thread_local() {
    tls_pool_cache_clear();
    tls_pool_cache_set(0x1111, 3, 10);

    let result = std::thread::spawn(|| {
        // Other thread should not see our cache
        tls_pool_cache_get(0x1111)
    })
    .join()
    .unwrap();

    assert!(result.is_none());
    // Our thread still has it
    assert_eq!(tls_pool_cache_get(0x1111), Some((3, 10)));
}

#[test]
fn tls_pool_cache_generation_detects_stale() {
    let pool = PoolManager::new(10, 64);
    let fake_conn = 0xDEAD as *mut c_void;

    // Simulate: slot 3 is ready with generation 5
    pool.slots[3].conn.store(fake_conn, Ordering::Relaxed);
    pool.slots[3].state.store(SLOT_READY, Ordering::Relaxed);
    pool.slots[3].generation.store(5, Ordering::Relaxed);

    // Cache it in TLS
    tls_pool_cache_set(0xAAAA, 3, 5);

    // Verify fast path would succeed
    let (idx, gen) = tls_pool_cache_get(0xAAAA).unwrap();
    assert_eq!(idx, 3);
    assert_eq!(
        pool.slots[idx as usize].generation.load(Ordering::Acquire),
        gen
    );

    // Now simulate reaper bumping generation
    pool.slots[3].generation.fetch_add(1, Ordering::SeqCst);

    // TLS cache still returns the old generation
    let (idx, gen) = tls_pool_cache_get(0xAAAA).unwrap();
    // But the slot generation no longer matches → stale!
    assert_ne!(
        pool.slots[idx as usize].generation.load(Ordering::Acquire),
        gen
    );
}
