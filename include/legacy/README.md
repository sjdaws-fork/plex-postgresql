# Legacy Shim Headers

This directory holds the legacy C-facing shim headers that used to live under `src/`.

The shim implementation now lives in Rust. These headers remain only as a compatibility
boundary for:
- historical include paths
- ABI/reference documentation for exported C symbols and shared layouts
- generated bridge headers such as `include/plex_pg_core_ffi.h`

Rules:
- add new runtime logic in Rust, not in these headers
- prefer `include/plex_pg_core_ffi.h` and `include/sql_translator.h` for stable ABI usage
- treat `src/*.h` and `src/interpose/*.h` as compatibility wrappers only
