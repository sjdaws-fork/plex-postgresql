use super::*;

// ═════════════════════════════════════════════════════════════════════════
// Db-to-Pool Mapping
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn db_to_pool_assign_and_find() {
    let dtp = DbToPool::new();
    dtp.assign(0x100, 5);
    assert_eq!(dtp.find(0x100), Some(5));
}

#[test]
fn db_to_pool_find_missing_returns_none() {
    let dtp = DbToPool::new();
    assert_eq!(dtp.find(0x100), None);
}

#[test]
fn db_to_pool_release_removes() {
    let dtp = DbToPool::new();
    dtp.assign(0x100, 5);
    assert_eq!(dtp.release(0x100), Some(5));
    assert_eq!(dtp.find(0x100), None);
}

#[test]
fn db_to_pool_multiple_handles_same_slot() {
    // Multiple sqlite3* handles can share a pool slot
    let dtp = DbToPool::new();
    dtp.assign(0x100, 5);
    dtp.assign(0x200, 5);
    assert_eq!(dtp.find(0x100), Some(5));
    assert_eq!(dtp.find(0x200), Some(5));
}

#[test]
fn db_to_pool_clear() {
    let dtp = DbToPool::new();
    dtp.assign(0x100, 5);
    dtp.assign(0x200, 7);
    dtp.clear();
    assert_eq!(dtp.len(), 0);
}
