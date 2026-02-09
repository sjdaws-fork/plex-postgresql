/*
 * Unit Tests for pg_config.c - SQL Classification
 * Tests: should_redirect(), should_skip_sql(), is_write_operation(), is_read_operation()
 *
 * These are pure functions that classify SQL and database paths.
 * Critical for routing correctness - wrong classification = data loss or silent failures.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Stub out globals and logging that pg_config.c references
int shim_passthrough_only = 0;
void pg_log_write(int level, const char *fmt, ...) { (void)level; (void)fmt; }

// Declare the functions we're testing
int should_redirect(const char *filename);
int should_skip_sql(const char *sql);
int is_write_operation(const char *sql);
int is_read_operation(const char *sql);

// Test framework
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

int main(void) {
    printf(BOLD "=== pg_config SQL Classification Tests ===" RESET "\n\n");

    // ========================================================================
    // should_redirect() - determines if a database file goes to PostgreSQL
    // ========================================================================
    printf(BOLD "should_redirect():" RESET "\n");

    test("NULL filename -> 0",
         should_redirect(NULL) == 0);

    test("Plex library.db -> 1",
         should_redirect("/var/lib/plexmediaserver/Plug-in Support/Databases/com.plexapp.plugins.library.db") == 1);

    test("Plex blobs.db -> 1",
         should_redirect("/var/lib/plexmediaserver/Plug-in Support/Databases/com.plexapp.plugins.library.blobs.db") == 1);

    test("Other SQLite db -> 0",
         should_redirect("/tmp/test.db") == 0);

    test("Empty string -> 0",
         should_redirect("") == 0);

    test("Partial match library -> 0",
         should_redirect("library.db") == 0);

    test("macOS Plex path -> 1",
         should_redirect("/Users/plex/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db") == 1);

    test("Linux Plex path -> 1",
         should_redirect("/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.blobs.db") == 1);

    test("Plex preferences.db -> 0",
         should_redirect("com.plexapp.plugins.preferences.db") == 0);

    // Test passthrough mode
    shim_passthrough_only = 1;
    test("Passthrough mode -> always 0",
         should_redirect("com.plexapp.plugins.library.db") == 0);
    shim_passthrough_only = 0;

    // ========================================================================
    // should_skip_sql() - determines if SQL should be no-op'd
    // ========================================================================
    printf("\n" BOLD "should_skip_sql():" RESET "\n");

    test("NULL -> 0",
         should_skip_sql(NULL) == 0);

    // Prefix patterns
    test("PRAGMA -> skip",
         should_skip_sql("PRAGMA journal_mode=WAL") == 1);

    test("VACUUM -> skip",
         should_skip_sql("VACUUM") == 1);

    test("REINDEX -> skip",
         should_skip_sql("REINDEX") == 1);

    test("ANALYZE sqlite_master -> skip",
         should_skip_sql("ANALYZE sqlite_master") == 1);

    test("ANALYZE normal_table -> no skip",
         should_skip_sql("ANALYZE metadata_items") == 0);

    test("BEGIN -> skip",
         should_skip_sql("BEGIN") == 1);

    test("BEGIN IMMEDIATE -> skip",
         should_skip_sql("BEGIN IMMEDIATE") == 1);

    test("COMMIT -> skip",
         should_skip_sql("COMMIT") == 1);

    test("ROLLBACK -> skip",
         should_skip_sql("ROLLBACK") == 1);

    test("SAVEPOINT -> skip",
         should_skip_sql("SAVEPOINT sp1") == 1);

    test("RELEASE SAVEPOINT -> skip",
         should_skip_sql("RELEASE SAVEPOINT sp1") == 1);

    test("ATTACH DATABASE -> skip",
         should_skip_sql("ATTACH DATABASE ':memory:' AS aux") == 1);

    test("DETACH DATABASE -> skip",
         should_skip_sql("DETACH DATABASE aux") == 1);

    test("SELECT load_extension -> skip",
         should_skip_sql("SELECT load_extension('/path/to/ext')") == 1);

    test("icu_load_collation -> skip",
         should_skip_sql("icu_load_collation('en_US', 'icu_root')") == 1);

    test("fts3_tokenizer -> skip",
         should_skip_sql("fts3_tokenizer('simple')") == 1);

    // Anywhere patterns
    test("sqlite_master anywhere -> skip",
         should_skip_sql("SELECT * FROM sqlite_master WHERE type='table'") == 1);

    test("sqlite_schema anywhere -> skip",
         should_skip_sql("SELECT name FROM sqlite_schema") == 1);

    test("spellfix anywhere -> skip",
         should_skip_sql("SELECT * FROM spellfix_table") == 1);

    test("SET $2=$2 dynamic no-op -> skip",
         should_skip_sql("UPDATE metadata_items SET $2=$2 WHERE id=$1") == 1);

    test("SET $1=$1 dynamic no-op -> skip",
         should_skip_sql("UPDATE metadata_items SET $1=$1 WHERE id=$3") == 1);

    test(":col=:col named param no-op -> skip",
         should_skip_sql("UPDATE metadata_items SET :col=:col WHERE id=:id") == 1);

    // Should NOT skip
    test("SELECT -> no skip",
         should_skip_sql("SELECT * FROM metadata_items") == 0);

    test("INSERT -> no skip",
         should_skip_sql("INSERT INTO metadata_items (title) VALUES ('test')") == 0);

    test("UPDATE -> no skip",
         should_skip_sql("UPDATE metadata_items SET title='test'") == 0);

    test("DELETE -> no skip",
         should_skip_sql("DELETE FROM metadata_items WHERE id=1") == 0);

    test("CREATE TABLE -> no skip",
         should_skip_sql("CREATE TABLE IF NOT EXISTS test (id INTEGER)") == 0);

    // Whitespace handling
    test("Leading spaces before PRAGMA -> skip",
         should_skip_sql("   PRAGMA table_info(metadata_items)") == 1);

    test("Leading tabs before BEGIN -> skip",
         should_skip_sql("\t\tBEGIN TRANSACTION") == 1);

    test("Leading newline before VACUUM -> skip",
         should_skip_sql("\nVACUUM") == 1);

    // Case insensitivity
    test("pragma lowercase -> skip",
         should_skip_sql("pragma journal_mode") == 1);

    test("begin lowercase -> skip",
         should_skip_sql("begin") == 1);

    test("Commit mixed case -> skip",
         should_skip_sql("Commit") == 1);

    // ========================================================================
    // is_write_operation()
    // ========================================================================
    printf("\n" BOLD "is_write_operation():" RESET "\n");

    test("NULL -> 0",
         is_write_operation(NULL) == 0);

    test("INSERT -> 1",
         is_write_operation("INSERT INTO metadata_items (title) VALUES ('test')") == 1);

    test("UPDATE -> 1",
         is_write_operation("UPDATE metadata_items SET title='test'") == 1);

    test("DELETE -> 1",
         is_write_operation("DELETE FROM metadata_items WHERE id=1") == 1);

    test("REPLACE -> 1",
         is_write_operation("REPLACE INTO tags (id, tag) VALUES (1, 'Action')") == 1);

    test("SELECT -> 0",
         is_write_operation("SELECT * FROM metadata_items") == 0);

    test("CREATE TABLE -> 0",
         is_write_operation("CREATE TABLE test (id INTEGER)") == 0);

    test("DROP TABLE -> 0",
         is_write_operation("DROP TABLE test") == 0);

    // Whitespace
    test("  INSERT with leading spaces -> 1",
         is_write_operation("  INSERT INTO test VALUES (1)") == 1);

    test("\\n\\tUPDATE with whitespace -> 1",
         is_write_operation("\n\tUPDATE test SET x=1") == 1);

    // Case insensitivity
    test("insert lowercase -> 1",
         is_write_operation("insert into test values (1)") == 1);

    test("Delete mixed case -> 1",
         is_write_operation("Delete FROM test WHERE 1=1") == 1);

    // ========================================================================
    // is_read_operation()
    // ========================================================================
    printf("\n" BOLD "is_read_operation():" RESET "\n");

    test("NULL -> 0",
         is_read_operation(NULL) == 0);

    test("SELECT -> 1",
         is_read_operation("SELECT * FROM metadata_items") == 1);

    test("INSERT -> 0",
         is_read_operation("INSERT INTO test VALUES (1)") == 0);

    test("UPDATE -> 0",
         is_read_operation("UPDATE test SET x=1") == 0);

    test("CREATE -> 0",
         is_read_operation("CREATE TABLE test (id INTEGER)") == 0);

    test("  SELECT with spaces -> 1",
         is_read_operation("  SELECT id FROM test") == 1);

    test("select lowercase -> 1",
         is_read_operation("select * from test") == 1);

    // Summary
    printf("\n" BOLD "=== Results ===" RESET "\n");
    printf("Passed: " GREEN "%d" RESET "\n", passed);
    printf("Failed: " RED "%d" RESET "\n", failed);
    printf("\n");

    return failed > 0 ? 1 : 0;
}
