use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::os::raw::c_int;
use std::os::raw::c_uint;
use std::os::raw::c_void;
use std::sync::{LazyLock, RwLock};
use std::fs::File;
use std::io::{BufRead, BufReader};
use crate::db_interpose_prepare_helpers::{
    add_if_not_exists_for_sqlite_ddl, alias_collection_sync_aggregates, prepare_query_loop_tick,
    prepare_simple_hash, prepare_time_ms, simplify_fts_for_sqlite, strip_collate_icu_root,
};
use crate::db_interpose_trace_helpers::{
    getenv_nonempty, list_any_token_in_haystack, list_contains_idx, read_first_line_trimmed,
    trim_first_line,
};
use crate::db_interpose_value_helpers::{
    pg_oid_to_sqlite_type_impl, pg_text_to_double_impl, pg_text_to_int64_impl, pg_text_to_int_impl,
    SQLITE_BLOB_CONST, SQLITE_FLOAT_CONST, SQLITE_INTEGER_CONST, SQLITE_TEXT_CONST,
};
pub use crate::libpq_helpers::PGresult;
use crate::pg_query_cache::rust_query_cache_store;

mod decltype;
mod string_utils;

use string_utils::{
    contains_ascii_icase, contains_ascii_icase_str, cstr_to_str, find_ascii_icase,
    find_closing_paren, find_subslice, has_boundary, is_next_numeric_boundary,
    is_prev_numeric_boundary, normalize_ident_token, push_capped, slice_eq_icase,
    split_csv_simple, starts_with_icase, write_buf, write_i32_to_buf, write_i64_to_buf,
};
pub(crate) use decltype::{normalize_sqlite_decltype_impl, pg_udt_to_sqlite_decltype_impl};
pub(crate) use string_utils::cstr_to_str_or_empty;

const SQLITE_NULL_CONST: i32 = 5;

static DECLTYPE_CACHE: LazyLock<RwLock<HashMap<String, CString>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static OID_TABLE_CACHE: LazyLock<RwLock<HashMap<u32, CString>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

extern "C" {
    fn PQgetisnull(res: *const PGresult, row: c_int, col: c_int) -> c_int;
    fn PQgetvalue(res: *const PGresult, row: c_int, col: c_int) -> *const c_char;
    fn PQgetlength(res: *const PGresult, row: c_int, col: c_int) -> c_int;
    fn PQftype(res: *const PGresult, col: c_int) -> c_uint;
    fn PQfname(res: *const PGresult, col: c_int) -> *const c_char;
    fn PQftable(res: *const PGresult, col: c_int) -> c_uint;
    fn PQntuples(res: *const PGresult) -> c_int;
    fn PQnfields(res: *const PGresult) -> c_int;
}

fn rewrite_server_library_uri_bytes(input_bytes: &[u8], out_cap: usize) -> Option<Vec<u8>> {
    const SERVER_PREFIX: &[u8] = b"server://";
    const NEEDLE: &[u8] = b"/com.plexapp.plugins.library/library/";
    const REPLACEMENT: &[u8] = b"library://";

    if find_subslice(input_bytes, SERVER_PREFIX).is_none()
        || find_subslice(input_bytes, NEEDLE).is_none()
    {
        return None;
    }

    let mut out_buf = Vec::with_capacity(input_bytes.len().min(out_cap));
    let mut in_pos = 0usize;
    let mut rewrites = 0;

    while in_pos < input_bytes.len() {
        if out_buf.len() >= out_cap {
            break;
        }
        let slice = &input_bytes[in_pos..];
        let Some(rel_match) = find_subslice(slice, SERVER_PREFIX) else {
            push_capped(&mut out_buf, out_cap, slice);
            break;
        };
        let abs_match = in_pos + rel_match;
        push_capped(&mut out_buf, out_cap, &input_bytes[in_pos..abs_match]);
        in_pos = abs_match;

        let search_start = in_pos + SERVER_PREFIX.len();
        if search_start >= input_bytes.len() {
            push_capped(&mut out_buf, out_cap, &input_bytes[in_pos..]);
            break;
        }

        let tail = &input_bytes[search_start..];
        let Some(rel_lib) = find_subslice(tail, NEEDLE) else {
            push_capped(&mut out_buf, out_cap, &input_bytes[in_pos..search_start]);
            in_pos = search_start;
            continue;
        };
        let abs_lib = search_start + rel_lib;
        let lib_end = abs_lib + NEEDLE.len();

        push_capped(&mut out_buf, out_cap, REPLACEMENT);
        in_pos = lib_end;
        rewrites += 1;
    }

    if rewrites == 0 {
        None
    } else {
        Some(out_buf)
    }
}

fn is_aggregate_alias(col: &str) -> bool {
    if col.is_empty() {
        return false;
    }
    let lower = col.to_ascii_lowercase();
    matches!(lower.as_str(), "count" | "sum" | "max" | "min" | "avg")
        || contains_ascii_icase_str(col, "count(")
}

fn pg_sql_has_timestamp_hint(pg_sql: &str) -> bool {
    pg_sql.contains("_at")
        || pg_sql.contains("changed_at")
        || pg_sql.contains("updated_at")
        || pg_sql.contains("created_at")
}

fn normalize_sql_literals_impl(sql: &str) -> Option<(String, Vec<String>)> {
    const MAX_NORMALIZED_PARAMS: usize = 32;

    let bytes = sql.as_bytes();
    if bytes.len() >= 6 && bytes[..6].eq_ignore_ascii_case(b"INSERT") {
        return None;
    }
    if !contains_ascii_icase(bytes, b"WHERE") {
        return None;
    }

    let mut out = String::with_capacity(sql.len() + MAX_NORMALIZED_PARAMS * 4);
    let mut params: Vec<String> = Vec::with_capacity(MAX_NORMALIZED_PARAMS);
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;

    while i < bytes.len() {
        if params.len() < MAX_NORMALIZED_PARAMS {
            let b = bytes[i];
            let is_number_start = b.is_ascii_digit()
                || (b == b'-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit());
            if is_number_start && !in_single && !in_double {
                let prev = if i == 0 { b' ' } else { bytes[i - 1] };
                if is_prev_numeric_boundary(prev) {
                    let num_start = i;
                    if bytes[i] == b'-' {
                        i += 1;
                    }
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
                        i += 1;
                        while i < bytes.len() && bytes[i].is_ascii_digit() {
                            i += 1;
                        }
                    }

                    if is_next_numeric_boundary(bytes, i) {
                        let lit = &sql[num_start..i];
                        params.push(lit.to_string());
                        out.push('$');
                        out.push_str(&(params.len()).to_string());
                        continue;
                    }
                    i = num_start;
                }
            }
        }

        let b = bytes[i];
        out.push(b as char);
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        }
        i += 1;
    }

    if params.is_empty() {
        return None;
    }
    Some((out, params))
}

fn is_library_db_path_impl(path: &str) -> bool {
    let mut bytes = path.as_bytes();
    if bytes.len() > 4 && (bytes.ends_with(b"-wal") || bytes.ends_with(b"-shm")) {
        bytes = &bytes[..bytes.len() - 4];
    }
    contains_ascii_icase(bytes, b"com.plexapp.plugins.library.db")
        || contains_ascii_icase(bytes, b"com.plexapp.plugins.library.blobs.db")
}

fn is_library_or_blobs_db_path_impl(path: &str) -> bool {
    contains_ascii_icase(path.as_bytes(), b"com.plexapp.plugins.library.db")
        || contains_ascii_icase(path.as_bytes(), b"com.plexapp.plugins.library.blobs.db")
}

fn is_blobs_db_path_impl(path: &str) -> bool {
    contains_ascii_icase(path.as_bytes(), b"com.plexapp.plugins.library.blobs.db")
}

fn contains_binary_bytes_impl(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    for (i, c) in data.iter().copied().enumerate() {
        if c < 0x20 && c != 0x09 && c != 0x0A && c != 0x0D {
            return true;
        }
        if c == 0x7F || c == 0xC0 || c == 0xC1 || c >= 0xF5 {
            return true;
        }
        if i == 0 && data.len() >= 2 && c == 0x1F && data[1] == 0x8B {
            return true;
        }
    }
    false
}

fn bytes_to_pg_hex_impl(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(2 + data.len() * 2);
    out.push('\\');
    out.push('x');
    for b in data {
        use std::fmt::Write;
        let _ = write!(&mut out, "{:02x}", b);
    }
    out
}

fn is_related_items_query_impl(pg_sql: &str) -> bool {
    pg_sql.contains("taggings as related")
}

fn should_mask_collection_metadata_type_impl(pg_sql: &str, col_name: &str, raw_val: i64) -> bool {
    raw_val == 18 && col_name.contains("metadata_type") && is_related_items_query_impl(pg_sql)
}

fn find_insert_column_index_impl(sql: &str, column_name: &str) -> i32 {
    if column_name.is_empty() {
        return -1;
    }
    let bytes = sql.as_bytes();
    if !(contains_ascii_icase(bytes, b"INSERT") && contains_ascii_icase(bytes, b"INTO")) {
        return -1;
    }
    let Some(cols_open) = find_ascii_icase(bytes, b"(") else {
        return -1;
    };
    let Some(cols_close) = find_closing_paren(bytes, cols_open + 1) else {
        return -1;
    };
    let cols_section = &sql[cols_open + 1..cols_close];
    let cols = split_csv_simple(cols_section);
    for (i, c) in cols.iter().enumerate() {
        if normalize_ident_token(c).eq_ignore_ascii_case(column_name) {
            return i as i32;
        }
    }
    -1
}

fn is_junk_metadata_insert_impl(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    if !(contains_ascii_icase(bytes, b"INSERT") && contains_ascii_icase(bytes, b"metadata_items")) {
        return false;
    }
    if contains_ascii_icase(bytes, b"metadata_item_settings")
        || contains_ascii_icase(bytes, b"metadata_item_views")
        || contains_ascii_icase(bytes, b"metadata_item_accounts")
        || contains_ascii_icase(bytes, b"metadata_item_clusters")
    {
        return false;
    }

    let Some(cols_open) = find_ascii_icase(bytes, b"(") else {
        return false;
    };
    let Some(cols_close) = find_closing_paren(bytes, cols_open + 1) else {
        return false;
    };
    let cols_section = &sql[cols_open + 1..cols_close];
    let cols = split_csv_simple(cols_section);
    if cols.is_empty() {
        return false;
    }

    let mut lib_idx: Option<usize> = None;
    let mut type_idx: Option<usize> = None;
    for (i, c) in cols.iter().enumerate() {
        let c = normalize_ident_token(c);
        if c.eq_ignore_ascii_case("library_section_id") {
            lib_idx = Some(i);
        }
        if c.eq_ignore_ascii_case("metadata_type") {
            type_idx = Some(i);
        }
    }
    let (Some(lib_idx), Some(type_idx)) = (lib_idx, type_idx) else {
        return false;
    };

    let Some(values_pos) = find_ascii_icase(bytes, b"VALUES") else {
        return false;
    };
    let values_bytes = &bytes[values_pos..];
    let Some(v_open_rel) = find_ascii_icase(values_bytes, b"(") else {
        return false;
    };
    let v_open = values_pos + v_open_rel;
    let Some(v_close) = find_closing_paren(bytes, v_open + 1) else {
        return false;
    };
    let values_section = &sql[v_open + 1..v_close];
    let vals = split_csv_simple(values_section);
    if lib_idx >= vals.len() || type_idx >= vals.len() {
        return false;
    }

    let lib_is_null = vals[lib_idx].trim_start().to_ascii_uppercase().starts_with("NULL");
    let type_is_null = vals[type_idx]
        .trim_start()
        .to_ascii_uppercase()
        .starts_with("NULL");
    lib_is_null && type_is_null
}

fn format_epoch_to_datetime_utc_impl(epoch: i64, out: *mut c_char, out_len: usize) -> c_int {
    if out.is_null() || out_len == 0 || epoch <= 0 {
        return 0;
    }

    let t = epoch as libc::time_t;
    let mut tm_utc: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::gmtime_r(&t, &mut tm_utc) };
    if ok.is_null() {
        return 0;
    }

    let fmt = b"%Y-%m-%d %H:%M:%S\0";
    let written = unsafe {
        libc::strftime(
            out as *mut libc::c_char,
            out_len,
            fmt.as_ptr() as *const libc::c_char,
            &tm_utc,
        )
    };
    i32::from(written != 0)
}

#[repr(C)]
pub struct RustNormalizedSql {
    pub normalized_sql: *mut c_char,
    pub param_values: *mut *mut c_char,
    pub param_count: c_int,
}


#[no_mangle]
pub extern "C" fn rust_decltype_hash(ptr: *const c_char) -> u32 {
    let mut hash: u32 = 5381;
    let s = cstr_to_str(ptr).unwrap_or("");
    for b in s.as_bytes() {
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(*b as u32);
    }
    hash
}

#[no_mangle]
pub extern "C" fn rust_decltype_cache_insert(key: *const c_char, decltype_val: *const c_char) -> c_int {
    let key_str = match cstr_to_str(key) {
        Some(s) if !s.is_empty() => s,
        _ => return 0,
    };

    let normalized = normalize_sqlite_decltype_impl(cstr_to_str(decltype_val));
    if normalized.is_null() {
        return 0;
    }
    let normalized_bytes = unsafe { CStr::from_ptr(normalized).to_bytes() };
    let normalized_owned = match CString::new(normalized_bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut cache = match DECLTYPE_CACHE.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.insert(key_str.to_string(), normalized_owned);
    1
}

#[no_mangle]
pub extern "C" fn rust_decltype_cache_lookup(key: *const c_char) -> *const c_char {
    let key_str = match cstr_to_str(key) {
        Some(s) if !s.is_empty() => s,
        _ => return std::ptr::null(),
    };
    let cache = match DECLTYPE_CACHE.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache
        .get(key_str)
        .map(|s| s.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn rust_oid_table_cache_insert(oid: c_uint, name: *const c_char) -> c_int {
    let name_str = match cstr_to_str(name) {
        Some(s) if !s.is_empty() => s,
        _ => return 0,
    };
    let cstr = match CString::new(name_str) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut cache = match OID_TABLE_CACHE.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.entry(oid).or_insert(cstr);
    1
}

#[no_mangle]
pub extern "C" fn rust_oid_table_cache_lookup(oid: c_uint) -> *const c_char {
    let cache = match OID_TABLE_CACHE.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache
        .get(&oid)
        .map(|s| s.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn rust_column_text_reformat_aggregate(
    col_name: *const c_char,
    oid: c_uint,
    pg_sql: *const c_char,
    source_value: *const c_char,
    out: *mut c_char,
    out_len: usize,
) -> c_int {
    if out.is_null() || out_len == 0 {
        return 0;
    }

    let col = match cstr_to_str(col_name) {
        Some(s) if !s.is_empty() => s,
        _ => return 0,
    };

    if !matches!(oid, 20 | 21 | 23) {
        return 0;
    }

    if !is_aggregate_alias(col) {
        return 0;
    }

    if oid == 20 {
        let val = pg_text_to_int64_impl(source_value);
        let pg_sql = cstr_to_str(pg_sql).unwrap_or("");
        if (col.eq_ignore_ascii_case("max") || col.eq_ignore_ascii_case("min"))
            && !pg_sql.is_empty()
            && pg_sql_has_timestamp_hint(pg_sql)
            && format_epoch_to_datetime_utc_impl(val, out, out_len) != 0
        {
            return 1;
        }
        return c_int::from(write_i64_to_buf(out, out_len, val));
    }

    let val = pg_text_to_int_impl(source_value);
    c_int::from(write_i32_to_buf(out, out_len, val))
}

#[no_mangle]
pub extern "C" fn rust_column_text_transform(
    col_name: *const c_char,
    oid: c_uint,
    pg_sql: *const c_char,
    source_value: *const c_char,
    source_len: usize,
    out: *mut c_char,
    out_len: usize,
) -> c_int {
    if out.is_null() || out_len == 0 || source_value.is_null() {
        return 0;
    }

    // Validate UTF-8 first.
    let bytes = unsafe { std::slice::from_raw_parts(source_value as *const u8, source_len) };
    if std::str::from_utf8(bytes).is_err() {
        unsafe {
            *out = 0;
        }
        return -1;
    }

    // Aggregate reformat for integer columns if needed.
    if rust_column_text_reformat_aggregate(col_name, oid, pg_sql, source_value, out, out_len) != 0 {
        return 1;
    }

    // URI rewrite.
    let out_cap = out_len.saturating_sub(1);
    if let Some(rewritten) = rewrite_server_library_uri_bytes(bytes, out_cap) {
        let n = rewritten.len().min(out_cap);
        unsafe {
            std::ptr::copy_nonoverlapping(rewritten.as_ptr(), out as *mut u8, n);
            *out.add(n) = 0;
        }
        return 1;
    }

    0
}

#[no_mangle]
pub extern "C" fn rust_decode_hex_bytes(
    hex: *const c_char,
    hex_len: usize,
    out: *mut u8,
    out_len: usize,
) -> c_int {
    if hex.is_null() || out.is_null() {
        return 0;
    }
    if hex_len == 0 || !hex_len.is_multiple_of(2) {
        return 0;
    }
    let expected = hex_len / 2;
    if out_len < expected {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(hex as *const u8, hex_len) };

    fn hex_val(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }

    let out_slice = unsafe { std::slice::from_raw_parts_mut(out, expected) };
    for i in 0..expected {
        let hi = match hex_val(bytes[i * 2]) {
            Some(v) => v,
            None => return 0,
        };
        let lo = match hex_val(bytes[i * 2 + 1]) {
            Some(v) => v,
            None => return 0,
        };
        out_slice[i] = (hi << 4) | lo;
    }
    expected as c_int
}

#[no_mangle]
pub extern "C" fn rust_expected_sqlite_type_for_decltype(decl: *const c_char) -> c_int {
    let t = match cstr_to_str(decl) {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return -1,
    };
    let bytes = t.as_bytes();

    if starts_with_icase(bytes, b"DT_INTEGER") {
        return SQLITE_INTEGER_CONST;
    }
    if starts_with_icase(bytes, b"INTEGER") && has_boundary(bytes, 7) {
        return SQLITE_INTEGER_CONST;
    }
    if starts_with_icase(bytes, b"BIGINT") && has_boundary(bytes, 6) {
        return SQLITE_INTEGER_CONST;
    }
    if t.eq_ignore_ascii_case("INT8")
        || t.eq_ignore_ascii_case("INT64")
        || t.eq_ignore_ascii_case("LONG")
        || t.eq_ignore_ascii_case("BOOLEAN")
        || t.eq_ignore_ascii_case("TIMESTAMP")
    {
        return SQLITE_INTEGER_CONST;
    }

    if t.eq_ignore_ascii_case("FLOAT") || t.eq_ignore_ascii_case("DOUBLE") || t.eq_ignore_ascii_case("REAL") {
        return SQLITE_FLOAT_CONST;
    }

    if starts_with_icase(bytes, b"VARCHAR") && has_boundary(bytes, 7) {
        return SQLITE_TEXT_CONST;
    }
    if t.eq_ignore_ascii_case("STRING") || t.eq_ignore_ascii_case("CHAR") || t.eq_ignore_ascii_case("TEXT") {
        return SQLITE_TEXT_CONST;
    }

    if t.eq_ignore_ascii_case("BLOB") {
        return SQLITE_BLOB_CONST;
    }

    -1
}

#[no_mangle]
pub extern "C" fn rust_decltype_cache_lookup_alias(alias: *const c_char) -> *const c_char {
    let alias_str = match cstr_to_str(alias) {
        Some(s) if !s.is_empty() => s,
        _ => return std::ptr::null(),
    };
    let Some((table, column)) = alias_str.split_once('_') else {
        return std::ptr::null();
    };
    if table.is_empty() || column.is_empty() {
        return std::ptr::null();
    }
    let key = format!("{}_{}", table, column);
    let cache = match DECLTYPE_CACHE.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache
        .get(&key)
        .map(|s| s.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn rust_pg_result_text_copy(
    result: *const PGresult,
    row: c_int,
    col: c_int,
    out: *mut c_char,
    out_len: usize,
) -> c_int {
    if result.is_null() || out.is_null() || out_len == 0 {
        return -1;
    }
    let is_null = unsafe { PQgetisnull(result, row, col) } != 0;
    if is_null {
        return -1;
    }
    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        return -1;
    }
    let len = unsafe { PQgetlength(result, row, col) };
    if len < 0 {
        return -1;
    }
    let len_usize = len as usize;
    let copy_len = len_usize.min(out_len.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(val_ptr as *const u8, out as *mut u8, copy_len);
        *out.add(copy_len) = 0;
    }
    len
}

#[no_mangle]
pub extern "C" fn rust_pg_result_blob_copy(
    result: *const PGresult,
    row: c_int,
    col: c_int,
    out: *mut u8,
    out_len: usize,
) -> c_int {
    if result.is_null() || out.is_null() || out_len == 0 {
        return -1;
    }
    let is_null = unsafe { PQgetisnull(result, row, col) } != 0;
    if is_null {
        return -1;
    }
    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        return -1;
    }
    let len = unsafe { PQgetlength(result, row, col) };
    if len <= 0 {
        return -1;
    }
    let len_usize = len as usize;
    let copy_len = len_usize.min(out_len);
    unsafe {
        std::ptr::copy_nonoverlapping(val_ptr as *const u8, out, copy_len);
    }
    copy_len as c_int
}

#[no_mangle]
pub extern "C" fn rust_pg_result_length(result: *const PGresult, row: c_int, col: c_int) -> c_int {
    if result.is_null() {
        return -1;
    }
    let is_null = unsafe { PQgetisnull(result, row, col) } != 0;
    if is_null {
        return -1;
    }
    unsafe { PQgetlength(result, row, col) }
}

#[no_mangle]
pub extern "C" fn rust_pg_result_int(result: *const PGresult, row: c_int, col: c_int) -> c_int {
    if result.is_null() {
        return 0;
    }
    let is_null = unsafe { PQgetisnull(result, row, col) } != 0;
    if is_null {
        return 0;
    }
    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        return 0;
    }
    pg_text_to_int_impl(val_ptr)
}

#[no_mangle]
pub extern "C" fn rust_pg_result_int64(result: *const PGresult, row: c_int, col: c_int) -> i64 {
    if result.is_null() {
        return 0;
    }
    let is_null = unsafe { PQgetisnull(result, row, col) } != 0;
    if is_null {
        return 0;
    }
    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        return 0;
    }
    pg_text_to_int64_impl(val_ptr)
}

#[no_mangle]
pub extern "C" fn rust_pg_result_double(result: *const PGresult, row: c_int, col: c_int) -> f64 {
    if result.is_null() {
        return 0.0;
    }
    let is_null = unsafe { PQgetisnull(result, row, col) } != 0;
    if is_null {
        return 0.0;
    }
    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        return 0.0;
    }
    pg_text_to_double_impl(val_ptr)
}

#[no_mangle]
pub extern "C" fn rust_pg_result_text_transform_copy(
    result: *const PGresult,
    row: c_int,
    col: c_int,
    col_name: *const c_char,
    oid: c_uint,
    pg_sql: *const c_char,
    is_null: c_int,
    out: *mut c_char,
    out_len: usize,
    preview: *mut c_char,
    preview_len: usize,
    source_len_out: *mut usize,
) -> c_int {
    if result.is_null() || out.is_null() || out_len == 0 {
        if !out.is_null() && out_len > 0 {
            unsafe {
                *out = 0;
            }
        }
        if !preview.is_null() && preview_len > 0 {
            unsafe {
                *preview = 0;
            }
        }
        if !source_len_out.is_null() {
            unsafe {
                *source_len_out = 0;
            }
        }
        return -3;
    }

    if is_null != 0 {
        unsafe {
            *out = 0;
        }
        if !preview.is_null() && preview_len > 0 {
            unsafe {
                *preview = 0;
            }
        }
        if !source_len_out.is_null() {
            unsafe {
                *source_len_out = 0;
            }
        }
        return -2;
    }

    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        unsafe {
            *out = 0;
        }
        if !preview.is_null() && preview_len > 0 {
            unsafe {
                *preview = 0;
            }
        }
        if !source_len_out.is_null() {
            unsafe {
                *source_len_out = 0;
            }
        }
        return -2;
    }

    let len = unsafe { PQgetlength(result, row, col) };
    if len < 0 {
        unsafe {
            *out = 0;
        }
        if !preview.is_null() && preview_len > 0 {
            unsafe {
                *preview = 0;
            }
        }
        if !source_len_out.is_null() {
            unsafe {
                *source_len_out = 0;
            }
        }
        return -3;
    }
    let len_usize = len as usize;
    if !source_len_out.is_null() {
        unsafe {
            *source_len_out = len_usize;
        }
    }

    if !preview.is_null() && preview_len > 0 {
        let copy_len = len_usize.min(preview_len.saturating_sub(1));
        unsafe {
            std::ptr::copy_nonoverlapping(val_ptr as *const u8, preview as *mut u8, copy_len);
            *preview.add(copy_len) = 0;
        }
    }

    let transform_rc = rust_column_text_transform(
        col_name,
        oid,
        pg_sql,
        val_ptr,
        len_usize,
        out,
        out_len,
    );
    if transform_rc != 0 {
        return transform_rc;
    }

    let copy_len = len_usize.min(out_len.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(val_ptr as *const u8, out as *mut u8, copy_len);
        *out.add(copy_len) = 0;
    }
    0
}

#[no_mangle]
pub extern "C" fn rust_pg_result_value_ptr_len(
    result: *const PGresult,
    row: c_int,
    col: c_int,
    ptr_out: *mut *const c_char,
    len_out: *mut c_int,
    is_null_out: *mut c_int,
) -> c_int {
    if result.is_null() {
        return 0;
    }

    let is_null = unsafe { PQgetisnull(result, row, col) };
    if !is_null_out.is_null() {
        unsafe {
            *is_null_out = is_null;
        }
    }
    if is_null != 0 {
        if !ptr_out.is_null() {
            unsafe {
                *ptr_out = std::ptr::null();
            }
        }
        if !len_out.is_null() {
            unsafe {
                *len_out = 0;
            }
        }
        return 1;
    }

    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        if !ptr_out.is_null() {
            unsafe {
                *ptr_out = std::ptr::null();
            }
        }
        if !len_out.is_null() {
            unsafe {
                *len_out = 0;
            }
        }
        return 0;
    }
    let len = unsafe { PQgetlength(result, row, col) };
    if len < 0 {
        if !ptr_out.is_null() {
            unsafe {
                *ptr_out = std::ptr::null();
            }
        }
        if !len_out.is_null() {
            unsafe {
                *len_out = 0;
            }
        }
        return 0;
    }

    if !ptr_out.is_null() {
        unsafe {
            *ptr_out = val_ptr;
        }
    }
    if !len_out.is_null() {
        unsafe {
            *len_out = len;
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn rust_pg_result_col_oid(result: *const PGresult, col: c_int) -> c_uint {
    if result.is_null() {
        return 0;
    }
    unsafe { PQftype(result, col) }
}

#[no_mangle]
pub extern "C" fn rust_pg_result_col_table_oid(result: *const PGresult, col: c_int) -> c_uint {
    if result.is_null() {
        return 0;
    }
    unsafe { PQftable(result, col) }
}

#[no_mangle]
pub extern "C" fn rust_pg_decode_bytea(
    result: *const PGresult,
    row: c_int,
    col: c_int,
    ptr_out: *mut *mut u8,
    len_out: *mut c_int,
    is_hex_out: *mut c_int,
    is_null_out: *mut c_int,
) -> c_int {
    if result.is_null()
        || ptr_out.is_null()
        || len_out.is_null()
        || is_hex_out.is_null()
        || is_null_out.is_null()
    {
        return 0;
    }

    let is_null = unsafe { PQgetisnull(result, row, col) };
    unsafe {
        *is_null_out = is_null;
    }
    if is_null != 0 {
        unsafe {
            *ptr_out = std::ptr::null_mut();
            *len_out = 0;
            *is_hex_out = 0;
        }
        return 1;
    }

    let val_ptr = unsafe { PQgetvalue(result, row, col) };
    if val_ptr.is_null() {
        unsafe {
            *ptr_out = std::ptr::null_mut();
            *len_out = 0;
            *is_hex_out = 0;
        }
        return 0;
    }
    let len = unsafe { PQgetlength(result, row, col) };
    if len < 0 {
        unsafe {
            *ptr_out = std::ptr::null_mut();
            *len_out = 0;
            *is_hex_out = 0;
        }
        return 0;
    }

    let len_usize = len as usize;
    let bytes = unsafe { std::slice::from_raw_parts(val_ptr as *const u8, len_usize) };
    if len_usize < 2 || bytes[0] != b'\\' || bytes[1] != b'x' {
        unsafe {
            *ptr_out = val_ptr as *mut u8;
            *len_out = len;
            *is_hex_out = 0;
        }
        return 1;
    }

    let hex = &bytes[2..];
    if (hex.len() % 2) != 0 {
        unsafe {
            *ptr_out = std::ptr::null_mut();
            *len_out = 0;
            *is_hex_out = 1;
        }
        return 0;
    }
    let bin_len = hex.len() / 2;
    let alloc = unsafe { libc::malloc(bin_len + 1) } as *mut u8;
    if alloc.is_null() {
        unsafe {
            *ptr_out = std::ptr::null_mut();
            *len_out = 0;
            *is_hex_out = 1;
        }
        return 0;
    }
    let ok = rust_decode_hex_bytes(hex.as_ptr() as *const c_char, hex.len(), alloc, bin_len);
    if ok == 0 {
        unsafe {
            libc::free(alloc as *mut c_void);
            *ptr_out = std::ptr::null_mut();
            *len_out = 0;
            *is_hex_out = 1;
        }
        return 0;
    }

    unsafe {
        *ptr_out = alloc;
        *len_out = bin_len as c_int;
        *is_hex_out = 1;
    }
    1
}

#[no_mangle]
pub extern "C" fn rust_query_cache_store_from_pgresult(
    cache_key: u64,
    result: *const PGresult,
    num_rows: c_int,
    num_cols: c_int,
    pg_sql: *const c_char,
) {
    if result.is_null() || cache_key == 0 || num_rows <= 0 || num_cols <= 0 {
        return;
    }

    let nr = num_rows as usize;
    let nc = num_cols as usize;
    let total = nr.saturating_mul(nc);

    let mut col_types: Vec<u32> = Vec::with_capacity(nc);
    let mut col_names: Vec<*const c_char> = Vec::with_capacity(nc);
    for c in 0..nc {
        col_types.push(unsafe { PQftype(result, c as c_int) } as u32);
        col_names.push(unsafe { PQfname(result, c as c_int) });
    }

    let mut values: Vec<*const c_char> = Vec::with_capacity(total);
    let mut lengths: Vec<i32> = Vec::with_capacity(total);
    let mut is_null: Vec<i32> = Vec::with_capacity(total);

    for r in 0..nr {
        for c in 0..nc {
            let null_flag = unsafe { PQgetisnull(result, r as c_int, c as c_int) };
            if null_flag != 0 {
                is_null.push(1);
                values.push(std::ptr::null());
                lengths.push(0);
            } else {
                let val_ptr = unsafe { PQgetvalue(result, r as c_int, c as c_int) };
                let len = unsafe { PQgetlength(result, r as c_int, c as c_int) };
                is_null.push(0);
                values.push(val_ptr);
                lengths.push(if len < 0 { 0 } else { len });
            }
        }
    }

    rust_query_cache_store(
        cache_key,
        num_rows,
        num_cols,
        col_types.as_ptr(),
        col_names.as_ptr(),
        values.as_ptr(),
        lengths.as_ptr(),
        is_null.as_ptr(),
        pg_sql,
    );
}

#[no_mangle]
pub extern "C" fn rust_get_table_from_pgresult(
    result: *const PGresult,
    out_rows: *mut *mut *mut c_char,
    out_rows_count: *mut c_int,
    out_cols_count: *mut c_int,
) -> c_int {
    if result.is_null() || out_rows.is_null() || out_rows_count.is_null() || out_cols_count.is_null() {
        return 0;
    }

    let num_rows = unsafe { PQntuples(result) };
    let num_cols = unsafe { PQnfields(result) };
    if num_rows < 0 || num_cols <= 0 {
        unsafe {
            *out_rows = std::ptr::null_mut();
            *out_rows_count = 0;
            *out_cols_count = 0;
        }
        return 0;
    }

    let total = (num_rows as usize + 1).saturating_mul(num_cols as usize) + 1;
    let rows_ptr = unsafe {
        libc::malloc(total * std::mem::size_of::<*mut c_char>()) as *mut *mut c_char
    };
    if rows_ptr.is_null() {
        unsafe {
            *out_rows = std::ptr::null_mut();
            *out_rows_count = 0;
            *out_cols_count = 0;
        }
        return 0;
    }
    unsafe {
        std::ptr::write_bytes(rows_ptr, 0, total);
    }

    let mut ok = true;
    // Column names (header row)
    for c in 0..num_cols {
        let name = unsafe { PQfname(result, c) };
        if !name.is_null() {
            let bytes = unsafe { CStr::from_ptr(name).to_bytes() };
            let buf = unsafe { libc::malloc(bytes.len() + 1) } as *mut u8;
            if buf.is_null() {
                ok = false;
                break;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
                *buf.add(bytes.len()) = 0;
                *rows_ptr.add(c as usize) = buf as *mut c_char;
            }
        }
    }

    // Data rows
    if ok {
        for r in 0..num_rows {
            for c in 0..num_cols {
                let idx = (r as usize + 1) * (num_cols as usize) + (c as usize);
                let is_null = unsafe { PQgetisnull(result, r, c) };
                if is_null != 0 {
                    unsafe { *rows_ptr.add(idx) = std::ptr::null_mut() };
                    continue;
                }
                let val_ptr = unsafe { PQgetvalue(result, r, c) };
                if val_ptr.is_null() {
                    unsafe { *rows_ptr.add(idx) = std::ptr::null_mut() };
                    continue;
                }
                let len = unsafe { PQgetlength(result, r, c) };
                let len_usize = if len < 0 { 0 } else { len as usize };
                let buf = unsafe { libc::malloc(len_usize + 1) } as *mut u8;
                if buf.is_null() {
                    ok = false;
                    break;
                }
                unsafe {
                    if len_usize > 0 {
                        std::ptr::copy_nonoverlapping(val_ptr as *const u8, buf, len_usize);
                    }
                    *buf.add(len_usize) = 0;
                    *rows_ptr.add(idx) = buf as *mut c_char;
                }
            }
            if !ok {
                break;
            }
        }
    }

    if !ok {
        unsafe {
            for i in 0..total {
                let ptr = *rows_ptr.add(i);
                if !ptr.is_null() {
                    libc::free(ptr as *mut c_void);
                }
            }
            libc::free(rows_ptr as *mut c_void);
            *out_rows = std::ptr::null_mut();
            *out_rows_count = 0;
            *out_cols_count = 0;
        }
        return 0;
    }

    unsafe {
        *rows_ptr.add(total - 1) = std::ptr::null_mut();
        *out_rows = rows_ptr;
        *out_rows_count = num_rows;
        *out_cols_count = num_cols;
    }
    1
}

#[no_mangle]
pub extern "C" fn rust_pg_create_column_value(
    result: *const PGresult,
    current_row: c_int,
    num_rows: c_int,
    col_idx: c_int,
) -> c_int {
    if result.is_null() || current_row < 0 || current_row >= num_rows {
        SQLITE_NULL_CONST
    } else {
        let is_null = unsafe { PQgetisnull(result, current_row, col_idx) };
        if is_null != 0 {
            SQLITE_NULL_CONST
        } else {
            let oid = unsafe { PQftype(result, col_idx) };
            pg_oid_to_sqlite_type_impl(oid as u32)
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_pg_result_type_info(
    result: *const PGresult,
    row: c_int,
    col: c_int,
    oid_out: *mut c_uint,
    is_null_out: *mut c_int,
    sqlite_type_out: *mut c_int,
) -> c_int {
    if result.is_null() {
        return 0;
    }
    let is_null = unsafe { PQgetisnull(result, row, col) };
    if !is_null_out.is_null() {
        unsafe {
            *is_null_out = is_null;
        }
    }
    let oid = unsafe { PQftype(result, col) };
    if !oid_out.is_null() {
        unsafe {
            *oid_out = oid;
        }
    }
    if !sqlite_type_out.is_null() {
        let sqlite_type = if is_null != 0 {
            SQLITE_NULL_CONST
        } else {
            pg_oid_to_sqlite_type_impl(oid as u32)
        };
        unsafe {
            *sqlite_type_out = sqlite_type;
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn rust_pg_result_col_name(result: *const PGresult, col: c_int) -> *const c_char {
    if result.is_null() {
        return std::ptr::null();
    }
    unsafe { PQfname(result, col) }
}

#[no_mangle]
pub extern "C" fn rust_step_clear_row_caches(
    cached_text: *mut *mut c_char,
    cached_blob: *mut *mut c_void,
    cached_blob_len: *mut c_int,
    decoded_blobs: *mut *mut c_void,
    decoded_blob_lens: *mut c_int,
    max_params: c_int,
    cached_row: *mut c_int,
    decoded_blob_row: *mut c_int,
) {
    if max_params <= 0 {
        if !cached_row.is_null() {
            unsafe { *cached_row = -1 };
        }
        if !decoded_blob_row.is_null() {
            unsafe { *decoded_blob_row = -1 };
        }
        return;
    }

    for i in 0..(max_params as isize) {
        unsafe {
            if !cached_text.is_null() {
                let slot = cached_text.offset(i);
                let ptr = *slot;
                if !ptr.is_null() {
                    libc::free(ptr as *mut c_void);
                    *slot = std::ptr::null_mut();
                }
            }

            if !cached_blob.is_null() {
                let slot = cached_blob.offset(i);
                let ptr = *slot;
                if !ptr.is_null() {
                    libc::free(ptr);
                    *slot = std::ptr::null_mut();
                }
            }

            if !cached_blob_len.is_null() {
                *cached_blob_len.offset(i) = 0;
            }

            if !decoded_blobs.is_null() {
                let slot = decoded_blobs.offset(i);
                let ptr = *slot;
                if !ptr.is_null() {
                    libc::free(ptr);
                    *slot = std::ptr::null_mut();
                }
            }

            if !decoded_blob_lens.is_null() {
                *decoded_blob_lens.offset(i) = 0;
            }
        }
    }

    if !cached_row.is_null() {
        unsafe { *cached_row = -1 };
    }
    if !decoded_blob_row.is_null() {
        unsafe { *decoded_blob_row = -1 };
    }
}

#[no_mangle]
pub extern "C" fn rust_pg_udt_to_sqlite_decltype(ptr: *const c_char) -> *const c_char {
    pg_udt_to_sqlite_decltype_impl(cstr_to_str(ptr))
}

#[no_mangle]
pub extern "C" fn rust_normalize_sqlite_decltype(ptr: *const c_char) -> *const c_char {
    normalize_sqlite_decltype_impl(cstr_to_str(ptr))
}

#[no_mangle]
pub extern "C" fn rust_prepare_simple_hash(ptr: *const c_char, max_len: i32) -> u32 {
    let s = cstr_to_str(ptr).unwrap_or("");
    prepare_simple_hash(s, max_len)
}

#[no_mangle]
pub extern "C" fn rust_prepare_time_ms() -> u64 {
    prepare_time_ms()
}

#[no_mangle]
pub extern "C" fn rust_prepare_query_loop_tick(
    sql: *const c_char,
    count_out: *mut c_int,
    elapsed_ms_out: *mut u64,
) -> c_int {
    if sql.is_null() {
        return 0;
    }
    let s = cstr_to_str(sql).unwrap_or("");
    let (detected, count, elapsed) = match prepare_query_loop_tick(s) {
        Some((count, elapsed)) => (1, count, elapsed),
        None => (0, 0, 0),
    };

    if !count_out.is_null() {
        unsafe {
            *count_out = count;
        }
    }
    if !elapsed_ms_out.is_null() {
        unsafe {
            *elapsed_ms_out = elapsed;
        }
    }
    detected
}

#[no_mangle]
pub extern "C" fn rust_maybe_alias_collection_sync_aggregates(
    sqlite_sql: *const c_char,
    pg_sql: *const c_char,
) -> *mut c_char {
    if sqlite_sql.is_null() || pg_sql.is_null() {
        return std::ptr::null_mut();
    }
    let sqlite_sql = match unsafe { CStr::from_ptr(sqlite_sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let pg_sql = match unsafe { CStr::from_ptr(pg_sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let Some(out) = alias_collection_sync_aggregates(sqlite_sql, pg_sql) else {
        return std::ptr::null_mut();
    };
    match std::ffi::CString::new(out) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rust_free_cstring(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = std::ffi::CString::from_raw(ptr);
    }
}

#[no_mangle]
pub extern "C" fn rust_strip_collate_icu_root(sql: *const c_char) -> *mut c_char {
    if sql.is_null() {
        return std::ptr::null_mut();
    }
    let sql = match unsafe { CStr::from_ptr(sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let Some(out) = strip_collate_icu_root(sql) else {
        return std::ptr::null_mut();
    };
    match std::ffi::CString::new(out) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rust_is_junk_metadata_insert(sql: *const c_char) -> c_int {
    if sql.is_null() {
        return 0;
    }
    let sql = match unsafe { CStr::from_ptr(sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(is_junk_metadata_insert_impl(sql))
}

#[no_mangle]
pub extern "C" fn rust_is_library_db_path(path: *const c_char) -> c_int {
    if path.is_null() {
        return 0;
    }
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(is_library_db_path_impl(path))
}

#[no_mangle]
pub extern "C" fn rust_is_library_or_blobs_db_path(path: *const c_char) -> c_int {
    if path.is_null() {
        return 0;
    }
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(is_library_or_blobs_db_path_impl(path))
}

#[no_mangle]
pub extern "C" fn rust_is_blobs_db_path(path: *const c_char) -> c_int {
    if path.is_null() {
        return 0;
    }
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(is_blobs_db_path_impl(path))
}

#[no_mangle]
pub extern "C" fn rust_trace_list_contains_idx(list: *const c_char, idx: c_int) -> c_int {
    if list.is_null() {
        return 0;
    }
    let list = match unsafe { CStr::from_ptr(list) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(list_contains_idx(list, idx))
}

#[no_mangle]
pub extern "C" fn rust_trace_list_any_token_in_haystack(
    list: *const c_char,
    haystack: *const c_char,
) -> c_int {
    if list.is_null() || haystack.is_null() {
        return 0;
    }
    let list = match unsafe { CStr::from_ptr(list) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let haystack = match unsafe { CStr::from_ptr(haystack) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(list_any_token_in_haystack(list, haystack))
}

#[no_mangle]
pub extern "C" fn rust_env_truthy(value: *const c_char) -> c_int {
    let s = unsafe { cstr_to_str_or_empty(value) };
    if s.is_empty() {
        return 0;
    }
    matches!(s.as_bytes()[0], b'1' | b'y' | b'Y' | b't' | b'T') as c_int
}

#[no_mangle]
pub extern "C" fn rust_simplify_fts_for_sqlite(sql: *const c_char) -> *mut c_char {
    if sql.is_null() {
        return std::ptr::null_mut();
    }
    let sql = match unsafe { CStr::from_ptr(sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let Some(out) = simplify_fts_for_sqlite(sql) else {
        return std::ptr::null_mut();
    };
    match std::ffi::CString::new(out) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rust_add_if_not_exists_for_sqlite_ddl(sql: *const c_char) -> *mut c_char {
    if sql.is_null() {
        return std::ptr::null_mut();
    }
    let sql = match unsafe { CStr::from_ptr(sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let Some(out) = add_if_not_exists_for_sqlite_ddl(sql) else {
        return std::ptr::null_mut();
    };
    match std::ffi::CString::new(out) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rust_format_epoch_to_datetime_utc(
    epoch: i64,
    out: *mut c_char,
    out_len: usize,
) -> c_int {
    format_epoch_to_datetime_utc_impl(epoch, out, out_len)
}

#[no_mangle]
pub extern "C" fn rust_contains_binary_bytes(data: *const u8, len: usize) -> c_int {
    if data.is_null() || len == 0 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    i32::from(contains_binary_bytes_impl(bytes))
}

#[no_mangle]
pub extern "C" fn rust_bytes_to_pg_hex(data: *const u8, len: usize) -> *mut c_char {
    if data.is_null() || len == 0 {
        return match std::ffi::CString::new("") {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        };
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    match std::ffi::CString::new(bytes_to_pg_hex_impl(bytes)) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rust_is_related_items_query(pg_sql: *const c_char) -> c_int {
    if pg_sql.is_null() {
        return 0;
    }
    let pg_sql = match unsafe { CStr::from_ptr(pg_sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(is_related_items_query_impl(pg_sql))
}

#[no_mangle]
pub extern "C" fn rust_should_mask_collection_metadata_type(
    pg_sql: *const c_char,
    col_name: *const c_char,
    raw_val: i64,
) -> c_int {
    if pg_sql.is_null() || col_name.is_null() {
        return 0;
    }
    let pg_sql = match unsafe { CStr::from_ptr(pg_sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let col_name = match unsafe { CStr::from_ptr(col_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    i32::from(should_mask_collection_metadata_type_impl(pg_sql, col_name, raw_val))
}

#[no_mangle]
pub extern "C" fn rust_find_insert_column_index(
    sql: *const c_char,
    column_name: *const c_char,
) -> c_int {
    if sql.is_null() || column_name.is_null() {
        return -1;
    }
    let sql = match unsafe { CStr::from_ptr(sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let column_name = match unsafe { CStr::from_ptr(column_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    find_insert_column_index_impl(sql, column_name)
}

#[no_mangle]
pub extern "C" fn rust_pg_oid_to_sqlite_type(oid: c_uint) -> c_int {
    pg_oid_to_sqlite_type_impl(oid)
}

#[no_mangle]
pub extern "C" fn rust_pg_text_to_int(value: *const c_char) -> c_int {
    pg_text_to_int_impl(value)
}

#[no_mangle]
pub extern "C" fn rust_pg_text_to_int64(value: *const c_char) -> i64 {
    pg_text_to_int64_impl(value)
}

#[no_mangle]
pub extern "C" fn rust_pg_text_to_double(value: *const c_char) -> f64 {
    pg_text_to_double_impl(value)
}

#[no_mangle]
pub extern "C" fn rust_read_first_line_trim_to_buf(
    path: *const c_char,
    out: *mut c_char,
    out_len: usize,
) -> c_int {
    if path.is_null() || out.is_null() || out_len < 2 {
        return 0;
    }
    unsafe {
        *out = 0;
    }

    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return 0;
    }
    let Some(trimmed) = trim_first_line(&line) else {
        return 0;
    };

    let bytes = trimmed.as_bytes();
    let n = bytes.len().min(out_len - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
        *out.add(n) = 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn rust_trace_prepare_sql_ok(_sql: *const c_char) -> c_int {
    // Keep current behavior from C: prepare SQL tracing is force-enabled.
    1
}

#[no_mangle]
pub extern "C" fn rust_load_badcast_config(
    enabled_out: *mut c_int,
    idx_out: *mut c_char,
    idx_len: usize,
    thread_out: *mut c_char,
    thread_len: usize,
    sql_out: *mut c_char,
    sql_len: usize,
    col_out: *mut c_char,
    col_len: usize,
) -> c_int {
    let enabled = if let Some(v) = getenv_nonempty("PLEX_PG_TRACE_BADCAST") {
        i32::from(v != "0")
    } else if let Some(v) = getenv_nonempty("PLEX_PG_LOG_LEVEL") {
        i32::from(v.eq_ignore_ascii_case("ERROR"))
    } else {
        0
    };

    let idx = getenv_nonempty("PLEX_PG_TRACE_BADCAST_IDX")
        .or_else(|| read_first_line_trimmed("/tmp/plex_pg_trace_badcast_idx"));
    let thread = getenv_nonempty("PLEX_PG_TRACE_BADCAST_THREAD")
        .or_else(|| read_first_line_trimmed("/tmp/plex_pg_trace_badcast_thread"));
    let sql = getenv_nonempty("PLEX_PG_TRACE_BADCAST_SQL_CONTAINS")
        .or_else(|| read_first_line_trimmed("/tmp/plex_pg_trace_badcast_sql_contains"));
    let col = getenv_nonempty("PLEX_PG_TRACE_BADCAST_COL_CONTAINS")
        .or_else(|| read_first_line_trimmed("/tmp/plex_pg_trace_badcast_col_contains"));

    if !enabled_out.is_null() {
        unsafe {
            *enabled_out = enabled;
        }
    }
    write_buf(idx_out, idx_len, idx.as_deref());
    write_buf(thread_out, thread_len, thread.as_deref());
    write_buf(sql_out, sql_len, sql.as_deref());
    write_buf(col_out, col_len, col.as_deref());

    1
}

#[no_mangle]
pub extern "C" fn rust_validate_utf8(ptr: *const c_char, len: usize) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
    i32::from(std::str::from_utf8(bytes).is_ok())
}

#[no_mangle]
pub extern "C" fn rust_rewrite_server_library_uri(
    input: *const c_char,
    out: *mut c_char,
    out_len: usize,
) -> i32 {
    if input.is_null() || out.is_null() || out_len < 16 {
        return 0;
    }

    let input_bytes = unsafe { CStr::from_ptr(input).to_bytes() };
    let out_cap = out_len.saturating_sub(1);
    let Some(out_buf) = rewrite_server_library_uri_bytes(input_bytes, out_cap) else {
        return 0;
    };

    let n = out_buf.len().min(out_cap);
    unsafe {
        if n > 0 {
            std::ptr::copy_nonoverlapping(out_buf.as_ptr(), out as *mut u8, n);
        }
        *out.add(n) = 0;
    }

    1
}

#[no_mangle]
pub extern "C" fn rust_normalize_sql_literals(sql: *const c_char) -> *mut RustNormalizedSql {
    if sql.is_null() {
        return std::ptr::null_mut();
    }
    let raw = match unsafe { CStr::from_ptr(sql) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let Some((normalized_sql, params)) = normalize_sql_literals_impl(raw) else {
        return std::ptr::null_mut();
    };

    let normalized_sql = match std::ffi::CString::new(normalized_sql) {
        Ok(s) => s.into_raw(),
        Err(_) => return std::ptr::null_mut(),
    };

    let mut param_ptrs: Vec<*mut c_char> = Vec::with_capacity(params.len());
    for p in params {
        match std::ffi::CString::new(p) {
            Ok(s) => param_ptrs.push(s.into_raw()),
            Err(_) => {
                for ptr in param_ptrs {
                    if !ptr.is_null() {
                        unsafe {
                            let _ = std::ffi::CString::from_raw(ptr);
                        }
                    }
                }
                unsafe {
                    let _ = std::ffi::CString::from_raw(normalized_sql);
                }
                return std::ptr::null_mut();
            }
        }
    }

    let mut boxed_params = param_ptrs.into_boxed_slice();
    let param_values = boxed_params.as_mut_ptr();
    let param_count = boxed_params.len() as c_int;
    std::mem::forget(boxed_params);

    Box::into_raw(Box::new(RustNormalizedSql {
        normalized_sql,
        param_values,
        param_count,
    }))
}

#[no_mangle]
pub extern "C" fn rust_free_normalized_sql(n: *mut RustNormalizedSql) {
    if n.is_null() {
        return;
    }

    let n = unsafe { Box::from_raw(n) };
    if !n.normalized_sql.is_null() {
        unsafe {
            let _ = std::ffi::CString::from_raw(n.normalized_sql);
        }
    }

    if !n.param_values.is_null() && n.param_count > 0 {
        let len = n.param_count as usize;
        let slice_ptr = std::ptr::slice_from_raw_parts_mut(n.param_values, len);
        let params = unsafe { Box::from_raw(slice_ptr) };
        for p in params.iter().copied() {
            if !p.is_null() {
                unsafe {
                    let _ = std::ffi::CString::from_raw(p);
                }
            }
        }
    }
}


#[cfg(test)]
#[path = "db_interpose_helpers_tests.rs"]
mod db_interpose_helpers_tests;
