use super::*;

// ── SQLSTATE tests (existing) ───────────────────────────────────────────

#[test]
fn stale_exact_match_returns_one() {
    assert_eq!(rust_is_stale_sqlstate(c("26000").as_ptr()), 1);
}

#[test]
fn stale_null_returns_zero() {
    assert_eq!(rust_is_stale_sqlstate(std::ptr::null()), 0);
}

#[test]
fn stale_empty_string_returns_zero() {
    assert_eq!(rust_is_stale_sqlstate(c("").as_ptr()), 0);
}

#[test]
fn stale_wrong_code_42p05_returns_zero() {
    assert_eq!(rust_is_stale_sqlstate(c("42P05").as_ptr()), 0);
}

#[test]
fn stale_close_but_wrong_26001_returns_zero() {
    assert_eq!(rust_is_stale_sqlstate(c("26001").as_ptr()), 0);
}

#[test]
fn stale_pure_helper_true() {
    assert!(is_stale_sqlstate("26000"));
}

#[test]
fn stale_pure_helper_false_for_prefix() {
    assert!(!is_stale_sqlstate("2600"));
}

#[test]
fn duplicate_exact_match_returns_one() {
    assert_eq!(rust_is_duplicate_sqlstate(c("42P05").as_ptr()), 1);
}

#[test]
fn duplicate_null_returns_zero() {
    assert_eq!(rust_is_duplicate_sqlstate(std::ptr::null()), 0);
}

#[test]
fn duplicate_empty_string_returns_zero() {
    assert_eq!(rust_is_duplicate_sqlstate(c("").as_ptr()), 0);
}

#[test]
fn duplicate_wrong_code_26000_returns_zero() {
    assert_eq!(rust_is_duplicate_sqlstate(c("26000").as_ptr()), 0);
}

#[test]
fn duplicate_close_but_wrong_42p06_returns_zero() {
    assert_eq!(rust_is_duplicate_sqlstate(c("42P06").as_ptr()), 0);
}

#[test]
fn duplicate_pure_helper_true() {
    assert!(is_duplicate_sqlstate("42P05"));
}

#[test]
fn duplicate_pure_helper_false_for_lowercase() {
    assert!(!is_duplicate_sqlstate("42p05"));
}
