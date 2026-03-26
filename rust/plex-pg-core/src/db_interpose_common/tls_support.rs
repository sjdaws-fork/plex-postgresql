use std::mem::size_of;
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::ptr;
use std::sync::Once;

#[repr(C)]
struct TlsState {
    in_interpose_call: c_int,
    prepare_v2_depth: c_int,
    in_resolve_tables: c_int,
    value_type_calls: c_long,
    column_type_calls: c_long,
    last_query: *const c_char,
}

static TLS_INIT: Once = Once::new();
static mut TLS_KEY: libc::pthread_key_t = 0;
static mut TLS_FALLBACK: TlsState = TlsState {
    in_interpose_call: 0,
    prepare_v2_depth: 0,
    in_resolve_tables: 0,
    value_type_calls: 0,
    column_type_calls: 0,
    last_query: ptr::null(),
};

#[cfg(target_os = "macos")]
unsafe extern "C" {
    static mut __stderrp: *mut libc::FILE;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    static mut stderr: *mut libc::FILE;
}

#[inline]
pub(crate) unsafe fn stderr_ptr() -> *mut libc::FILE {
    #[cfg(target_os = "macos")]
    {
        __stderrp
    }
    #[cfg(not(target_os = "macos"))]
    {
        stderr
    }
}

unsafe extern "C" fn tls_destructor(ptr: *mut c_void) {
    if !ptr.is_null() {
        libc::free(ptr);
    }
}

fn tls_key() -> libc::pthread_key_t {
    TLS_INIT.call_once(|| unsafe {
        let mut key: libc::pthread_key_t = 0;
        if libc::pthread_key_create(&mut key as *mut _, Some(tls_destructor)) == 0 {
            TLS_KEY = key;
        } else {
            TLS_KEY = 0;
        }
    });
    unsafe { TLS_KEY }
}

unsafe fn tls_state() -> *mut TlsState {
    let key = tls_key();
    if key == 0 {
        return ptr::addr_of_mut!(TLS_FALLBACK);
    }
    let ptr_val = libc::pthread_getspecific(key) as *mut TlsState;
    if !ptr_val.is_null() {
        return ptr_val;
    }
    let new = libc::calloc(1, size_of::<TlsState>()) as *mut TlsState;
    if new.is_null() {
        return ptr::addr_of_mut!(TLS_FALLBACK);
    }
    libc::pthread_setspecific(key, new as *mut c_void);
    new
}

pub(crate) fn tls_in_interpose_call_ptr() -> *mut c_int {
    unsafe { ptr::addr_of_mut!((*tls_state()).in_interpose_call) }
}

pub(crate) fn tls_prepare_v2_depth_ptr() -> *mut c_int {
    unsafe { ptr::addr_of_mut!((*tls_state()).prepare_v2_depth) }
}

pub(crate) fn tls_in_resolve_tables_ptr() -> *mut c_int {
    unsafe { ptr::addr_of_mut!((*tls_state()).in_resolve_tables) }
}

pub(crate) fn tls_value_type_calls_ptr() -> *mut c_long {
    unsafe { ptr::addr_of_mut!((*tls_state()).value_type_calls) }
}

pub(crate) fn tls_column_type_calls_ptr() -> *mut c_long {
    unsafe { ptr::addr_of_mut!((*tls_state()).column_type_calls) }
}

pub(crate) fn tls_last_query_ptr() -> *mut *const c_char {
    unsafe { ptr::addr_of_mut!((*tls_state()).last_query) }
}
