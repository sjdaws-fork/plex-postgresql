use std::os::raw::{c_char, c_int};

use crate::ffi_types::{sqlite3_stmt, PgConnection, PgStmt};

use super::super::support::{orig_step, sql_translate, sql_translation_free};
use super::super::{
    note_pg_conn_error, SQLITE_DONE, SQLITE_ROW, STEP_RESULT_DONE, STEP_RESULT_ERROR,
    STEP_RESULT_FALLBACK, STEP_RESULT_ROW,
};
use super::free_expanded_sql;

pub(super) unsafe fn handle_cached_read(
    p_stmt: *mut sqlite3_stmt,
    pg_conn: *mut PgConnection,
    sql: *const c_char,
    expanded_sql: *mut c_char,
) -> c_int {
    let cached_read_conn =
        crate::db_interpose_step_write_utils::rust_step_pick_thread_connection(pg_conn);
    let mut cached_branch_rc = STEP_RESULT_FALLBACK;
    let cached = crate::pg_statement::rust_cached_stmt_find(p_stmt as usize) as *mut PgStmt;
    let sqlite_result = orig_step(p_stmt);

    if sqlite_result == SQLITE_ROW || sqlite_result == SQLITE_DONE {
        let mut cached_rc = STEP_RESULT_DONE;
        if crate::db_interpose_step_cached_read_utils::rust_step_cached_read_finalize_advance(
            cached,
            expanded_sql,
            &mut cached_rc,
        ) != 0
        {
            return cached_rc;
        }

        let mut trans = sql_translate(sql);
        if trans.success != 0 && !trans.sql.is_null() {
            let new_stmt =
                crate::db_interpose_step_cached_read_utils::rust_step_cached_read_prepare_stmt(
                    cached,
                    cached_read_conn,
                    sql,
                    p_stmt,
                    trans.sql,
                );
            if !new_stmt.is_null() {
                let mut conn_error = 0;
                cached_branch_rc =
                    crate::db_interpose_step_cached_read_utils::rust_step_cached_read_execute(
                        new_stmt,
                        cached_read_conn,
                        sql,
                        trans.sql,
                        &mut conn_error,
                    );
                if conn_error != 0 && cached_branch_rc == STEP_RESULT_ERROR {
                    note_pg_conn_error();
                }
            }
        }
        sql_translation_free(&mut trans as *mut _);
    }

    if cached_branch_rc == STEP_RESULT_ROW
        || cached_branch_rc == STEP_RESULT_DONE
        || cached_branch_rc == STEP_RESULT_ERROR
    {
        free_expanded_sql(expanded_sql);
        return cached_branch_rc;
    }

    free_expanded_sql(expanded_sql);
    sqlite_result
}
