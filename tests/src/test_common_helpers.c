/*
 * Unit Tests for db_interpose_common.c helper functions
 * Tests: is_library_db_path(), simple_str_replace()
 *
 * These are pure functions used for path classification and string manipulation.
 * is_library_db_path() determines which databases get the PostgreSQL treatment.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ============================================================================
// Copy of is_library_db_path and simple_str_replace from db_interpose_common.c
// Copied here to avoid pulling in libpq/sqlite3/pthread dependencies
// ============================================================================

int is_library_db_path(const char *path) {
    if (!path) return 0;
    return strstr(path, "com.plexapp.plugins.library.db") != NULL ||
           strstr(path, "com.plexapp.plugins.library.blobs.db") != NULL;
}

char* simple_str_replace(const char *str, const char *old, const char *new_str) {
    if (!str || !old || !new_str) return NULL;

    const char *pos = strstr(str, old);
    if (!pos) return NULL;

    size_t old_len = strlen(old);
    size_t new_len = strlen(new_str);
    size_t result_len = strlen(str) - old_len + new_len;

    char *result = malloc(result_len + 1);
    if (!result) return NULL;

    size_t prefix_len = pos - str;
    memcpy(result, str, prefix_len);
    memcpy(result + prefix_len, new_str, new_len);
    strcpy(result + prefix_len + new_len, pos + old_len);

    return result;
}

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
    printf(BOLD "=== Common Helper Function Tests ===" RESET "\n\n");

    // ========================================================================
    // is_library_db_path()
    // ========================================================================
    printf(BOLD "is_library_db_path():" RESET "\n");

    test("NULL -> 0",
         is_library_db_path(NULL) == 0);

    test("Empty string -> 0",
         is_library_db_path("") == 0);

    test("Plex library.db -> 1",
         is_library_db_path("/data/Databases/com.plexapp.plugins.library.db") == 1);

    test("Plex library.blobs.db -> 1",
         is_library_db_path("/data/Databases/com.plexapp.plugins.library.blobs.db") == 1);

    test("Just the filename library.db -> 1",
         is_library_db_path("com.plexapp.plugins.library.db") == 1);

    test("Just the filename blobs.db -> 1",
         is_library_db_path("com.plexapp.plugins.library.blobs.db") == 1);

    test("Other database -> 0",
         is_library_db_path("com.plexapp.plugins.preferences.db") == 0);

    test("Random path -> 0",
         is_library_db_path("/tmp/test.db") == 0);

    test("Partial match 'library' -> 0",
         is_library_db_path("library.db") == 0);

    test("macOS full path -> 1",
         is_library_db_path("/Users/plex/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db") == 1);

    test("Linux Docker path -> 1",
         is_library_db_path("/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.blobs.db") == 1);

    // WAL file contains the db pattern as a prefix, so strstr matches.
    // This is intentional — WAL files belong to the same database.
    test("WAL file -> 1 (contains db pattern)",
         is_library_db_path("com.plexapp.plugins.library.db-wal") == 1);

    // ========================================================================
    // simple_str_replace()
    // ========================================================================
    printf("\n" BOLD "simple_str_replace():" RESET "\n");

    char *result;

    // NULL inputs
    test("NULL str -> NULL",
         simple_str_replace(NULL, "old", "new") == NULL);

    test("NULL old -> NULL",
         simple_str_replace("hello", NULL, "new") == NULL);

    test("NULL new -> NULL",
         simple_str_replace("hello", "old", NULL) == NULL);

    // No match
    test("No match -> NULL",
         simple_str_replace("hello world", "xyz", "abc") == NULL);

    // Basic replacement
    result = simple_str_replace("hello world", "world", "earth");
    test("Basic replace 'world' -> 'earth'",
         result != NULL && strcmp(result, "hello earth") == 0);
    free(result);

    // Replace at start
    result = simple_str_replace("hello world", "hello", "goodbye");
    test("Replace at start 'hello' -> 'goodbye'",
         result != NULL && strcmp(result, "goodbye world") == 0);
    free(result);

    // Replace at end
    result = simple_str_replace("hello world", "world", "!");
    test("Replace at end 'world' -> '!'",
         result != NULL && strcmp(result, "hello !") == 0);
    free(result);

    // Replace with longer string
    result = simple_str_replace("ab", "a", "xyz");
    test("Replace shorter with longer",
         result != NULL && strcmp(result, "xyzb") == 0);
    free(result);

    // Replace with shorter string
    result = simple_str_replace("hello world", "hello", "hi");
    test("Replace longer with shorter",
         result != NULL && strcmp(result, "hi world") == 0);
    free(result);

    // Replace with empty string (deletion)
    result = simple_str_replace("hello world", "hello ", "");
    test("Replace with empty (delete)",
         result != NULL && strcmp(result, "world") == 0);
    free(result);

    // Empty old string matches at start
    result = simple_str_replace("hello", "", "X");
    test("Empty old matches at start -> prepend",
         result != NULL && strcmp(result, "Xhello") == 0);
    free(result);

    // Replace only first occurrence (function only replaces first)
    result = simple_str_replace("aaa", "a", "b");
    test("Only replaces first occurrence",
         result != NULL && strcmp(result, "baa") == 0);
    free(result);

    // Real-world: SQL transformation
    result = simple_str_replace(
        "INSERT OR REPLACE INTO tags",
        "INSERT OR REPLACE INTO",
        "INSERT INTO"
    );
    test("SQL: INSERT OR REPLACE -> INSERT",
         result != NULL && strcmp(result, "INSERT INTO tags") == 0);
    free(result);

    // Summary
    printf("\n" BOLD "=== Results ===" RESET "\n");
    printf("Passed: " GREEN "%d" RESET "\n", passed);
    printf("Failed: " RED "%d" RESET "\n", failed);
    printf("\n");

    return failed > 0 ? 1 : 0;
}
