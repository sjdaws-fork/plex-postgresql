use super::*;

// ═════════════════════════════════════════════════════════════════════════
// Global Atomics
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn global_metadata_id_default_zero() {
    let pm = PoolManager::new(1, 64);
    assert_eq!(pm.global_metadata_id.load(Ordering::Relaxed), 0);
}

#[test]
fn global_metadata_id_set_and_get() {
    let pm = PoolManager::new(1, 64);
    pm.global_metadata_id.store(12345, Ordering::Relaxed);
    assert_eq!(pm.global_metadata_id.load(Ordering::Relaxed), 12345);
}

#[test]
fn global_last_insert_rowid_set_and_get() {
    let pm = PoolManager::new(1, 64);
    pm.global_last_insert_rowid.store(67890, Ordering::Relaxed);
    assert_eq!(pm.global_last_insert_rowid.load(Ordering::Relaxed), 67890);
}
