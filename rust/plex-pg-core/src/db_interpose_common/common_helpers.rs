use super::*;

#[no_mangle]
pub extern "C" fn rust_get_type_name(tinfo: *mut c_void) -> *const c_char {
    if tinfo.is_null() {
        return UNKNOWN_STR.as_ptr() as *const c_char;
    }
    unsafe {
        let name_ptr = (tinfo as *const *const c_char).add(1);
        let name = *name_ptr;
        if name.is_null() {
            UNKNOWN_STR.as_ptr() as *const c_char
        } else {
            name
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_rewrite_blobs_schema_migrations(
    _sql: *const c_char,
    _db_path: *const c_char,
) -> *mut c_char {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn rust_simple_str_replace(
    str_ptr: *const c_char,
    old_ptr: *const c_char,
    new_ptr: *const c_char,
) -> *mut c_char {
    if str_ptr.is_null() || old_ptr.is_null() || new_ptr.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let pos = libc::strstr(str_ptr, old_ptr);
        if pos.is_null() {
            return ptr::null_mut();
        }

        let old_len = libc::strlen(old_ptr);
        let new_len = libc::strlen(new_ptr);
        let str_len = libc::strlen(str_ptr);
        let result_len = str_len - old_len + new_len;

        let result = libc::malloc(result_len + 1) as *mut c_char;
        if result.is_null() {
            return ptr::null_mut();
        }

        let prefix_len = (pos as usize).wrapping_sub(str_ptr as usize);
        libc::memcpy(result as *mut c_void, str_ptr as *const c_void, prefix_len);
        libc::memcpy(
            result.add(prefix_len) as *mut c_void,
            new_ptr as *const c_void,
            new_len,
        );
        libc::strcpy(result.add(prefix_len + new_len), pos.add(old_len));

        result
    }
}

#[no_mangle]
pub extern "C" fn get_type_name(tinfo: *mut c_void) -> *const c_char {
    rust_get_type_name(tinfo)
}

#[no_mangle]
pub extern "C" fn rewrite_blobs_schema_migrations(
    sql: *const c_char,
    db_path: *const c_char,
) -> *mut c_char {
    rust_rewrite_blobs_schema_migrations(sql, db_path)
}

#[no_mangle]
pub extern "C" fn simple_str_replace(
    str_ptr: *const c_char,
    old_ptr: *const c_char,
    new_ptr: *const c_char,
) -> *mut c_char {
    rust_simple_str_replace(str_ptr, old_ptr, new_ptr)
}
