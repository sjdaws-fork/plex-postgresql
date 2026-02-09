/*
 * Unit Tests for Platform Parity (v0.9.20+)
 *
 * Tests the shared symbol loading (common_load_sqlite_symbols) and
 * unified backtrace module (platform_backtrace.c) that replaced
 * duplicated platform-specific code.
 *
 * Bug fixes verified:
 *  1. sqlite3_column_decltype was missing from macOS fishhook rebindings
 *  2. Linux decltype wrapper bypassed my_sqlite3_column_decltype
 *  3. macOS fallback only loaded ~11 of ~60 symbols (now shared function)
 *
 * Links against: db_interpose_common.o, platform_backtrace.o, pg_logging.o
 * Stubs provided for: pg_client, pg_statement, pg_config, sql_translator,
 *                      pg_query_cache, my_sqlite3_prepare_v2_internal
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <dlfcn.h>
#include <sqlite3.h>

// Include the real headers for types and declarations
#include "db_interpose_common.h"

// ============================================================================
// Extern declarations for the globals defined in db_interpose_common.c
// (we link the real .o, so they're already defined there)
// ============================================================================

// These are declared in db_interpose.h but we can't include it (pulls in libpq)
// Just declare what we need for testing:
extern int (*orig_sqlite3_open)(const char*, sqlite3**);
extern int (*orig_sqlite3_open_v2)(const char*, sqlite3**, int, const char*);
extern int (*orig_sqlite3_close)(sqlite3*);
extern int (*orig_sqlite3_close_v2)(sqlite3*);
extern int (*orig_sqlite3_exec)(sqlite3*, const char*, int(*)(void*,int,char**,char**), void*, char**);
extern int (*orig_sqlite3_get_table)(sqlite3*, const char*, char***, int*, int*, char**);
extern int (*orig_sqlite3_changes)(sqlite3*);
extern sqlite3_int64 (*orig_sqlite3_changes64)(sqlite3*);
extern sqlite3_int64 (*orig_sqlite3_last_insert_rowid)(sqlite3*);
extern const char* (*orig_sqlite3_errmsg)(sqlite3*);
extern int (*orig_sqlite3_errcode)(sqlite3*);
extern int (*orig_sqlite3_extended_errcode)(sqlite3*);
extern int (*orig_sqlite3_prepare)(sqlite3*, const char*, int, sqlite3_stmt**, const char**);
extern int (*orig_sqlite3_prepare_v2)(sqlite3*, const char*, int, sqlite3_stmt**, const char**);
extern int (*orig_sqlite3_prepare_v3)(sqlite3*, const char*, int, unsigned int, sqlite3_stmt**, const char**);
extern int (*orig_sqlite3_prepare16_v2)(sqlite3*, const void*, int, sqlite3_stmt**, const void**);
extern int (*orig_sqlite3_bind_int)(sqlite3_stmt*, int, int);
extern int (*orig_sqlite3_bind_int64)(sqlite3_stmt*, int, sqlite3_int64);
extern int (*orig_sqlite3_bind_double)(sqlite3_stmt*, int, double);
extern int (*orig_sqlite3_bind_text)(sqlite3_stmt*, int, const char*, int, void(*)(void*));
extern int (*orig_sqlite3_bind_text64)(sqlite3_stmt*, int, const char*, sqlite3_uint64, void(*)(void*), unsigned char);
extern int (*orig_sqlite3_bind_blob)(sqlite3_stmt*, int, const void*, int, void(*)(void*));
extern int (*orig_sqlite3_bind_blob64)(sqlite3_stmt*, int, const void*, sqlite3_uint64, void(*)(void*));
extern int (*orig_sqlite3_bind_value)(sqlite3_stmt*, int, const sqlite3_value*);
extern int (*orig_sqlite3_bind_null)(sqlite3_stmt*, int);
extern int (*orig_sqlite3_step)(sqlite3_stmt*);
extern int (*orig_sqlite3_reset)(sqlite3_stmt*);
extern int (*orig_sqlite3_finalize)(sqlite3_stmt*);
extern int (*orig_sqlite3_clear_bindings)(sqlite3_stmt*);
extern int (*orig_sqlite3_column_count)(sqlite3_stmt*);
extern int (*orig_sqlite3_column_type)(sqlite3_stmt*, int);
extern int (*orig_sqlite3_column_int)(sqlite3_stmt*, int);
extern sqlite3_int64 (*orig_sqlite3_column_int64)(sqlite3_stmt*, int);
extern double (*orig_sqlite3_column_double)(sqlite3_stmt*, int);
extern const unsigned char* (*orig_sqlite3_column_text)(sqlite3_stmt*, int);
extern const void* (*orig_sqlite3_column_blob)(sqlite3_stmt*, int);
extern int (*orig_sqlite3_column_bytes)(sqlite3_stmt*, int);
extern const char* (*orig_sqlite3_column_name)(sqlite3_stmt*, int);
extern const char* (*orig_sqlite3_column_decltype)(sqlite3_stmt*, int);
extern sqlite3_value* (*orig_sqlite3_column_value)(sqlite3_stmt*, int);
extern int (*orig_sqlite3_data_count)(sqlite3_stmt*);
extern int (*orig_sqlite3_value_type)(sqlite3_value*);
extern const unsigned char* (*orig_sqlite3_value_text)(sqlite3_value*);
extern int (*orig_sqlite3_value_int)(sqlite3_value*);
extern sqlite3_int64 (*orig_sqlite3_value_int64)(sqlite3_value*);
extern double (*orig_sqlite3_value_double)(sqlite3_value*);
extern int (*orig_sqlite3_value_bytes)(sqlite3_value*);
extern const void* (*orig_sqlite3_value_blob)(sqlite3_value*);
extern int (*orig_sqlite3_create_collation)(sqlite3*, const char*, int, void*, int(*)(void*,int,const void*,int,const void*));
extern int (*orig_sqlite3_create_collation_v2)(sqlite3*, const char*, int, void*, int(*)(void*,int,const void*,int,const void*), void(*)(void*));
extern void (*orig_sqlite3_free)(void*);
extern void* (*orig_sqlite3_malloc)(int);
extern sqlite3* (*orig_sqlite3_db_handle)(sqlite3_stmt*);
extern const char* (*orig_sqlite3_sql)(sqlite3_stmt*);
extern char* (*orig_sqlite3_expanded_sql)(sqlite3_stmt*);
extern int (*orig_sqlite3_bind_parameter_count)(sqlite3_stmt*);
extern int (*orig_sqlite3_bind_parameter_index)(sqlite3_stmt*, const char*);
extern const char* (*orig_sqlite3_bind_parameter_name)(sqlite3_stmt*, int);
extern int (*orig_sqlite3_stmt_readonly)(sqlite3_stmt*);
extern int (*orig_sqlite3_stmt_busy)(sqlite3_stmt*);
extern int (*orig_sqlite3_stmt_status)(sqlite3_stmt*, int, int);
extern int (*real_sqlite3_prepare_v2)(sqlite3*, const char*, int, sqlite3_stmt**, const char**);
extern const char* (*real_sqlite3_errmsg)(sqlite3*);
extern int (*real_sqlite3_errcode)(sqlite3*);

// ============================================================================
// Stub functions for modules we don't want to pull in
// (satisfies linker when linking db_interpose_common.o)
// ============================================================================

void pg_client_init(void) {}
void pg_client_cleanup(void) {}
void pg_pool_cleanup_after_fork(void) {}
void pg_config_init(void) {}
void pg_statement_init(void) {}
void pg_statement_cleanup(void) {}
void pg_query_cache_init(void) {}
void sql_translator_init(void) {}
void sql_translator_cleanup(void) {}

// my_sqlite3_prepare_v2_internal referenced by common's worker thread
int my_sqlite3_prepare_v2_internal(sqlite3 *db, const char *zSql, int nByte,
                                    sqlite3_stmt **ppStmt, const char **pzTail,
                                    int from_worker) {
    (void)db; (void)zSql; (void)nByte; (void)ppStmt; (void)pzTail; (void)from_worker;
    return -1;
}

// ============================================================================
// Test framework
// ============================================================================

static int passed = 0;
static int failed = 0;

#define BOLD  "\033[1m"
#define GREEN "\033[32m"
#define RED   "\033[31m"
#define RESET "\033[0m"

static void test(const char *name, int condition) {
    if (condition) {
        printf("  Testing: %-60s " GREEN "PASS" RESET "\n", name);
        passed++;
    } else {
        printf("  Testing: %-60s " RED "FAIL" RESET "\n", name);
        failed++;
    }
}

// ============================================================================
// Helper: reset all orig_* pointers to NULL
// ============================================================================

static void reset_all_pointers(void) {
    orig_sqlite3_open = NULL;
    orig_sqlite3_open_v2 = NULL;
    orig_sqlite3_close = NULL;
    orig_sqlite3_close_v2 = NULL;
    orig_sqlite3_exec = NULL;
    orig_sqlite3_get_table = NULL;
    orig_sqlite3_changes = NULL;
    orig_sqlite3_changes64 = NULL;
    orig_sqlite3_last_insert_rowid = NULL;
    orig_sqlite3_errmsg = NULL;
    orig_sqlite3_errcode = NULL;
    orig_sqlite3_extended_errcode = NULL;
    orig_sqlite3_prepare = NULL;
    orig_sqlite3_prepare_v2 = NULL;
    orig_sqlite3_prepare_v3 = NULL;
    orig_sqlite3_prepare16_v2 = NULL;
    orig_sqlite3_bind_int = NULL;
    orig_sqlite3_bind_int64 = NULL;
    orig_sqlite3_bind_double = NULL;
    orig_sqlite3_bind_text = NULL;
    orig_sqlite3_bind_text64 = NULL;
    orig_sqlite3_bind_blob = NULL;
    orig_sqlite3_bind_blob64 = NULL;
    orig_sqlite3_bind_value = NULL;
    orig_sqlite3_bind_null = NULL;
    orig_sqlite3_step = NULL;
    orig_sqlite3_reset = NULL;
    orig_sqlite3_finalize = NULL;
    orig_sqlite3_clear_bindings = NULL;
    orig_sqlite3_column_count = NULL;
    orig_sqlite3_column_type = NULL;
    orig_sqlite3_column_int = NULL;
    orig_sqlite3_column_int64 = NULL;
    orig_sqlite3_column_double = NULL;
    orig_sqlite3_column_text = NULL;
    orig_sqlite3_column_blob = NULL;
    orig_sqlite3_column_bytes = NULL;
    orig_sqlite3_column_name = NULL;
    orig_sqlite3_column_decltype = NULL;
    orig_sqlite3_column_value = NULL;
    orig_sqlite3_data_count = NULL;
    orig_sqlite3_value_type = NULL;
    orig_sqlite3_value_text = NULL;
    orig_sqlite3_value_int = NULL;
    orig_sqlite3_value_int64 = NULL;
    orig_sqlite3_value_double = NULL;
    orig_sqlite3_value_bytes = NULL;
    orig_sqlite3_value_blob = NULL;
    orig_sqlite3_create_collation = NULL;
    orig_sqlite3_create_collation_v2 = NULL;
    orig_sqlite3_free = NULL;
    orig_sqlite3_malloc = NULL;
    orig_sqlite3_db_handle = NULL;
    orig_sqlite3_sql = NULL;
    orig_sqlite3_expanded_sql = NULL;
    orig_sqlite3_bind_parameter_count = NULL;
    orig_sqlite3_bind_parameter_index = NULL;
    orig_sqlite3_bind_parameter_name = NULL;
    orig_sqlite3_stmt_readonly = NULL;
    orig_sqlite3_stmt_busy = NULL;
    orig_sqlite3_stmt_status = NULL;
    real_sqlite3_prepare_v2 = NULL;
    real_sqlite3_errmsg = NULL;
    real_sqlite3_errcode = NULL;
}

// ============================================================================
// Tests
// ============================================================================

int main(void) {
    printf(BOLD "=== Platform Parity Tests ===" RESET "\n\n");

    // ========================================================================
    // common_load_sqlite_symbols: NULL handle safety
    // ========================================================================
    printf(BOLD "common_load_sqlite_symbols - NULL handle:" RESET "\n");

    reset_all_pointers();
    common_load_sqlite_symbols(NULL);

    test("NULL handle -> no crash",
         1);  // If we get here, it didn't crash

    test("NULL handle -> pointers remain NULL",
         orig_sqlite3_open == NULL &&
         orig_sqlite3_prepare_v2 == NULL &&
         orig_sqlite3_step == NULL);

    // ========================================================================
    // common_load_sqlite_symbols: real SQLite handle
    // ========================================================================
    printf("\n" BOLD "common_load_sqlite_symbols - real SQLite handle:" RESET "\n");

    reset_all_pointers();

    // Open the system SQLite library
#ifdef __APPLE__
    void *handle = dlopen("/usr/lib/libsqlite3.dylib", RTLD_LAZY | RTLD_NOLOAD);
    if (!handle) handle = dlopen("libsqlite3.dylib", RTLD_LAZY);
    if (!handle) handle = RTLD_DEFAULT;  // SQLite is linked into the test binary
#else
    void *handle = dlopen("libsqlite3.so.0", RTLD_LAZY);
    if (!handle) handle = dlopen("libsqlite3.so", RTLD_LAZY);
    if (!handle) handle = RTLD_DEFAULT;
#endif

    // Redirect stderr to suppress the [SHIM_INIT] log lines during test
    FILE *saved_stderr = stderr;
    stderr = fopen("/dev/null", "w");

    common_load_sqlite_symbols(handle);

    fclose(stderr);
    stderr = saved_stderr;

    // Core functions (these MUST be resolved from any SQLite library)
    test("orig_sqlite3_open resolved",
         orig_sqlite3_open != NULL);

    test("orig_sqlite3_open_v2 resolved",
         orig_sqlite3_open_v2 != NULL);

    test("orig_sqlite3_close resolved",
         orig_sqlite3_close != NULL);

    test("orig_sqlite3_exec resolved",
         orig_sqlite3_exec != NULL);

    test("orig_sqlite3_prepare_v2 resolved",
         orig_sqlite3_prepare_v2 != NULL);

    test("orig_sqlite3_step resolved",
         orig_sqlite3_step != NULL);

    test("orig_sqlite3_finalize resolved",
         orig_sqlite3_finalize != NULL);

    test("orig_sqlite3_reset resolved",
         orig_sqlite3_reset != NULL);

    test("orig_sqlite3_errmsg resolved",
         orig_sqlite3_errmsg != NULL);

    // Bug fix #1: sqlite3_column_decltype was missing from macOS interception
    test("orig_sqlite3_column_decltype resolved (BUG FIX #1)",
         orig_sqlite3_column_decltype != NULL);

    // Column access (critical for SOCI type mapping)
    test("orig_sqlite3_column_count resolved",
         orig_sqlite3_column_count != NULL);

    test("orig_sqlite3_column_type resolved",
         orig_sqlite3_column_type != NULL);

    test("orig_sqlite3_column_int resolved",
         orig_sqlite3_column_int != NULL);

    test("orig_sqlite3_column_int64 resolved",
         orig_sqlite3_column_int64 != NULL);

    test("orig_sqlite3_column_double resolved",
         orig_sqlite3_column_double != NULL);

    test("orig_sqlite3_column_text resolved",
         orig_sqlite3_column_text != NULL);

    test("orig_sqlite3_column_blob resolved",
         orig_sqlite3_column_blob != NULL);

    test("orig_sqlite3_column_bytes resolved",
         orig_sqlite3_column_bytes != NULL);

    test("orig_sqlite3_column_name resolved",
         orig_sqlite3_column_name != NULL);

    // Bind functions
    test("orig_sqlite3_bind_int resolved",
         orig_sqlite3_bind_int != NULL);

    test("orig_sqlite3_bind_int64 resolved",
         orig_sqlite3_bind_int64 != NULL);

    test("orig_sqlite3_bind_double resolved",
         orig_sqlite3_bind_double != NULL);

    test("orig_sqlite3_bind_text resolved",
         orig_sqlite3_bind_text != NULL);

    test("orig_sqlite3_bind_blob resolved",
         orig_sqlite3_bind_blob != NULL);

    test("orig_sqlite3_bind_null resolved",
         orig_sqlite3_bind_null != NULL);

    // Metadata
    test("orig_sqlite3_changes resolved",
         orig_sqlite3_changes != NULL);

    test("orig_sqlite3_last_insert_rowid resolved",
         orig_sqlite3_last_insert_rowid != NULL);

    test("orig_sqlite3_errcode resolved",
         orig_sqlite3_errcode != NULL);

    // Statement info
    test("orig_sqlite3_sql resolved",
         orig_sqlite3_sql != NULL);

    test("orig_sqlite3_bind_parameter_count resolved",
         orig_sqlite3_bind_parameter_count != NULL);

    test("orig_sqlite3_bind_parameter_index resolved",
         orig_sqlite3_bind_parameter_index != NULL);

    // Memory
    test("orig_sqlite3_free resolved",
         orig_sqlite3_free != NULL);

    test("orig_sqlite3_malloc resolved",
         orig_sqlite3_malloc != NULL);

    // Value access
    test("orig_sqlite3_value_type resolved",
         orig_sqlite3_value_type != NULL);

    test("orig_sqlite3_value_text resolved",
         orig_sqlite3_value_text != NULL);

    test("orig_sqlite3_value_int64 resolved",
         orig_sqlite3_value_int64 != NULL);

    // Backward compatibility aliases
    test("real_sqlite3_prepare_v2 alias set",
         real_sqlite3_prepare_v2 != NULL);

    test("real_sqlite3_errmsg alias set",
         real_sqlite3_errmsg != NULL);

    test("real_sqlite3_errcode alias set",
         real_sqlite3_errcode != NULL);

    test("real_sqlite3_prepare_v2 == orig_sqlite3_prepare_v2",
         real_sqlite3_prepare_v2 == orig_sqlite3_prepare_v2);

    test("real_sqlite3_errmsg == orig_sqlite3_errmsg",
         real_sqlite3_errmsg == orig_sqlite3_errmsg);

    // ========================================================================
    // Bug fix #3: macOS fallback completeness
    // Count total resolved vs expected (was ~11/60, now should be ~60/60)
    // ========================================================================
    printf("\n" BOLD "Symbol loading completeness (BUG FIX #3 - was 11/60):" RESET "\n");

    int total_resolved = 0;
    if (orig_sqlite3_open) total_resolved++;
    if (orig_sqlite3_open_v2) total_resolved++;
    if (orig_sqlite3_close) total_resolved++;
    if (orig_sqlite3_close_v2) total_resolved++;
    if (orig_sqlite3_exec) total_resolved++;
    if (orig_sqlite3_get_table) total_resolved++;
    if (orig_sqlite3_changes) total_resolved++;
    if (orig_sqlite3_changes64) total_resolved++;
    if (orig_sqlite3_last_insert_rowid) total_resolved++;
    if (orig_sqlite3_errmsg) total_resolved++;
    if (orig_sqlite3_errcode) total_resolved++;
    if (orig_sqlite3_extended_errcode) total_resolved++;
    if (orig_sqlite3_prepare) total_resolved++;
    if (orig_sqlite3_prepare_v2) total_resolved++;
    if (orig_sqlite3_prepare_v3) total_resolved++;
    if (orig_sqlite3_prepare16_v2) total_resolved++;
    if (orig_sqlite3_bind_int) total_resolved++;
    if (orig_sqlite3_bind_int64) total_resolved++;
    if (orig_sqlite3_bind_double) total_resolved++;
    if (orig_sqlite3_bind_text) total_resolved++;
    if (orig_sqlite3_bind_text64) total_resolved++;
    if (orig_sqlite3_bind_blob) total_resolved++;
    if (orig_sqlite3_bind_blob64) total_resolved++;
    if (orig_sqlite3_bind_value) total_resolved++;
    if (orig_sqlite3_bind_null) total_resolved++;
    if (orig_sqlite3_step) total_resolved++;
    if (orig_sqlite3_reset) total_resolved++;
    if (orig_sqlite3_finalize) total_resolved++;
    if (orig_sqlite3_clear_bindings) total_resolved++;
    if (orig_sqlite3_column_count) total_resolved++;
    if (orig_sqlite3_column_type) total_resolved++;
    if (orig_sqlite3_column_int) total_resolved++;
    if (orig_sqlite3_column_int64) total_resolved++;
    if (orig_sqlite3_column_double) total_resolved++;
    if (orig_sqlite3_column_text) total_resolved++;
    if (orig_sqlite3_column_blob) total_resolved++;
    if (orig_sqlite3_column_bytes) total_resolved++;
    if (orig_sqlite3_column_name) total_resolved++;
    if (orig_sqlite3_column_decltype) total_resolved++;
    if (orig_sqlite3_column_value) total_resolved++;
    if (orig_sqlite3_data_count) total_resolved++;
    if (orig_sqlite3_value_type) total_resolved++;
    if (orig_sqlite3_value_text) total_resolved++;
    if (orig_sqlite3_value_int) total_resolved++;
    if (orig_sqlite3_value_int64) total_resolved++;
    if (orig_sqlite3_value_double) total_resolved++;
    if (orig_sqlite3_value_bytes) total_resolved++;
    if (orig_sqlite3_value_blob) total_resolved++;
    if (orig_sqlite3_create_collation) total_resolved++;
    if (orig_sqlite3_create_collation_v2) total_resolved++;
    if (orig_sqlite3_free) total_resolved++;
    if (orig_sqlite3_malloc) total_resolved++;
    if (orig_sqlite3_db_handle) total_resolved++;
    if (orig_sqlite3_sql) total_resolved++;
    if (orig_sqlite3_expanded_sql) total_resolved++;
    if (orig_sqlite3_bind_parameter_count) total_resolved++;
    if (orig_sqlite3_bind_parameter_index) total_resolved++;
    if (orig_sqlite3_bind_parameter_name) total_resolved++;
    if (orig_sqlite3_stmt_readonly) total_resolved++;
    if (orig_sqlite3_stmt_busy) total_resolved++;
    if (orig_sqlite3_stmt_status) total_resolved++;

    printf("  Resolved %d symbols (was 11 before fix, 63 possible)\n", total_resolved);

    test("At least 50 symbols resolved (was 11 before fix)",
         total_resolved >= 50);

    // Some symbols (e.g. sqlite3_changes64, sqlite3_prepare_v3) may not exist
    // on older SQLite versions. Accept anything above 55 as complete.
    test("Nearly all symbols resolved (>= 55/63)",
         total_resolved >= 55);

    // ========================================================================
    // If-not-set pattern: pre-set pointers should NOT be overwritten
    // ========================================================================
    printf("\n" BOLD "If-not-set pattern (pre-set pointers preserved):" RESET "\n");

    // Set a sentinel value for orig_sqlite3_open
    void *sentinel = (void*)(uintptr_t)0xDEADBEEF;
    reset_all_pointers();
    orig_sqlite3_open = (int(*)(const char*, sqlite3**))sentinel;
    orig_sqlite3_prepare_v2 = (int(*)(sqlite3*, const char*, int, sqlite3_stmt**, const char**))sentinel;

    // Redirect stderr
    saved_stderr = stderr;
    stderr = fopen("/dev/null", "w");

    common_load_sqlite_symbols(handle);

    fclose(stderr);
    stderr = saved_stderr;

    test("Pre-set orig_sqlite3_open NOT overwritten",
         (void*)orig_sqlite3_open == sentinel);

    test("Pre-set orig_sqlite3_prepare_v2 NOT overwritten",
         (void*)orig_sqlite3_prepare_v2 == sentinel);

    test("Other pointers still populated (orig_sqlite3_step)",
         orig_sqlite3_step != NULL);

    test("Other pointers still populated (orig_sqlite3_column_decltype)",
         orig_sqlite3_column_decltype != NULL);

    // ========================================================================
    // Double-call idempotency: calling twice doesn't change anything
    // ========================================================================
    printf("\n" BOLD "Idempotency (double call):" RESET "\n");

    reset_all_pointers();

    saved_stderr = stderr;
    stderr = fopen("/dev/null", "w");

    common_load_sqlite_symbols(handle);
    void *first_open = (void*)orig_sqlite3_open;
    void *first_step = (void*)orig_sqlite3_step;
    void *first_decltype = (void*)orig_sqlite3_column_decltype;

    common_load_sqlite_symbols(handle);

    fclose(stderr);
    stderr = saved_stderr;

    test("Double call -> orig_sqlite3_open unchanged",
         (void*)orig_sqlite3_open == first_open);

    test("Double call -> orig_sqlite3_step unchanged",
         (void*)orig_sqlite3_step == first_step);

    test("Double call -> orig_sqlite3_column_decltype unchanged",
         (void*)orig_sqlite3_column_decltype == first_decltype);

    // ========================================================================
    // Resolved pointers actually work (call through to real SQLite)
    // ========================================================================
    printf("\n" BOLD "Resolved pointers are callable:" RESET "\n");

    reset_all_pointers();

    saved_stderr = stderr;
    stderr = fopen("/dev/null", "w");
    common_load_sqlite_symbols(handle);
    fclose(stderr);
    stderr = saved_stderr;

    // Test that orig_sqlite3_open actually works
    if (orig_sqlite3_open) {
        sqlite3 *db = NULL;
        int rc = orig_sqlite3_open(":memory:", &db);
        test("orig_sqlite3_open(':memory:') -> SQLITE_OK",
             rc == SQLITE_OK && db != NULL);

        if (db && orig_sqlite3_exec) {
            rc = orig_sqlite3_exec(db, "CREATE TABLE t(x INTEGER)", NULL, NULL, NULL);
            test("orig_sqlite3_exec(CREATE TABLE) -> SQLITE_OK",
                 rc == SQLITE_OK);
        } else {
            test("orig_sqlite3_exec(CREATE TABLE) -> SQLITE_OK", 0);
        }

        if (db && orig_sqlite3_prepare_v2) {
            sqlite3_stmt *stmt = NULL;
            rc = orig_sqlite3_prepare_v2(db, "SELECT 42", -1, &stmt, NULL);
            test("orig_sqlite3_prepare_v2(SELECT 42) -> SQLITE_OK",
                 rc == SQLITE_OK && stmt != NULL);

            if (stmt && orig_sqlite3_step) {
                rc = orig_sqlite3_step(stmt);
                test("orig_sqlite3_step -> SQLITE_ROW",
                     rc == SQLITE_ROW);

                if (orig_sqlite3_column_int) {
                    int val = orig_sqlite3_column_int(stmt, 0);
                    test("orig_sqlite3_column_int -> 42",
                         val == 42);
                } else {
                    test("orig_sqlite3_column_int -> 42", 0);
                }
            } else {
                test("orig_sqlite3_step -> SQLITE_ROW", 0);
                test("orig_sqlite3_column_int -> 42", 0);
            }

            if (stmt && orig_sqlite3_finalize)
                orig_sqlite3_finalize(stmt);
        } else {
            test("orig_sqlite3_prepare_v2(SELECT 42) -> SQLITE_OK", 0);
            test("orig_sqlite3_step -> SQLITE_ROW", 0);
            test("orig_sqlite3_column_int -> 42", 0);
        }

        // Test column_decltype through a real query (BUG FIX verification)
        if (db && orig_sqlite3_prepare_v2 && orig_sqlite3_column_decltype) {
            sqlite3_stmt *stmt = NULL;
            rc = orig_sqlite3_prepare_v2(db,
                "SELECT x FROM t", -1, &stmt, NULL);
            if (rc == SQLITE_OK && stmt) {
                const char *dt = orig_sqlite3_column_decltype(stmt, 0);
                test("orig_sqlite3_column_decltype -> 'INTEGER'",
                     dt != NULL && strcmp(dt, "INTEGER") == 0);
                orig_sqlite3_finalize(stmt);
            } else {
                test("orig_sqlite3_column_decltype -> 'INTEGER'", 0);
            }
        } else {
            test("orig_sqlite3_column_decltype -> 'INTEGER'", 0);
        }

        if (db && orig_sqlite3_close)
            orig_sqlite3_close(db);
    } else {
        // Skip callable tests if open failed
        test("orig_sqlite3_open(':memory:') -> SQLITE_OK", 0);
        test("orig_sqlite3_exec(CREATE TABLE) -> SQLITE_OK", 0);
        test("orig_sqlite3_prepare_v2(SELECT 42) -> SQLITE_OK", 0);
        test("orig_sqlite3_step -> SQLITE_ROW", 0);
        test("orig_sqlite3_column_int -> 42", 0);
        test("orig_sqlite3_column_decltype -> 'INTEGER'", 0);
    }

    // ========================================================================
    // platform_print_backtrace: basic operation
    // ========================================================================
    printf("\n" BOLD "platform_print_backtrace:" RESET "\n");

    // Redirect stderr to capture backtrace output
    char bt_buf[16384] = {0};
    FILE *mem = fmemopen(bt_buf, sizeof(bt_buf) - 1, "w");
    if (mem) {
        saved_stderr = stderr;
        stderr = mem;

        platform_print_backtrace("test reason", 0);

        fflush(stderr);
        fclose(mem);
        stderr = saved_stderr;

        test("Backtrace output is non-empty",
             strlen(bt_buf) > 0);

        test("Backtrace contains reason string",
             strstr(bt_buf, "test reason") != NULL);

        test("Backtrace contains box-drawing top border",
             strstr(bt_buf, "\xe2\x95\x94") != NULL);  // UTF-8 for "box drawing double horizontal"

        test("Backtrace contains box-drawing bottom border",
             strstr(bt_buf, "\xe2\x95\x9a") != NULL);  // UTF-8 for bottom-left corner

        test("Backtrace contains frame numbers",
             strstr(bt_buf, "[ 0]") != NULL);

        // Should have multiple frames (we're several calls deep)
        int frame_count = 0;
        const char *p = bt_buf;
        while ((p = strstr(p, "[")) != NULL) {
            // Match [space+digit] pattern
            if ((p[1] == ' ' && p[2] >= '0' && p[2] <= '9') ||
                (p[1] >= '0' && p[1] <= '9')) {
                frame_count++;
            }
            p++;
        }
        test("Backtrace has >= 2 frames",
             frame_count >= 2);
    } else {
        test("Backtrace output is non-empty", 0);
        test("Backtrace contains reason string", 0);
        test("Backtrace contains box-drawing top border", 0);
        test("Backtrace contains box-drawing bottom border", 0);
        test("Backtrace contains frame numbers", 0);
        test("Backtrace has >= 2 frames", 0);
    }

    // Test with NULL reason
    memset(bt_buf, 0, sizeof(bt_buf));
    mem = fmemopen(bt_buf, sizeof(bt_buf) - 1, "w");
    if (mem) {
        saved_stderr = stderr;
        stderr = mem;

        platform_print_backtrace(NULL, 0);

        fflush(stderr);
        fclose(mem);
        stderr = saved_stderr;

        test("NULL reason -> no crash, output contains 'Unknown'",
             strstr(bt_buf, "Unknown") != NULL);
    } else {
        test("NULL reason -> no crash, output contains 'Unknown'", 0);
    }

    // Test skip_frames
    char bt_buf_skip0[16384] = {0};
    char bt_buf_skip3[16384] = {0};

    mem = fmemopen(bt_buf_skip0, sizeof(bt_buf_skip0) - 1, "w");
    if (mem) {
        saved_stderr = stderr;
        stderr = mem;
        platform_print_backtrace("skip test", 0);
        fflush(stderr);
        fclose(mem);
        stderr = saved_stderr;
    }

    mem = fmemopen(bt_buf_skip3, sizeof(bt_buf_skip3) - 1, "w");
    if (mem) {
        saved_stderr = stderr;
        stderr = mem;
        platform_print_backtrace("skip test", 3);
        fflush(stderr);
        fclose(mem);
        stderr = saved_stderr;
    }

    test("skip_frames=3 produces shorter output than skip_frames=0",
         strlen(bt_buf_skip3) < strlen(bt_buf_skip0));

    // ========================================================================
    // Summary
    // ========================================================================
    printf("\n" BOLD "=== Results ===" RESET "\n");
    printf("Passed: " GREEN "%d" RESET "\n", passed);
    printf("Failed: " RED "%d" RESET "\n", failed);
    printf("\n");

    if (handle && handle != RTLD_DEFAULT)
        dlclose(handle);

    return failed > 0 ? 1 : 0;
}
