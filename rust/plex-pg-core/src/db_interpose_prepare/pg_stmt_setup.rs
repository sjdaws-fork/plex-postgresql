use super::*;

pub(super) unsafe fn copy_param_names(pg_stmt: *mut PgStmt, trans: &SqlTranslation) {
    if pg_stmt.is_null() {
        return;
    }
    if trans.param_names.is_null() || trans.param_count <= 0 {
        return;
    }
    let s = &mut *pg_stmt;
    let count = trans.param_count as usize;
    let alloc = libc::malloc(count * std::mem::size_of::<*mut c_char>()) as *mut *mut c_char;
    if alloc.is_null() {
        return;
    }
    for i in 0..count {
        let name_ptr = *trans.param_names.add(i);
        *alloc.add(i) = if name_ptr.is_null() {
            ptr::null_mut()
        } else {
            libc::strdup(name_ptr)
        };
    }
    s.param_names = alloc;
}

pub(super) unsafe fn apply_prepared_stmt_settings(pg_stmt: *mut PgStmt) {
    if pg_stmt.is_null() {
        return;
    }
    let s = &mut *pg_stmt;
    if s.pg_sql.is_null() {
        return;
    }
    s.sql_hash = pg_hash_sql(s.pg_sql);
    if !prepared_statements_disabled() {
        libc::snprintf(
            s.stmt_name.as_mut_ptr(),
            s.stmt_name.len(),
            b"ps_%llx\0".as_ptr() as *const c_char,
            s.sql_hash as libc::c_ulonglong,
        );
        s.use_prepared = 1;
    } else {
        s.use_prepared = 0;
        s.stmt_name[0] = 0;
    }
}
