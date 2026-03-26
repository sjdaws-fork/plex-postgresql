use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::{Once, OnceLock};

use crate::pg_config::PgEnvConfig;

use super::config::load_conn_config;

pub(crate) type ConnConfig = PgEnvConfig;

static CONN_CONFIG: OnceLock<PgEnvConfig> = OnceLock::new();
pub(crate) static CLIENT_INIT: Once = Once::new();

pub(super) fn conn_config() -> &'static ConnConfig {
    CONN_CONFIG.get_or_init(load_conn_config)
}

pub(super) fn write_str_to_cbuf(buf: &mut [c_char], src: &str) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(buf.len().saturating_sub(1));
    for i in 0..len {
        buf[i] = bytes[i] as c_char;
    }
    if !buf.is_empty() {
        buf[len] = 0;
    }
}

pub(super) fn cbuf_to_string(buf: &[c_char]) -> String {
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}
