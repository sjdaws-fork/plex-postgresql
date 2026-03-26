use super::*;

// ═════════════════════════════════════════════════════════════════════════
// Fork Safety
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn pool_reset_for_child_clears_all_slots() {
    let pm = PoolManager::new(5, 64);
    let fake_conn = 0xBEEF as *mut c_void;

    // Set up slot 2 as READY with a connection
    pm.slots[2].conn.store(fake_conn, Ordering::Relaxed);
    pm.slots[2].state.store(SLOT_READY, Ordering::Relaxed);
    pm.slots[2].owner_thread.store(999, Ordering::Relaxed);
    pm.slots[2].generation.store(5, Ordering::Relaxed);

    pm.registry.register(0x100, 0xAAA);
    pm.db_to_pool.assign(0x100, 2);

    pm.reset_for_child();

    // All slots should be FREE with null conn
    for slot in &pm.slots {
        assert_eq!(slot.state.load(Ordering::Relaxed), SLOT_FREE);
        assert!(slot.conn.load(Ordering::Relaxed).is_null());
        assert_eq!(slot.owner_thread.load(Ordering::Relaxed), 0);
    }
    // Generation should have been bumped
    assert!(pm.slots[2].generation.load(Ordering::Relaxed) > 5);

    // Registry and db_to_pool should be empty
    assert_eq!(pm.registry.len(), 0);
    assert_eq!(pm.db_to_pool.len(), 0);
}
