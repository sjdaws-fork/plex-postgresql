use std::os::raw::c_uint;
use std::ptr;

pub(crate) const MAX_FAKE_VALUES: usize = 4096;
pub(crate) const PG_FAKE_VALUE_MAGIC: u32 = 0x50475641;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PgFakeValue {
    pub magic: u32,
    pub pg_stmt: *mut libc::c_void,
    pub col_idx: libc::c_int,
    pub row_idx: libc::c_int,
    pub owner_thread: libc::pthread_t,
}

#[no_mangle]
pub static mut fake_value_pool: [PgFakeValue; MAX_FAKE_VALUES] = [PgFakeValue {
    magic: 0,
    pg_stmt: ptr::null_mut(),
    col_idx: 0,
    row_idx: 0,
    owner_thread: 0 as libc::pthread_t,
}; MAX_FAKE_VALUES];

#[no_mangle]
pub static mut fake_value_next: c_uint = 0;

#[no_mangle]
pub static mut fake_value_mutex: libc::pthread_mutex_t = libc::PTHREAD_MUTEX_INITIALIZER;

pub fn rust_pg_check_fake_value(
    p_val: *mut crate::ffi_types::sqlite3_value,
) -> *mut PgFakeValue {
    if p_val.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let ptr_val = p_val as usize;
        let pool_ptr = ptr::addr_of!(fake_value_pool) as *const PgFakeValue;
        let pool_start = pool_ptr as usize;
        let pool_end = pool_ptr.add(MAX_FAKE_VALUES) as usize;
        if ptr_val >= pool_start && ptr_val < pool_end {
            let fake = p_val as *mut PgFakeValue;
            if (*fake).magic == PG_FAKE_VALUE_MAGIC {
                return fake;
            }
        }
    }
    ptr::null_mut()
}
