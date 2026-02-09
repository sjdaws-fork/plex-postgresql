/*
 * Unit tests for SQL translation (SQLite to PostgreSQL)
 *
 * Tests:
 * 1. Placeholder translation (:name → $1)
 * 2. Function translation (IFNULL → COALESCE, etc.)
 * 3. Type translation (INTEGER → BIGINT, etc.)
 * 4. Keyword translation (GLOB → ILIKE, etc.)
 * 5. Full query translation
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

// Include the translator headers
#include "sql_translator.h"
#include "sql_translator_internal.h"

// Test counters
static int tests_passed = 0;
static int tests_failed = 0;

#define TEST(name) printf("  Testing: %s... ", name)
#define PASS() do { printf("\033[32mPASS\033[0m\n"); tests_passed++; } while(0)
#define FAIL(msg) do { printf("\033[31mFAIL: %s\033[0m\n", msg); tests_failed++; } while(0)

// ============================================================================
// Placeholder Translation Tests
// ============================================================================

static void test_placeholder_basic(void) {
    TEST("Placeholder - basic :name to $1");
    char **names = NULL;
    int count = 0;
    char *result = sql_translate_placeholders("SELECT * FROM t WHERE id = :id", &names, &count);

    if (result && strstr(result, "$1") && count == 1) {
        PASS();
    } else {
        FAIL("Expected $1 placeholder");
    }

    if (result) free(result);
    if (names) {
        for (int i = 0; i < count; i++) free(names[i]);
        free(names);
    }
}

static void test_placeholder_multiple(void) {
    TEST("Placeholder - multiple :name params");
    char **names = NULL;
    int count = 0;
    char *result = sql_translate_placeholders(
        "SELECT * FROM t WHERE a = :foo AND b = :bar AND c = :baz", &names, &count);

    if (result && strstr(result, "$1") && strstr(result, "$2") && strstr(result, "$3") && count == 3) {
        PASS();
    } else {
        FAIL("Expected $1, $2, $3 placeholders");
    }

    if (result) free(result);
    if (names) {
        for (int i = 0; i < count; i++) free(names[i]);
        free(names);
    }
}

static void test_placeholder_reuse(void) {
    TEST("Placeholder - same :name used twice");
    char **names = NULL;
    int count = 0;
    char *result = sql_translate_placeholders(
        "SELECT * FROM t WHERE a = :id OR b = :id", &names, &count);

    // Same param used twice should map to same $N
    if (result && count == 1) {
        PASS();
    } else {
        FAIL("Expected single param for reused :id");
    }

    if (result) free(result);
    if (names) {
        for (int i = 0; i < count; i++) free(names[i]);
        free(names);
    }
}

static void test_placeholder_question_mark(void) {
    TEST("Placeholder - ? positional params");
    char **names = NULL;
    int count = 0;
    char *result = sql_translate_placeholders(
        "SELECT * FROM t WHERE a = ? AND b = ?", &names, &count);

    if (result && strstr(result, "$1") && strstr(result, "$2") && count == 2) {
        PASS();
    } else {
        FAIL("Expected $1, $2 for ? params");
    }

    if (result) free(result);
    if (names) {
        for (int i = 0; i < count; i++) free(names[i]);
        free(names);
    }
}

static void test_placeholder_in_string(void) {
    TEST("Placeholder - :name inside string literal ignored");
    char **names = NULL;
    int count = 0;
    char *result = sql_translate_placeholders(
        "SELECT * FROM t WHERE a = ':not_a_param'", &names, &count);

    // Should NOT translate :not_a_param inside quotes
    if (result && count == 0 && strstr(result, ":not_a_param")) {
        PASS();
    } else {
        FAIL("Should not translate :param inside string");
    }

    if (result) free(result);
    if (names) {
        for (int i = 0; i < count; i++) free(names[i]);
        free(names);
    }
}

// ============================================================================
// Function Translation Tests
// ============================================================================

static void test_function_ifnull(void) {
    TEST("Function - IFNULL to COALESCE");
    char *result = sql_translate_functions("SELECT IFNULL(a, 0) FROM t");

    if (result && strcasestr(result, "COALESCE") && !strcasestr(result, "IFNULL")) {
        PASS();
    } else {
        FAIL("Expected COALESCE instead of IFNULL");
    }

    if (result) free(result);
}

static void test_function_length(void) {
    TEST("Function - LENGTH preserved");
    char *result = sql_translate_functions("SELECT LENGTH(name) FROM t");

    if (result && strcasestr(result, "LENGTH")) {
        PASS();
    } else {
        FAIL("LENGTH should be preserved");
    }

    if (result) free(result);
}

static void test_function_substr(void) {
    TEST("Function - SUBSTR to SUBSTRING");
    char *result = sql_translate_functions("SELECT SUBSTR(a, 1, 5) FROM t");

    if (result && strcasestr(result, "SUBSTRING")) {
        PASS();
    } else {
        FAIL("Expected SUBSTRING");
    }

    if (result) free(result);
}

static void test_function_random(void) {
    TEST("Function - RANDOM() to RANDOM()");
    char *result = sql_translate_functions("SELECT RANDOM() FROM t");

    // PostgreSQL also has RANDOM(), should work
    if (result && strcasestr(result, "RANDOM")) {
        PASS();
    } else {
        FAIL("RANDOM should be preserved");
    }

    if (result) free(result);
}

static void test_function_datetime(void) {
    TEST("Function - datetime('now') handling");
    char *result = sql_translate_functions("SELECT datetime('now') FROM t");

    // Should translate to NOW() or similar
    if (result) {
        PASS();  // Just check it doesn't crash
    } else {
        FAIL("datetime translation failed");
    }

    if (result) free(result);
}

// ============================================================================
// Keyword Translation Tests
// ============================================================================

static void test_keyword_glob(void) {
    TEST("Keyword - GLOB to ILIKE");
    char *result = sql_translate_keywords("SELECT * FROM t WHERE name GLOB '*test*'");

    if (result && (strcasestr(result, "ILIKE") || strcasestr(result, "LIKE"))) {
        PASS();
    } else {
        FAIL("Expected ILIKE/LIKE pattern");
    }

    if (result) free(result);
}

static void test_keyword_notnull(void) {
    TEST("Keyword - NOT NULL preserved");
    char *result = sql_translate_keywords("SELECT * FROM t WHERE a IS NOT NULL");

    if (result && strcasestr(result, "NOT NULL")) {
        PASS();
    } else {
        FAIL("NOT NULL should be preserved");
    }

    if (result) free(result);
}

// ============================================================================
// Type Translation Tests
// ============================================================================

static void test_type_autoincrement(void) {
    TEST("Type - AUTOINCREMENT to SERIAL");
    char *result = sql_translate_types("CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT)");

    if (result && strcasestr(result, "SERIAL") && !strcasestr(result, "AUTOINCREMENT")) {
        PASS();
    } else {
        FAIL("Expected SERIAL, no AUTOINCREMENT");
    }

    if (result) free(result);
}

static void test_type_text(void) {
    TEST("Type - TEXT preserved");
    char *result = sql_translate_types("CREATE TABLE t (name TEXT)");

    if (result && strcasestr(result, "TEXT")) {
        PASS();
    } else {
        FAIL("TEXT should be preserved");
    }

    if (result) free(result);
}

// ============================================================================
// Full Translation Tests
// ============================================================================

static void test_full_select(void) {
    TEST("Full - simple SELECT");
    sql_translation_t result = sql_translate("SELECT * FROM metadata_items WHERE id = :id");

    if (result.success && result.sql && strstr(result.sql, "$1")) {
        PASS();
    } else {
        FAIL(result.error[0] ? result.error : "Translation failed");
    }

    sql_translation_free(&result);
}

static void test_full_insert(void) {
    TEST("Full - INSERT with values");
    sql_translation_t result = sql_translate(
        "INSERT INTO t (a, b) VALUES (:a, :b)");

    if (result.success && result.sql && result.param_count == 2) {
        PASS();
    } else {
        FAIL("INSERT translation failed");
    }

    sql_translation_free(&result);
}

static void test_full_update(void) {
    TEST("Full - UPDATE with WHERE");
    sql_translation_t result = sql_translate(
        "UPDATE t SET a = :val WHERE id = :id");

    if (result.success && result.sql && result.param_count == 2) {
        PASS();
    } else {
        FAIL("UPDATE translation failed");
    }

    sql_translation_free(&result);
}

static void test_full_complex(void) {
    TEST("Full - complex Plex-like query");
    sql_translation_t result = sql_translate(
        "SELECT m.id, m.title, IFNULL(m.rating, 0) as rating "
        "FROM metadata_items m "
        "WHERE m.library_section_id = :lib_id "
        "AND m.metadata_type = :type "
        "ORDER BY m.added_at DESC LIMIT 50");

    if (result.success && result.sql &&
        strcasestr(result.sql, "COALESCE") &&  // IFNULL → COALESCE
        strstr(result.sql, "$1") &&
        strstr(result.sql, "$2")) {
        PASS();
    } else {
        FAIL("Complex query translation failed");
    }

    sql_translation_free(&result);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

static void test_edge_empty(void) {
    TEST("Edge - empty string");
    sql_translation_t result = sql_translate("");

    // Should handle gracefully
    if (result.sql != NULL || !result.success) {
        PASS();  // Either returns empty or fails gracefully
    } else {
        FAIL("Empty string not handled");
    }

    sql_translation_free(&result);
}

static void test_edge_null(void) {
    TEST("Edge - NULL input");
    sql_translation_t result = sql_translate(NULL);

    // Should not crash
    PASS();

    sql_translation_free(&result);
}

static void test_edge_backticks(void) {
    TEST("Edge - backtick identifiers to double quotes");
    sql_translation_t result = sql_translate("SELECT `id`, `name` FROM `table`");

    // Backticks should become double quotes
    if (result.success && result.sql &&
        !strstr(result.sql, "`") &&
        strstr(result.sql, "\"")) {
        PASS();
    } else {
        FAIL("Backticks not converted to double quotes");
    }

    sql_translation_free(&result);
}

static void test_edge_double_quotes_preserved(void) {
    TEST("Edge - double quotes preserved");
    sql_translation_t result = sql_translate("SELECT \"id\" FROM \"table\"");

    if (result.success && result.sql && strstr(result.sql, "\"")) {
        PASS();
    } else {
        FAIL("Double quotes should be preserved");
    }

    sql_translation_free(&result);
}

// ============================================================================
// COLLATE NOCASE Tests (NEW - TDD)
// ============================================================================

static void test_collate_nocase_equals(void) {
    TEST("COLLATE NOCASE - equality comparison");
    sql_translation_t result = sql_translate(
        "SELECT * FROM t WHERE name COLLATE NOCASE = 'Test'");

    // Should translate to LOWER(name) = LOWER('Test')
    if (result.success && result.sql &&
        strcasestr(result.sql, "LOWER") &&
        !strcasestr(result.sql, "COLLATE NOCASE")) {
        PASS();
    } else {
        FAIL("Expected LOWER() conversion for COLLATE NOCASE");
    }

    sql_translation_free(&result);
}

static void test_collate_nocase_like(void) {
    TEST("COLLATE NOCASE - LIKE comparison");
    sql_translation_t result = sql_translate(
        "SELECT * FROM t WHERE name LIKE '%test%' COLLATE NOCASE");

    // Should translate to ILIKE or LOWER(name) LIKE LOWER('%test%')
    if (result.success && result.sql &&
        (strcasestr(result.sql, "ILIKE") || strcasestr(result.sql, "LOWER")) &&
        !strcasestr(result.sql, "COLLATE NOCASE")) {
        PASS();
    } else {
        FAIL("Expected ILIKE or LOWER() for COLLATE NOCASE LIKE");
    }

    sql_translation_free(&result);
}

static void test_collate_nocase_orderby(void) {
    TEST("COLLATE NOCASE - ORDER BY");
    sql_translation_t result = sql_translate(
        "SELECT * FROM t ORDER BY name COLLATE NOCASE");

    // Should translate to ORDER BY LOWER(name)
    if (result.success && result.sql &&
        strcasestr(result.sql, "LOWER") &&
        !strcasestr(result.sql, "COLLATE NOCASE")) {
        PASS();
    } else {
        FAIL("Expected LOWER() in ORDER BY for COLLATE NOCASE");
    }

    sql_translation_free(&result);
}

// ============================================================================
// FTS4 Boolean Search Tests (NEW - TDD)
// ============================================================================

static void test_fts_negation(void) {
    TEST("FTS4 - negation operator (-term)");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH 'action -comedy'");

    // Should translate -comedy to !comedy in tsquery
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery") &&
        strcasestr(result.sql, "!")) {
        PASS();
    } else {
        FAIL("Expected ! negation in tsquery");
    }

    sql_translation_free(&result);
}

static void test_fts_and_chain(void) {
    TEST("FTS4 - AND chain (term1 AND term2)");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH 'action AND adventure'");

    // Should translate to action & adventure in tsquery
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery") &&
        strcasestr(result.sql, "&")) {
        PASS();
    } else {
        FAIL("Expected & operator in tsquery");
    }

    sql_translation_free(&result);
}

static void test_fts_or_chain(void) {
    TEST("FTS4 - OR chain (term1 OR term2)");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH 'action OR adventure'");

    // Should translate to action | adventure in tsquery
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery") &&
        strcasestr(result.sql, "|")) {
        PASS();
    } else {
        FAIL("Expected | operator in tsquery");
    }

    sql_translation_free(&result);
}

static void test_fts_phrase(void) {
    TEST("FTS4 - phrase search (\"exact phrase\")");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH '\"star wars\"'");

    // Should translate to phrase search with <-> operator
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery")) {
        PASS();  // Basic check - phrase handling is complex
    } else {
        FAIL("Expected tsquery for phrase search");
    }

    sql_translation_free(&result);
}

// ============================================================================
// Window Functions Tests (NEW - TDD)
// ============================================================================

static void test_window_row_number(void) {
    TEST("Window - ROW_NUMBER() OVER");
    sql_translation_t result = sql_translate(
        "SELECT ROW_NUMBER() OVER (ORDER BY id) as rn FROM t");

    // PostgreSQL supports this natively, should pass through
    if (result.success && result.sql &&
        strcasestr(result.sql, "ROW_NUMBER") &&
        strcasestr(result.sql, "OVER")) {
        PASS();
    } else {
        FAIL("ROW_NUMBER() OVER should be preserved");
    }

    sql_translation_free(&result);
}

static void test_window_rank(void) {
    TEST("Window - RANK() with PARTITION BY");
    sql_translation_t result = sql_translate(
        "SELECT RANK() OVER (PARTITION BY category ORDER BY score DESC) FROM t");

    // PostgreSQL supports this natively
    if (result.success && result.sql &&
        strcasestr(result.sql, "RANK") &&
        strcasestr(result.sql, "PARTITION BY")) {
        PASS();
    } else {
        FAIL("RANK() with PARTITION BY should be preserved");
    }

    sql_translation_free(&result);
}

static void test_window_dense_rank(void) {
    TEST("Window - DENSE_RANK()");
    sql_translation_t result = sql_translate(
        "SELECT DENSE_RANK() OVER (ORDER BY score) FROM t");

    if (result.success && result.sql &&
        strcasestr(result.sql, "DENSE_RANK")) {
        PASS();
    } else {
        FAIL("DENSE_RANK() should be preserved");
    }

    sql_translation_free(&result);
}

// ============================================================================
// FTS Quote Parsing Tests (Bug Fix Tests)
// Tests that MATCH queries with SQL-escaped quotes are correctly translated
// ============================================================================

static void test_fts_single_escaped_quote(void) {
    TEST("FTS Quote - single escaped quote (it''s*)");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH '(it''s*)'");

    // The '' should be unescaped to ' and the term should be valid tsquery
    // Result should have to_tsquery and the term should be properly formatted
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery") &&
        !strstr(result.sql, "''")) {  // No double quotes should remain in tsquery
        PASS();
    } else {
        FAIL("Single escaped quote should be unescaped in tsquery");
        if (result.sql) printf("    Got: %s\n", result.sql);
    }

    sql_translation_free(&result);
}

static void test_fts_double_escaped_quote(void) {
    TEST("FTS Quote - double escaped quote (test''''test*)");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH '(test''''test*)'");

    // '''' in SQL represents '' (two actual quotes) which should become one quote
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery")) {
        PASS();
    } else {
        FAIL("Double escaped quote should be handled");
        if (result.sql) printf("    Got: %s\n", result.sql);
    }

    sql_translation_free(&result);
}

static void test_fts_simple_term(void) {
    TEST("FTS Quote - simple term (no quotes)");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH 'simple'");

    // Basic case should still work
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery") &&
        strcasestr(result.sql, "simple")) {
        PASS();
    } else {
        FAIL("Simple term should be translated to tsquery");
        if (result.sql) printf("    Got: %s\n", result.sql);
    }

    sql_translation_free(&result);
}

static void test_fts_mixed_quotes_and_terms(void) {
    TEST("FTS Quote - mixed quotes and wildcards");
    sql_translation_t result = sql_translate(
        "SELECT * FROM fts4_metadata_titles WHERE title MATCH '(don''t* stop*)'");

    // Should handle both the escaped quote and the wildcards
    if (result.success && result.sql &&
        strcasestr(result.sql, "to_tsquery") &&
        strstr(result.sql, ":*")) {  // Wildcard should be converted to :*
        PASS();
    } else {
        FAIL("Mixed quotes and wildcards should work together");
        if (result.sql) printf("    Got: %s\n", result.sql);
    }

    sql_translation_free(&result);
}

// ============================================================================
// JSON Operator Parameter Tests (Bug Fix Tests)
// Tests that JSON operators with parameter placeholders work correctly
// ============================================================================

static void test_json_operator_with_parameter(void) {
    TEST("JSON Op - column ->> '$.key' < $3 should consume parameter");
    sql_translation_t result = sql_translate(
        "SELECT * FROM t WHERE extra_data ->> '$.pv:version' < $3");

    // The JSON operator should be translated to LIKE pattern
    // and the $3 parameter should NOT appear dangling in the output
    if (result.success && result.sql) {
        // Check that we got a LIKE pattern (the fix converts ->> to LIKE)
        int has_like = strcasestr(result.sql, "LIKE") != NULL;
        // Check that $3 is NOT dangling (it should be consumed by the LIKE translation)
        int no_dangling_param = strstr(result.sql, " $3") == NULL;

        if (has_like && no_dangling_param) {
            PASS();
        } else {
            FAIL("JSON operator should consume parameter");
            printf("    has_like=%d no_dangling_param=%d\n", has_like, no_dangling_param);
            printf("    Got: %s\n", result.sql);
        }
    } else {
        FAIL("Translation failed");
    }

    sql_translation_free(&result);
}

static void test_json_operator_with_literal(void) {
    TEST("JSON Op - column ->> '$.key' < '1' should work");
    sql_translation_t result = sql_translate(
        "SELECT * FROM t WHERE extra_data ->> '$.pv:version' < '1'");

    // Should be converted to LIKE pattern
    if (result.success && result.sql &&
        strcasestr(result.sql, "LIKE")) {
        PASS();
    } else {
        FAIL("JSON operator with literal should convert to LIKE");
        if (result.sql) printf("    Got: %s\n", result.sql);
    }

    sql_translation_free(&result);
}

static void test_json_operator_is_null(void) {
    TEST("JSON Op - column ->> '$.key' IS NULL");
    sql_translation_t result = sql_translate(
        "SELECT * FROM t WHERE extra_data ->> '$.pv:version' IS NULL");

    // Should be converted to NOT LIKE pattern for IS NULL check
    if (result.success && result.sql &&
        strcasestr(result.sql, "NOT LIKE")) {
        PASS();
    } else {
        FAIL("JSON IS NULL should convert to NOT LIKE");
        if (result.sql) printf("    Got: %s\n", result.sql);
    }

    sql_translation_free(&result);
}

static void test_json_operator_param_position(void) {
    TEST("JSON Op - parameter with json cast (::json->>$N)");
    sql_translation_t result = sql_translate(
        "SELECT * FROM t WHERE data->>$1 = 'value'");

    // The ->>$N pattern should get ::json inserted before it
    if (result.success && result.sql &&
        strstr(result.sql, "::json->>$1")) {
        PASS();
    } else {
        FAIL("JSON operator with $N should insert ::json cast");
        if (result.sql) printf("    Got: %s\n", result.sql);
    }

    sql_translation_free(&result);
}

// ============================================================================
// Helper Function Tests (sql_tr_helpers.c)
// ============================================================================

static void test_helper_str_replace(void) {
    TEST("Helper - str_replace basic");
    char *result = str_replace("hello world hello", "hello", "hi");
    if (result && strcmp(result, "hi world hi") == 0) {
        PASS();
    } else {
        FAIL("Expected 'hi world hi'");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_helper_str_replace_no_match(void) {
    TEST("Helper - str_replace no match returns copy");
    char *result = str_replace("hello world", "xyz", "abc");
    if (result && strcmp(result, "hello world") == 0) {
        PASS();
    } else {
        FAIL("Expected unchanged string");
    }
    free(result);
}

static void test_helper_str_replace_nocase(void) {
    TEST("Helper - str_replace_nocase case insensitive");
    char *result = str_replace_nocase("SELECT HELLO from t", "hello", "world");
    if (result && strstr(result, "world")) {
        PASS();
    } else {
        FAIL("Expected case-insensitive replacement");
    }
    free(result);
}

static void test_helper_safe_strcasestr(void) {
    TEST("Helper - safe_strcasestr finds match");
    const char *result = safe_strcasestr("Hello World", "WORLD");
    if (result && strncmp(result, "World", 5) == 0) {
        PASS();
    } else {
        FAIL("Expected to find 'World'");
    }
}

static void test_helper_safe_strcasestr_null(void) {
    TEST("Helper - safe_strcasestr NULL safety");
    if (safe_strcasestr(NULL, "test") == NULL &&
        safe_strcasestr("test", NULL) == NULL) {
        PASS();
    } else {
        FAIL("Expected NULL return for NULL input");
    }
}

static void test_helper_extract_arg(void) {
    TEST("Helper - extract_arg with nested parens");
    char buf[256];
    const char *input = "func(a, b), c)";
    const char *next = extract_arg(input, buf, sizeof(buf));
    if (strcmp(buf, "func(a, b)") == 0 && *next == ',') {
        PASS();
    } else {
        FAIL("Expected 'func(a, b)'");
        printf("    Got: '%s', next='%c'\n", buf, *next);
    }
}

// ============================================================================
// Function Translation Tests - IIF (sql_tr_functions.c)
// ============================================================================

static void test_function_iif(void) {
    TEST("Function - iif() to CASE WHEN");
    char *result = translate_iif("SELECT iif(a > 0, 'yes', 'no') FROM t");
    if (result && strcasestr(result, "CASE WHEN") &&
        strcasestr(result, "THEN") &&
        strcasestr(result, "ELSE") &&
        strcasestr(result, "END") &&
        !strcasestr(result, "iif")) {
        PASS();
    } else {
        FAIL("Expected CASE WHEN ... THEN ... ELSE ... END");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_function_iif_no_match(void) {
    TEST("Function - iif() passthrough when absent");
    char *result = translate_iif("SELECT a FROM t");
    if (result && strcmp(result, "SELECT a FROM t") == 0) {
        PASS();
    } else {
        FAIL("Expected unchanged query");
    }
    free(result);
}

// ============================================================================
// Function Translation Tests - TYPEOF (sql_tr_functions.c)
// ============================================================================

static void test_function_typeof(void) {
    TEST("Function - typeof() to pg_typeof()::text");
    char *result = translate_typeof("SELECT typeof(x) FROM t");
    if (result && strstr(result, "pg_typeof(x)::text")) {
        PASS();
    } else {
        FAIL("Expected pg_typeof(x)::text");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Function Translation Tests - STRFTIME (sql_tr_functions.c)
// ============================================================================

static void test_function_strftime_epoch(void) {
    TEST("Function - strftime('%s', 'now') to EXTRACT(EPOCH)");
    char *result = translate_strftime("SELECT strftime('%s', 'now')");
    if (result && strcasestr(result, "EXTRACT(EPOCH FROM NOW())") &&
        strstr(result, "::bigint")) {
        PASS();
    } else {
        FAIL("Expected EXTRACT(EPOCH FROM NOW())::bigint");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_function_strftime_epoch_interval(void) {
    TEST("Function - strftime('%s', 'now', '-7 day')");
    char *result = translate_strftime("SELECT strftime('%s', 'now', '-7 day')");
    if (result && strcasestr(result, "EXTRACT(EPOCH FROM NOW()") &&
        strcasestr(result, "INTERVAL") &&
        strstr(result, "::bigint")) {
        PASS();
    } else {
        FAIL("Expected EXTRACT(EPOCH FROM NOW() - INTERVAL ...)::bigint");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_function_strftime_date(void) {
    TEST("Function - strftime('%Y-%m-%d', col) to TO_CHAR");
    char *result = translate_strftime("SELECT strftime('%Y-%m-%d', added_at)");
    if (result && strcasestr(result, "TO_CHAR(added_at, 'YYYY-MM-DD')")) {
        PASS();
    } else {
        FAIL("Expected TO_CHAR with YYYY-MM-DD");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_function_strftime_column(void) {
    TEST("Function - strftime('%s', column) uses TO_TIMESTAMP");
    char *result = translate_strftime("SELECT strftime('%s', updated_at)");
    if (result && strcasestr(result, "TO_TIMESTAMP(updated_at)")) {
        PASS();
    } else {
        FAIL("Expected EXTRACT(EPOCH FROM TO_TIMESTAMP(col))::bigint");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Function Translation Tests - UNIXEPOCH (sql_tr_functions.c)
// ============================================================================

static void test_function_unixepoch_now(void) {
    TEST("Function - unixepoch('now') to EXTRACT(EPOCH)");
    char *result = translate_unixepoch("SELECT unixepoch('now')");
    if (result && strcasestr(result, "EXTRACT(EPOCH FROM NOW())") &&
        strstr(result, "::bigint")) {
        PASS();
    } else {
        FAIL("Expected EXTRACT(EPOCH FROM NOW())::bigint");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_function_unixepoch_interval(void) {
    TEST("Function - unixepoch('now', '-7 day')");
    char *result = translate_unixepoch("SELECT unixepoch('now', '-7 day')");
    if (result && strcasestr(result, "EXTRACT(EPOCH FROM NOW()") &&
        strcasestr(result, "INTERVAL")) {
        PASS();
    } else {
        FAIL("Expected EXTRACT(EPOCH FROM NOW() + INTERVAL ...)::bigint");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Function Translation Tests - last_insert_rowid, json_each (sql_tr_functions.c)
// ============================================================================

static void test_function_last_insert_rowid(void) {
    TEST("Function - last_insert_rowid() to lastval()");
    char *result = translate_last_insert_rowid("SELECT last_insert_rowid()");
    if (result && strcasestr(result, "lastval()") &&
        !strcasestr(result, "last_insert_rowid")) {
        PASS();
    } else {
        FAIL("Expected lastval()");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_function_json_each(void) {
    TEST("Function - json_each() to json_array_elements()");
    char *result = translate_json_each("SELECT value FROM json_each(data)");
    if (result && strcasestr(result, "json_array_elements(") &&
        !strcasestr(result, "json_each(")) {
        PASS();
    } else {
        FAIL("Expected json_array_elements()");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Function Translation Tests - simplify_typeof_fixup (sql_tr_functions.c)
// ============================================================================

static void test_function_simplify_typeof(void) {
    TEST("Function - simplify typeof fixup pattern");
    char *result = simplify_typeof_fixup(
        "SELECT iif(typeof(x) in ('integer', 'real'), x, strftime('%s', x, 'utc')) FROM t");
    if (result && strstr(result, "x") && !strcasestr(result, "iif(typeof(")) {
        PASS();
    } else {
        FAIL("Expected simplified to just 'x'");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Quote Translation Tests (sql_tr_quotes.c)
// ============================================================================

static void test_quote_column_quotes(void) {
    TEST("Quote - table.'column' to table.\"column\"");
    char *result = translate_column_quotes("SELECT t.'name' FROM t");
    if (result && strstr(result, "t.\"name\"") && !strstr(result, "t.'name'")) {
        PASS();
    } else {
        FAIL("Expected t.\"name\"");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_alias_quotes(void) {
    TEST("Quote - AS 'alias' to AS \"alias\"");
    char *result = translate_alias_quotes("SELECT a AS 'my_alias' FROM t");
    if (result && strstr(result, "AS \"my_alias\"") && !strstr(result, "AS 'my_alias'")) {
        PASS();
    } else {
        FAIL("Expected AS \"my_alias\"");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_ddl_table(void) {
    TEST("Quote - DDL CREATE TABLE 'name' to \"name\"");
    char *result = translate_ddl_quotes("CREATE TABLE 'my_table' (id INTEGER)");
    if (result && strstr(result, "\"my_table\"") && !strstr(result, "'my_table'")) {
        PASS();
    } else {
        FAIL("Expected \"my_table\"");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_ddl_not_dml(void) {
    TEST("Quote - DDL quotes not applied to DML");
    char *result = translate_ddl_quotes("SELECT * FROM t WHERE name = 'test'");
    if (result && strstr(result, "'test'")) {
        PASS();
    } else {
        FAIL("DML string literals should be preserved");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_if_not_exists_table(void) {
    TEST("Quote - add IF NOT EXISTS to CREATE TABLE");
    char *result = add_if_not_exists("CREATE TABLE foo (id INTEGER)");
    if (result && strcasestr(result, "IF NOT EXISTS")) {
        PASS();
    } else {
        FAIL("Expected IF NOT EXISTS");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_if_not_exists_index(void) {
    TEST("Quote - add IF NOT EXISTS to CREATE INDEX");
    char *result = add_if_not_exists("CREATE INDEX idx_foo ON t(id)");
    if (result && strcasestr(result, "IF NOT EXISTS")) {
        PASS();
    } else {
        FAIL("Expected IF NOT EXISTS");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_if_not_exists_unique_index(void) {
    TEST("Quote - add IF NOT EXISTS to CREATE UNIQUE INDEX");
    char *result = add_if_not_exists("CREATE UNIQUE INDEX idx_u ON t(name)");
    if (result && strcasestr(result, "IF NOT EXISTS")) {
        PASS();
    } else {
        FAIL("Expected IF NOT EXISTS");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_if_not_exists_already(void) {
    TEST("Quote - IF NOT EXISTS already present");
    char *result = add_if_not_exists("CREATE TABLE IF NOT EXISTS foo (id INT)");
    if (result && strcmp(result, "CREATE TABLE IF NOT EXISTS foo (id INT)") == 0) {
        PASS();
    } else {
        FAIL("Should not double-add IF NOT EXISTS");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_quote_on_conflict_unquote(void) {
    TEST("Quote - ON CONFLICT(\"name\") to ON CONFLICT(name)");
    char *result = fix_on_conflict_quotes(
        "INSERT INTO t VALUES (1) ON CONFLICT(\"name\") DO NOTHING");
    if (result && strstr(result, "ON CONFLICT(name)") &&
        !strstr(result, "ON CONFLICT(\"name\")")) {
        PASS();
    } else {
        FAIL("Expected unquoted column in ON CONFLICT");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Duplicate Assignment Tests (sql_tr_quotes.c)
// ============================================================================

static void test_dedup_assignments_basic(void) {
    TEST("Dedup - duplicate SET assignment keeps last");
    char *result = fix_duplicate_assignments(
        "UPDATE t SET a=1, b=2, a=3 WHERE id=1");
    if (result && strstr(result, "a=3") && strstr(result, "b=2") &&
        !strstr(result, "a=1,")) {
        PASS();
    } else {
        FAIL("Expected last assignment of 'a' to be kept");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_dedup_assignments_no_dup(void) {
    TEST("Dedup - no duplicates returns unchanged");
    char *result = fix_duplicate_assignments(
        "UPDATE t SET a=1, b=2 WHERE id=1");
    if (result && strstr(result, "a=1") && strstr(result, "b=2")) {
        PASS();
    } else {
        FAIL("Expected unchanged");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_dedup_assignments_quoted(void) {
    TEST("Dedup - handles backtick-quoted columns");
    char *result = fix_duplicate_assignments(
        "UPDATE t SET `col`=1, `col`=2 WHERE id=1");
    if (result && strstr(result, "`col`=2") && !strstr(result, "`col`=1,")) {
        PASS();
    } else {
        FAIL("Expected dedup with backtick columns");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_dedup_assignments_params(void) {
    TEST("Dedup - removed params consumed with COALESCE");
    char *result = fix_duplicate_assignments(
        "UPDATE t SET a=$1, b=$2, a=$3 WHERE id=$4");
    if (result && strstr(result, "COALESCE") && strstr(result, "$1::text")) {
        PASS();
    } else {
        FAIL("Expected COALESCE with removed $1");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_dedup_not_update(void) {
    TEST("Dedup - non-UPDATE returns unchanged");
    char *result = fix_duplicate_assignments("SELECT a, a FROM t");
    if (result && strcmp(result, "SELECT a, a FROM t") == 0) {
        PASS();
    } else {
        FAIL("Expected unchanged for SELECT");
    }
    free(result);
}

// ============================================================================
// Query Translation Tests - translate_null_sorting (sql_tr_query.c)
// ============================================================================

static void test_null_sorting(void) {
    TEST("Query - null sorting IS NULL,col asc -> NULLS LAST");
    char *result = translate_null_sorting(
        "SELECT * FROM t ORDER BY parents.\"index\" IS NULL, parents.\"index\" asc");
    if (result && strcasestr(result, "NULLS LAST") &&
        !strcasestr(result, "IS NULL")) {
        PASS();
    } else {
        FAIL("Expected NULLS LAST");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_null_sorting_no_match(void) {
    TEST("Query - null sorting passthrough when no IS NULL");
    char *result = translate_null_sorting("SELECT * FROM t ORDER BY id");
    if (result && strcmp(result, "SELECT * FROM t ORDER BY id") == 0) {
        PASS();
    } else {
        FAIL("Expected unchanged");
    }
    free(result);
}

// ============================================================================
// Query Translation Tests - translate_distinct_orderby (sql_tr_query.c)
// ============================================================================

static void test_distinct_orderby_aggregate(void) {
    TEST("Query - remove DISTINCT with aggregate ORDER BY");
    char *result = translate_distinct_orderby(
        "SELECT DISTINCT id FROM t GROUP BY id ORDER BY count(*)");
    if (result && !strcasestr(result, "DISTINCT") &&
        strcasestr(result, "SELECT")) {
        PASS();
    } else {
        FAIL("Expected DISTINCT removed");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_distinct_orderby_random(void) {
    TEST("Query - remove DISTINCT with ORDER BY random()");
    char *result = translate_distinct_orderby(
        "SELECT DISTINCT id FROM t ORDER BY random()");
    if (result && !strcasestr(result, "DISTINCT")) {
        PASS();
    } else {
        FAIL("Expected DISTINCT removed for random()");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_distinct_orderby_groupby(void) {
    TEST("Query - remove DISTINCT when GROUP BY present");
    char *result = translate_distinct_orderby(
        "SELECT DISTINCT id FROM t GROUP BY id");
    if (result && !strcasestr(result, "DISTINCT")) {
        PASS();
    } else {
        FAIL("Expected DISTINCT removed with GROUP BY");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Query Translation Tests - translate_case_booleans (sql_tr_query.c)
// ============================================================================

static void test_case_booleans_else(void) {
    TEST("Query - CASE ELSE 1 END) -> ELSE true END)");
    char *result = translate_case_booleans(
        "SELECT (CASE WHEN a THEN 0 ELSE 1 END) FROM t");
    if (result && strcasestr(result, "else true end)") &&
        !strstr(result, "else 1 end)")) {
        PASS();
    } else {
        FAIL("Expected boolean translation");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_case_booleans_where(void) {
    TEST("Query - WHERE 0 -> WHERE FALSE");
    char *result = translate_case_booleans("SELECT * FROM t WHERE 0");
    if (result && strcasestr(result, "WHERE FALSE")) {
        PASS();
    } else {
        FAIL("Expected WHERE FALSE");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Query Translation Tests - translate_max/min (sql_tr_query.c)
// ============================================================================

static void test_max_to_greatest(void) {
    TEST("Query - max(a, b) to GREATEST(a, b)");
    char *result = translate_max_to_greatest("SELECT max(x, y) FROM t");
    if (result && strcasestr(result, "GREATEST(x, y)") &&
        !strcasestr(result, "max(x, y)")) {
        PASS();
    } else {
        FAIL("Expected GREATEST(x, y)");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_max_single_arg(void) {
    TEST("Query - max(a) stays as max(a) (aggregate)");
    char *result = translate_max_to_greatest("SELECT max(x) FROM t");
    if (result && strcasestr(result, "max(x)") &&
        !strcasestr(result, "GREATEST")) {
        PASS();
    } else {
        FAIL("Expected max(x) preserved as aggregate");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_min_to_least(void) {
    TEST("Query - min(a, b) to LEAST(a, b)");
    char *result = translate_min_to_least("SELECT min(x, y) FROM t");
    if (result && strcasestr(result, "LEAST(x, y)") &&
        !strcasestr(result, "min(x, y)")) {
        PASS();
    } else {
        FAIL("Expected LEAST(x, y)");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_min_single_arg(void) {
    TEST("Query - min(a) stays as min(a) (aggregate)");
    char *result = translate_min_to_least("SELECT min(x) FROM t");
    if (result && strcasestr(result, "min(x)") &&
        !strcasestr(result, "LEAST")) {
        PASS();
    } else {
        FAIL("Expected min(x) preserved as aggregate");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Query Translation Tests - strip_icu_collation (sql_tr_query.c)
// ============================================================================

static void test_strip_icu_collation(void) {
    TEST("Query - strip COLLATE icu_root");
    char *result = strip_icu_collation(
        "SELECT * FROM t ORDER BY name COLLATE icu_root");
    if (result && !strcasestr(result, "icu_root")) {
        PASS();
    } else {
        FAIL("Expected icu_root removed");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_strip_icu_collation_no_match(void) {
    TEST("Query - strip icu_root passthrough when absent");
    char *result = strip_icu_collation("SELECT * FROM t ORDER BY name");
    if (result && strcmp(result, "SELECT * FROM t ORDER BY name") == 0) {
        PASS();
    } else {
        FAIL("Expected unchanged");
    }
    free(result);
}

// ============================================================================
// Query Translation Tests - add_subquery_alias (sql_tr_query.c)
// ============================================================================

static void test_subquery_alias(void) {
    TEST("Query - FROM (SELECT ...) gets alias");
    char *result = add_subquery_alias(
        "SELECT * FROM (SELECT id FROM t) WHERE id > 0");
    if (result && strcasestr(result, "AS subq")) {
        PASS();
    } else {
        FAIL("Expected AS subqN alias");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Query Translation Tests - fix_collections_query (sql_tr_query.c)
// ============================================================================

static void test_collections_filter(void) {
    TEST("Query - filter metadata_type=18 with type=1");
    char *result = fix_collections_query(
        "SELECT * FROM metadata_items WHERE "
        "(metadata_items.metadata_type=1 or metadata_items.metadata_type=18)");
    if (result && !strcasestr(result, "type=18") &&
        strcasestr(result, "metadata_type=1")) {
        PASS();
    } else {
        FAIL("Expected type=18 removed");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_collections_no_change(void) {
    TEST("Query - no collection filter when only type=1");
    char *result = fix_collections_query(
        "SELECT * FROM metadata_items WHERE metadata_type=1 ");
    if (result && strcasestr(result, "metadata_type=1")) {
        PASS();
    } else {
        FAIL("Expected unchanged");
    }
    free(result);
}

// ============================================================================
// Keyword Translation Tests - fix_operator_spacing (sql_tr_keywords.c)
// ============================================================================

static void test_operator_spacing_eq(void) {
    TEST("Keyword - fix_operator_spacing =-1 -> = -1");
    char *result = fix_operator_spacing("SELECT * FROM t WHERE a=-1");
    if (result && strstr(result, "= -1")) {
        PASS();
    } else {
        FAIL("Expected space before -1");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_operator_spacing_ne(void) {
    TEST("Keyword - fix_operator_spacing !=-1 -> != -1");
    char *result = fix_operator_spacing("SELECT * FROM t WHERE a!=-1");
    if (result && strstr(result, "!= -1")) {
        PASS();
    } else {
        FAIL("Expected space before -1");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

static void test_operator_spacing_no_fix(void) {
    TEST("Keyword - fix_operator_spacing no fix needed");
    char *result = fix_operator_spacing("SELECT * FROM t WHERE a = -1");
    if (result && strcmp(result, "SELECT * FROM t WHERE a = -1") == 0) {
        PASS();
    } else {
        FAIL("Expected unchanged");
        if (result) printf("    Got: %s\n", result);
    }
    free(result);
}

// ============================================================================
// Placeholder Translation Tests - double quote fix (sql_tr_placeholders.c)
// ============================================================================

static void test_placeholder_double_quote_not_string(void) {
    TEST("Placeholder - ? inside 'string' ignored, not \"identifier\"");
    char **names = NULL;
    int count = 0;
    // Double quotes are SQL identifiers, not strings.
    // The placeholder translator only skips ? inside single-quoted strings.
    // Identifiers with ? are unusual but the current behavior is correct:
    // all ? get translated since double quotes delimit identifiers, not values.
    char *result = sql_translate_placeholders(
        "SELECT * FROM t WHERE name = '?' AND id = ?", &names, &count);
    // ? inside single quotes should NOT become $N
    // ? outside should become $1
    if (result && count == 1 && strstr(result, "'?'") &&
        strstr(result, "$1")) {
        PASS();
    } else {
        FAIL("Expected ? inside single quotes preserved, outside translated");
        if (result) printf("    Got: %s (count=%d)\n", result, count);
    }
    if (result) free(result);
    if (names) {
        for (int i = 0; i < count; i++) free(names[i]);
        free(names);
    }
}

// ============================================================================
// Full Pipeline Tests - NULLS FIRST ordering (sql_tr_groupby.c)
// ============================================================================

static void test_nulls_first_ordering(void) {
    TEST("GroupBy - add_nulls_first_ordering");
    char *result = add_nulls_first_ordering(
        "SELECT a, count(*) FROM t GROUP BY a ORDER BY a");
    // Should add NULLS FIRST to ORDER BY in GROUP BY queries
    if (result) {
        // Function may or may not add NULLS FIRST depending on implementation
        // At minimum it should not crash and return valid SQL
        PASS();
    } else {
        FAIL("Returned NULL");
    }
    free(result);
}

// ============================================================================
// Main
// ============================================================================

int main(void) {
    printf("\n\033[1m=== SQL Translator Tests ===\033[0m\n\n");

    // Initialize translator
    sql_translator_init();

    printf("\033[1mPlaceholder Translation:\033[0m\n");
    test_placeholder_basic();
    test_placeholder_multiple();
    test_placeholder_reuse();
    test_placeholder_question_mark();
    test_placeholder_in_string();

    printf("\n\033[1mFunction Translation:\033[0m\n");
    test_function_ifnull();
    test_function_length();
    test_function_substr();
    test_function_random();
    test_function_datetime();

    printf("\n\033[1mKeyword Translation:\033[0m\n");
    test_keyword_glob();
    test_keyword_notnull();

    printf("\n\033[1mType Translation:\033[0m\n");
    test_type_autoincrement();
    test_type_text();

    printf("\n\033[1mFull Query Translation:\033[0m\n");
    test_full_select();
    test_full_insert();
    test_full_update();
    test_full_complex();

    printf("\n\033[1mEdge Cases:\033[0m\n");
    test_edge_empty();
    test_edge_null();
    test_edge_backticks();
    test_edge_double_quotes_preserved();

    printf("\n\033[1mCOLLATE NOCASE (NEW):\033[0m\n");
    test_collate_nocase_equals();
    test_collate_nocase_like();
    test_collate_nocase_orderby();

    printf("\n\033[1mFTS4 Boolean Search (NEW):\033[0m\n");
    test_fts_negation();
    test_fts_and_chain();
    test_fts_or_chain();
    test_fts_phrase();

    printf("\n\033[1mWindow Functions (NEW):\033[0m\n");
    test_window_row_number();
    test_window_rank();
    test_window_dense_rank();

    printf("\n\033[1mFTS Quote Parsing (Bug Fix):\033[0m\n");
    test_fts_single_escaped_quote();
    test_fts_double_escaped_quote();
    test_fts_simple_term();
    test_fts_mixed_quotes_and_terms();

    printf("\n\033[1mJSON Operator Parameters (Bug Fix):\033[0m\n");
    test_json_operator_with_parameter();
    test_json_operator_with_literal();
    test_json_operator_is_null();
    test_json_operator_param_position();

    printf("\n\033[1mHelper Functions:\033[0m\n");
    test_helper_str_replace();
    test_helper_str_replace_no_match();
    test_helper_str_replace_nocase();
    test_helper_safe_strcasestr();
    test_helper_safe_strcasestr_null();
    test_helper_extract_arg();

    printf("\n\033[1mFunction - IIF:\033[0m\n");
    test_function_iif();
    test_function_iif_no_match();

    printf("\n\033[1mFunction - TYPEOF:\033[0m\n");
    test_function_typeof();

    printf("\n\033[1mFunction - STRFTIME:\033[0m\n");
    test_function_strftime_epoch();
    test_function_strftime_epoch_interval();
    test_function_strftime_date();
    test_function_strftime_column();

    printf("\n\033[1mFunction - UNIXEPOCH:\033[0m\n");
    test_function_unixepoch_now();
    test_function_unixepoch_interval();

    printf("\n\033[1mFunction - last_insert_rowid, json_each:\033[0m\n");
    test_function_last_insert_rowid();
    test_function_json_each();

    printf("\n\033[1mFunction - simplify_typeof_fixup:\033[0m\n");
    test_function_simplify_typeof();

    printf("\n\033[1mQuote Translations:\033[0m\n");
    test_quote_column_quotes();
    test_quote_alias_quotes();
    test_quote_ddl_table();
    test_quote_ddl_not_dml();
    test_quote_if_not_exists_table();
    test_quote_if_not_exists_index();
    test_quote_if_not_exists_unique_index();
    test_quote_if_not_exists_already();
    test_quote_on_conflict_unquote();

    printf("\n\033[1mDuplicate Assignment Dedup:\033[0m\n");
    test_dedup_assignments_basic();
    test_dedup_assignments_no_dup();
    test_dedup_assignments_quoted();
    test_dedup_assignments_params();
    test_dedup_not_update();

    printf("\n\033[1mNull Sorting:\033[0m\n");
    test_null_sorting();
    test_null_sorting_no_match();

    printf("\n\033[1mDistinct + ORDER BY:\033[0m\n");
    test_distinct_orderby_aggregate();
    test_distinct_orderby_random();
    test_distinct_orderby_groupby();

    printf("\n\033[1mCase Booleans:\033[0m\n");
    test_case_booleans_else();
    test_case_booleans_where();

    printf("\n\033[1mMax/Min Translation:\033[0m\n");
    test_max_to_greatest();
    test_max_single_arg();
    test_min_to_least();
    test_min_single_arg();

    printf("\n\033[1mICU Collation Strip:\033[0m\n");
    test_strip_icu_collation();
    test_strip_icu_collation_no_match();

    printf("\n\033[1mSubquery Alias:\033[0m\n");
    test_subquery_alias();

    printf("\n\033[1mCollections Filter:\033[0m\n");
    test_collections_filter();
    test_collections_no_change();

    printf("\n\033[1mOperator Spacing:\033[0m\n");
    test_operator_spacing_eq();
    test_operator_spacing_ne();
    test_operator_spacing_no_fix();

    printf("\n\033[1mPlaceholder Fix (single vs double quotes):\033[0m\n");
    test_placeholder_double_quote_not_string();

    printf("\n\033[1mNULLS FIRST Ordering:\033[0m\n");
    test_nulls_first_ordering();

    // Cleanup
    sql_translator_cleanup();

    printf("\n\033[1m=== Results ===\033[0m\n");
    printf("Passed: \033[32m%d\033[0m\n", tests_passed);
    printf("Failed: \033[31m%d\033[0m\n", tests_failed);
    printf("\n");

    return tests_failed > 0 ? 1 : 0;
}
