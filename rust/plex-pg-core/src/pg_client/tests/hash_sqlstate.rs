use super::*;

// ── fnv1a_str / rust_hash_sql (existing tests) ──────────────────────────

#[test]
fn hash_null_returns_zero() {
    assert_eq!(rust_hash_sql(std::ptr::null()), 0);
}

#[test]
fn hash_same_string_is_deterministic() {
    let sql = "SELECT id FROM metadata WHERE guid = $1";
    let h1 = fnv1a_str(sql);
    let h2 = fnv1a_str(sql);
    assert_eq!(h1, h2);
}

#[test]
fn hash_different_strings_differ() {
    let h1 = fnv1a_str("SELECT 1");
    let h2 = fnv1a_str("SELECT 2");
    assert_ne!(h1, h2);
}

#[test]
fn hash_empty_string_is_nonzero() {
    let h = fnv1a_str("");
    assert_ne!(h, 0);
}

#[test]
fn hash_empty_string_consistent() {
    assert_eq!(fnv1a_str(""), fnv1a_str(""));
}

#[test]
fn hash_known_value_matches_c_implementation() {
    let expected: u64 = {
        let mut h: u64 = 14695981039346656037;
        for b in b"SELECT 1" {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h
    };
    assert_eq!(fnv1a_str("SELECT 1"), expected);
}

#[test]
fn hash_similar_strings_differ() {
    let h1 = fnv1a_str("INSERT INTO t VALUES ($1)");
    let h2 = fnv1a_str("INSERT INTO t VALUES ($2)");
    assert_ne!(h1, h2);
}

#[test]
fn hash_ffi_nonempty_nonzero() {
    let cs = c("SELECT * FROM metadata");
    assert_ne!(rust_hash_sql(cs.as_ptr()), 0);
}

#[test]
fn hash_ffi_matches_pure_helper() {
    let sql = "UPDATE metadata SET title=$1 WHERE id=$2";
    let cs = c(sql);
    assert_eq!(rust_hash_sql(cs.as_ptr()), fnv1a_str(sql));
}

#[test]
fn hash_case_sensitive() {
    assert_ne!(fnv1a_str("select 1"), fnv1a_str("SELECT 1"));
}
