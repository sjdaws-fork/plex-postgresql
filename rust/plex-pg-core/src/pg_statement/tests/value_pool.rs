use super::*;

#[test]
fn pg_value_create_sets_magic() {
    let ptr = rust_create_column_value(0x1234, 0, SQLITE_INTEGER);
    assert!(!ptr.is_null());
    let val = unsafe { &*ptr };
    assert_eq!(val.magic, PG_VALUE_MAGIC);
    assert_eq!(val.stmt, 0x1234);
    assert_eq!(val.col_idx, 0);
    assert_eq!(val.sqlite_type, SQLITE_INTEGER);
}

#[test]
fn pg_value_is_our_value_true() {
    let ptr = rust_create_column_value(0x5678, 3, SQLITE_TEXT);
    assert_eq!(rust_is_our_value(ptr), 1);
}

#[test]
fn pg_value_is_our_value_null_false() {
    assert_eq!(rust_is_our_value(std::ptr::null()), 0);
}

#[test]
fn pg_value_pool_wraps_around() {
    for i in 0..MAX_PG_VALUES + 10 {
        let ptr = rust_create_column_value(i, 0, SQLITE_INTEGER);
        assert!(!ptr.is_null());
    }
}
