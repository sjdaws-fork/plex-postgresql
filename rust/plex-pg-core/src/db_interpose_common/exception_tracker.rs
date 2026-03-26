use super::*;

const MAX_EXCEPTION_TYPES: usize = 64;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ExceptionTypeTracker {
    pub(crate) type_name: *const c_char,
    pub(crate) count: c_int,
    pub(crate) logged_with_trace: c_int,
}

static mut EXCEPTION_TYPES: [ExceptionTypeTracker; MAX_EXCEPTION_TYPES] = [ExceptionTypeTracker {
    type_name: ptr::null(),
    count: 0,
    logged_with_trace: 0,
}; MAX_EXCEPTION_TYPES];
static mut EXCEPTION_TYPE_COUNT: c_int = 0;

pub(super) unsafe fn get_exception_tracker_impl(
    type_name: *const c_char,
) -> *mut ExceptionTypeTracker {
    let mut exc_guard = PthreadMutexGuard::lock(ptr::addr_of_mut!(exception_tracker_mutex));

    for i in 0..EXCEPTION_TYPE_COUNT {
        let tracker = &mut EXCEPTION_TYPES[i as usize] as *mut ExceptionTypeTracker;
        let tracker_ref = &mut *tracker;
        if tracker_ref.type_name == type_name
            || (!tracker_ref.type_name.is_null()
                && !type_name.is_null()
                && libc::strcmp(tracker_ref.type_name, type_name) == 0)
        {
            tracker_ref.count += 1;
            exc_guard.unlock();
            return tracker;
        }
    }

    if (EXCEPTION_TYPE_COUNT as usize) < MAX_EXCEPTION_TYPES {
        let tracker =
            &mut EXCEPTION_TYPES[EXCEPTION_TYPE_COUNT as usize] as *mut ExceptionTypeTracker;
        (*tracker).type_name = type_name;
        (*tracker).count = 1;
        (*tracker).logged_with_trace = 0;
        EXCEPTION_TYPE_COUNT += 1;
        exc_guard.unlock();
        return tracker;
    }

    exc_guard.unlock();
    ptr::null_mut()
}

pub(super) fn reset_exception_tracking_impl() {
    total_exception_count.store(0, Ordering::SeqCst);
    unsafe {
        EXCEPTION_TYPE_COUNT = 0;
    }
}
