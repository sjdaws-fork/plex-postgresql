use super::*;

#[test]
fn registry_register_and_find() {
    let mut reg = StmtRegistry::new();
    reg.register(0x1000, 0x2000);
    assert_eq!(reg.find(0x1000), Some(0x2000));
}

#[test]
fn registry_find_missing_returns_none() {
    let reg = StmtRegistry::new();
    assert_eq!(reg.find(0xDEAD), None);
}

#[test]
fn registry_unregister_removes_both_maps() {
    let mut reg = StmtRegistry::new();
    reg.register(0x3000, 0x4000);
    assert!(reg.is_ours(0x4000));
    reg.unregister(0x3000);
    assert_eq!(reg.find(0x3000), None);
    assert!(!reg.is_ours(0x4000));
}

#[test]
fn registry_is_ours_true_for_registered() {
    let mut reg = StmtRegistry::new();
    reg.register(0x5000, 0x6000);
    assert!(reg.is_ours(0x6000));
}

#[test]
fn registry_is_ours_false_for_unregistered() {
    let reg = StmtRegistry::new();
    assert!(!reg.is_ours(0xBEEF));
}

#[test]
fn registry_replace_existing_mapping() {
    let mut reg = StmtRegistry::new();
    reg.register(0x7000, 0x8000);
    assert_eq!(reg.find(0x7000), Some(0x8000));
    reg.register(0x7000, 0x9000);
    assert_eq!(reg.find(0x7000), Some(0x9000));
    assert!(!reg.is_ours(0x8000));
    assert!(reg.is_ours(0x9000));
}

#[test]
fn registry_clear_empties_all() {
    let mut reg = StmtRegistry::new();
    reg.register(0xA000, 0xB000);
    reg.register(0xC000, 0xD000);
    assert_eq!(reg.len(), 2);
    reg.clear();
    assert_eq!(reg.len(), 0);
    assert_eq!(reg.find(0xA000), None);
}

#[test]
fn registry_concurrent_readers() {
    let key = 0xFF_E000_usize;
    let val = 0xFF_F000_usize;

    REGISTRY
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .register(key, val);

    let handles: Vec<_> = (0..8)
        .map(move |_| {
            std::thread::spawn(move || {
                let reg = REGISTRY.read().unwrap_or_else(|e| e.into_inner());
                reg.find(key)
            })
        })
        .collect();

    for h in handles {
        assert_eq!(h.join().unwrap(), Some(val));
    }

    REGISTRY
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .unregister(key);
}

#[test]
fn ffi_register_find_unregister() {
    let s = 0x10000_usize;
    let p = 0x20000_usize;

    rust_stmt_register(s, p);
    assert_eq!(rust_stmt_find(s), p);
    assert_eq!(rust_stmt_is_ours(p), 1);

    rust_stmt_unregister(s);
    assert_eq!(rust_stmt_find(s), 0);
    assert_eq!(rust_stmt_is_ours(p), 0);
}

#[test]
fn ffi_find_null_returns_zero() {
    assert_eq!(rust_stmt_find(0), 0);
}

#[test]
fn ffi_find_any_checks_registry_first() {
    let s = 0x30000_usize;
    let p = 0x40000_usize;

    rust_stmt_register(s, p);
    assert_eq!(rust_stmt_find_any(s), p);
    rust_stmt_unregister(s);
}
