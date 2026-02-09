/*
 * Comprehensive unit tests for INSERT OR REPLACE → ON CONFLICT translation
 *
 * Tests translate_insert_or_replace() from sql_tr_upsert.c covering:
 * - All 28 table mappings in conflict_targets[]
 * - Schema prefix stripping (plex.table → table)
 * - Special column handling (updated_at COALESCE, view_count GREATEST)
 * - metadata_item_settings special case
 * - Default fallback for unknown tables
 * - Edge cases: NULL, non-INSERT, no column list
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "sql_translator.h"
#include "sql_translator_internal.h"

// Test counters
static int tests_passed = 0;
static int tests_failed = 0;

#define TEST(name) printf("  Testing: %-60s ", name)
#define PASS() do { printf("\033[32mPASS\033[0m\n"); tests_passed++; } while(0)
#define FAIL(msg) do { printf("\033[31mFAIL: %s\033[0m\n", msg); tests_failed++; } while(0)

// Helper: check that result contains a substring (case-insensitive)
static int contains(const char *haystack, const char *needle) {
    if (!haystack || !needle) return 0;
    return strcasestr(haystack, needle) != NULL;
}

// Helper: check that result contains a substring (case-sensitive)
static int contains_exact(const char *haystack, const char *needle) {
    if (!haystack || !needle) return 0;
    return strstr(haystack, needle) != NULL;
}

// ============================================================================
// Edge Cases
// ============================================================================

static void test_null_input(void) {
    TEST("NULL input → NULL");
    char *result = translate_insert_or_replace(NULL);
    if (result == NULL) { PASS(); } else { FAIL("Expected NULL"); free(result); }
}

static void test_non_insert(void) {
    TEST("SELECT → returns unchanged");
    char *result = translate_insert_or_replace("SELECT * FROM metadata_items");
    if (result && strcmp(result, "SELECT * FROM metadata_items") == 0) { PASS(); }
    else { FAIL("Expected unchanged SQL"); }
    free(result);
}

static void test_plain_insert(void) {
    TEST("Plain INSERT (no OR REPLACE) → returns unchanged");
    char *result = translate_insert_or_replace("INSERT INTO tags (id, tag) VALUES (1, 'test')");
    if (result && strcmp(result, "INSERT INTO tags (id, tag) VALUES (1, 'test')") == 0) { PASS(); }
    else { FAIL("Expected unchanged SQL"); }
    free(result);
}

static void test_no_column_list(void) {
    TEST("INSERT OR REPLACE without column list → fallback strips OR REPLACE");
    char *result = translate_insert_or_replace("INSERT OR REPLACE INTO tags VALUES (1, 'test', 0)");
    if (result && contains(result, "INSERT INTO") && !contains(result, "OR REPLACE")) { PASS(); }
    else { FAIL("Expected OR REPLACE stripped"); }
    free(result);
}

// ============================================================================
// All 28 Table Mappings — id-based conflict targets
// ============================================================================

// Helper to test a table with (id) conflict target
static void test_id_table(const char *table_name, const char *display_name) {
    char test_name[128];
    snprintf(test_name, sizeof(test_name), "%s → ON CONFLICT (id)", display_name);
    TEST(test_name);

    char sql[512];
    snprintf(sql, sizeof(sql),
        "INSERT OR REPLACE INTO %s (id, name, created_at) VALUES (1, 'test', 12345)",
        table_name);

    char *result = translate_insert_or_replace(sql);
    if (result && contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        !contains(result, "OR REPLACE") &&
        contains(result, "INSERT INTO")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_all_id_tables(void) {
    printf("\n\033[1mTables with (id) conflict target:\033[0m\n");

    test_id_table("metadata_items",             "metadata_items");
    test_id_table("media_items",                "media_items");
    test_id_table("media_parts",                "media_parts");
    test_id_table("media_streams",              "media_streams");
    test_id_table("tags",                       "tags");
    test_id_table("taggings",                   "taggings");
    test_id_table("statistics_media",           "statistics_media");
    test_id_table("statistics_resources",       "statistics_resources");
    test_id_table("play_queue_generators",      "play_queue_generators");
    test_id_table("play_queue_items",           "play_queue_items");
    test_id_table("play_queues",                "play_queues");
    test_id_table("activities",                 "activities");
    test_id_table("accounts",                   "accounts");
    test_id_table("devices",                    "devices");
    test_id_table("directories",               "directories");
    test_id_table("library_sections",           "library_sections");
    test_id_table("locations",                  "locations");
    test_id_table("plugins",                    "plugins");
    test_id_table("media_grabs",               "media_grabs");
    test_id_table("metadata_relations",         "metadata_relations");
    test_id_table("versioned_metadata_items",   "versioned_metadata_items");
    test_id_table("external_metadata_sources",  "external_metadata_sources");
    test_id_table("blobs",                      "blobs");
}

// ============================================================================
// Tables with composite UNIQUE conflict targets
// ============================================================================

static void test_statistics_bandwidth(void) {
    printf("\n\033[1mTables with composite/unique conflict targets:\033[0m\n");

    TEST("statistics_bandwidth → ON CONFLICT (account_id, device_id, ...)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO statistics_bandwidth "
        "(id, account_id, device_id, timespan, at, lan, bytes) "
        "VALUES (1, 2, 3, 4, 5, 1, 1024)");

    if (result &&
        contains(result, "ON CONFLICT (account_id, device_id, timespan, at, lan) DO UPDATE SET") &&
        !contains(result, "OR REPLACE")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_locatables(void) {
    TEST("locatables → ON CONFLICT (location_id, locatable_id, locatable_type)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO locatables "
        "(id, location_id, locatable_id, locatable_type, created_at) "
        "VALUES (1, 10, 20, 'MediaItem', 12345)");

    if (result &&
        contains(result, "ON CONFLICT (location_id, locatable_id, locatable_type) DO UPDATE SET") &&
        !contains(result, "OR REPLACE")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_location_places(void) {
    TEST("location_places → ON CONFLICT (location_id, guid)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO location_places "
        "(id, location_id, guid, name) "
        "VALUES (1, 10, 'abc-123', 'Home')");

    if (result &&
        contains(result, "ON CONFLICT (location_id, guid) DO UPDATE SET")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_media_stream_settings(void) {
    TEST("media_stream_settings → ON CONFLICT (media_stream_id, account_id)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO media_stream_settings "
        "(id, media_stream_id, account_id, selected) "
        "VALUES (1, 100, 1, 1)");

    if (result &&
        contains(result, "ON CONFLICT (media_stream_id, account_id) DO UPDATE SET")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_preferences(void) {
    TEST("preferences → ON CONFLICT (name)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO preferences "
        "(id, name, value) "
        "VALUES (1, 'FriendlyName', 'My Plex')");

    if (result &&
        contains(result, "ON CONFLICT (name) DO UPDATE SET")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// metadata_item_settings special case
// ============================================================================

static void test_metadata_item_settings_strips_or_replace(void) {
    printf("\n\033[1mmetadata_item_settings special case:\033[0m\n");

    TEST("metadata_item_settings → strips OR REPLACE, no ON CONFLICT");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO metadata_item_settings "
        "(id, account_id, guid, rating, view_count) "
        "VALUES (1, 1, 'plex://movie/abc', 8.0, 5)");

    if (result &&
        contains(result, "INSERT INTO metadata_item_settings") &&
        !contains(result, "OR REPLACE") &&
        !contains(result, "ON CONFLICT")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_metadata_item_settings_preserves_values(void) {
    TEST("metadata_item_settings → preserves column list + VALUES");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO metadata_item_settings "
        "(id, account_id, guid, rating) VALUES (1, 1, 'guid', 5.0)");

    if (result &&
        contains(result, "account_id") &&
        contains(result, "rating") &&
        contains(result, "VALUES")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Schema prefix stripping (plex.table → table)
// ============================================================================

static void test_schema_prefix(void) {
    printf("\n\033[1mSchema prefix handling:\033[0m\n");

    TEST("plex.tags → resolved to tags, ON CONFLICT (id)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO plex.tags (id, tag, tag_type) VALUES (1, 'Action', 0)");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_schema_prefix_composite(void) {
    TEST("plex.preferences → resolved, ON CONFLICT (name)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO plex.preferences (id, name, value) VALUES (1, 'key', 'val')");

    if (result &&
        contains(result, "ON CONFLICT (name) DO UPDATE SET")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_schema_prefix_metadata_item_settings(void) {
    TEST("plex.metadata_item_settings → special case still works");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO plex.metadata_item_settings "
        "(id, account_id, guid, rating) VALUES (1, 1, 'guid', 5.0)");

    if (result &&
        contains(result, "INSERT INTO") &&
        !contains(result, "OR REPLACE") &&
        !contains(result, "ON CONFLICT")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Special column handling in SET clause
// ============================================================================

static void test_updated_at_coalesce(void) {
    printf("\n\033[1mSpecial column handling in SET clause:\033[0m\n");

    TEST("updated_at → COALESCE(EXCLUDED.updated_at, ...)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags "
        "(id, tag, tag_type, updated_at) "
        "VALUES (1, 'Action', 0, 12345)");

    if (result &&
        contains(result, "COALESCE(EXCLUDED.updated_at")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_changed_at_coalesce(void) {
    TEST("changed_at → COALESCE(EXCLUDED.changed_at, ...)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO metadata_items "
        "(id, title, changed_at) "
        "VALUES (1, 'Movie', 12345)");

    if (result &&
        contains(result, "COALESCE(EXCLUDED.changed_at")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_view_count_greatest(void) {
    TEST("view_count → GREATEST(EXCLUDED.view_count, ...)");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags "
        "(id, tag, view_count) "
        "VALUES (1, 'Action', 5)");

    if (result &&
        contains(result, "GREATEST(EXCLUDED.view_count")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_regular_column_excluded(void) {
    TEST("Regular column → col = EXCLUDED.col");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags "
        "(id, tag, tag_type) "
        "VALUES (1, 'Action', 0)");

    if (result &&
        contains_exact(result, "tag = EXCLUDED.tag") &&
        contains_exact(result, "tag_type = EXCLUDED.tag_type")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_id_column_skipped_in_set(void) {
    TEST("id column → skipped in SET clause");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags "
        "(id, tag) "
        "VALUES (1, 'Action')");

    if (result &&
        contains(result, "DO UPDATE SET") &&
        !contains_exact(result, "id = EXCLUDED.id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_conflict_columns_skipped_in_set(void) {
    TEST("Composite conflict cols → skipped in SET clause");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO statistics_bandwidth "
        "(id, account_id, device_id, timespan, at, lan, bytes) "
        "VALUES (1, 2, 3, 4, 5, 1, 1024)");

    // account_id, device_id, timespan, at, lan should be skipped
    // bytes should be in SET clause
    if (result &&
        contains_exact(result, "bytes = EXCLUDED.bytes") &&
        !contains_exact(result, "account_id = EXCLUDED.account_id") &&
        !contains_exact(result, "device_id = EXCLUDED.device_id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// RETURNING id
// ============================================================================

static void test_returning_id_for_id_conflict(void) {
    printf("\n\033[1mRETURNING id:\033[0m\n");

    TEST("(id) conflict → appends RETURNING id");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags (id, tag) VALUES (1, 'Action')");

    if (result && contains(result, "RETURNING id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_no_returning_for_non_id_conflict(void) {
    TEST("(name) conflict → no RETURNING id");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO preferences (id, name, value) VALUES (1, 'key', 'val')");

    // preferences has ON CONFLICT (name) — no 'id' in conflict columns → no RETURNING
    // But wait — the code checks: if (strcasestr(target->conflict_columns, "id") != NULL)
    // "name" doesn't contain "id", so no RETURNING id. Good.
    if (result && !contains(result, "RETURNING id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_returning_for_composite_with_id(void) {
    TEST("Composite conflict with account_id → has RETURNING id");
    // statistics_bandwidth conflict cols contain "account_id" which contains "id"
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO statistics_bandwidth "
        "(id, account_id, device_id, timespan, at, lan, bytes) "
        "VALUES (1, 2, 3, 4, 5, 1, 1024)");

    // "account_id, device_id, timespan, at, lan" — strcasestr for "id" finds "account_id"
    // So this WILL have RETURNING id (the code checks substring, not exact match)
    if (result && contains(result, "RETURNING id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Unknown table → default fallback (id)
// ============================================================================

static void test_unknown_table_default(void) {
    printf("\n\033[1mDefault fallback for unknown tables:\033[0m\n");

    TEST("unknown_table → ON CONFLICT (id) as default");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO some_unknown_table (id, data) VALUES (1, 'test')");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        !contains(result, "OR REPLACE")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_unknown_table_set_clause(void) {
    TEST("unknown_table → SET skips id, includes others");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO some_unknown_table (id, data, value) VALUES (1, 'test', 42)");

    if (result &&
        contains_exact(result, "data = EXCLUDED.data") &&
        contains_exact(result, "value = EXCLUDED.value") &&
        !contains_exact(result, "id = EXCLUDED.id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Case insensitivity
// ============================================================================

static void test_case_insensitive_keyword(void) {
    printf("\n\033[1mCase handling:\033[0m\n");

    TEST("insert or replace INTO → works (mixed case)");
    char *result = translate_insert_or_replace(
        "insert or replace INTO tags (id, tag) VALUES (1, 'Action')");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        !contains(result, "or replace")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_case_insensitive_table(void) {
    TEST("METADATA_ITEMS (uppercase table) → resolved correctly");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO METADATA_ITEMS (id, title) VALUES (1, 'Test')");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Quoted column names
// ============================================================================

static void test_quoted_columns(void) {
    printf("\n\033[1mQuoted column names:\033[0m\n");

    TEST("Quoted columns → parsed correctly");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags "
        "(\"id\", \"tag\", \"tag_type\") "
        "VALUES (1, 'Action', 0)");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        contains_exact(result, "tag = EXCLUDED.tag")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_mixed_quoted_columns(void) {
    TEST("Mixed quoted/unquoted columns → all parsed");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags "
        "(\"id\", tag, \"tag_type\", created_at) "
        "VALUES (1, 'Action', 0, 12345)");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        contains_exact(result, "tag = EXCLUDED.tag") &&
        contains_exact(result, "tag_type = EXCLUDED.tag_type") &&
        contains_exact(result, "created_at = EXCLUDED.created_at")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Semicolons and trailing whitespace
// ============================================================================

static void test_trailing_semicolon(void) {
    printf("\n\033[1mTrailing content handling:\033[0m\n");

    TEST("Trailing semicolon → stripped before ON CONFLICT");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags (id, tag) VALUES (1, 'Action');");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        !contains_exact(result, "; ON CONFLICT")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_trailing_whitespace(void) {
    TEST("Trailing whitespace → handled cleanly");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO tags (id, tag) VALUES (1, 'Action')   ");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Realistic Plex SQL patterns
// ============================================================================

static void test_real_plex_media_items(void) {
    printf("\n\033[1mRealistic Plex SQL patterns:\033[0m\n");

    TEST("Real media_items INSERT → correct translation");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO media_items "
        "(id, library_section_id, section_location_id, metadata_item_id, "
        "media_type, width, height, size, duration, bitrate, "
        "container, video_codec, audio_codec, display_aspect_ratio, "
        "frames_per_second, audio_channels, \"index\", created_at, updated_at) "
        "VALUES (100, 1, 1, 50, 1, 1920, 1080, 5000000, 7200, 5000, "
        "'mkv', 'h264', 'aac', 1.78, 23.976, 2, 0, 1234567890, 1234567890)");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        contains(result, "COALESCE(EXCLUDED.updated_at") &&
        !contains_exact(result, "id = EXCLUDED.id") &&
        contains(result, "RETURNING id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_real_plex_statistics_bandwidth(void) {
    TEST("Real statistics_bandwidth INSERT → composite conflict");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO statistics_bandwidth "
        "(id, account_id, device_id, timespan, at, lan, bytes, created_at, updated_at) "
        "VALUES (500, 1, 42, 3600, 1234567890, 1, 1048576, 1234567890, 1234567890)");

    if (result &&
        contains(result, "ON CONFLICT (account_id, device_id, timespan, at, lan)") &&
        contains(result, "DO UPDATE SET") &&
        contains(result, "bytes = EXCLUDED.bytes") &&
        contains(result, "COALESCE(EXCLUDED.updated_at")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

static void test_real_plex_metadata_items(void) {
    TEST("Real metadata_items INSERT → large column set");
    char *result = translate_insert_or_replace(
        "INSERT OR REPLACE INTO metadata_items "
        "(id, metadata_type, title, title_sort, original_title, "
        "studio, rating, summary, tagline, year, \"index\", "
        "library_section_id, created_at, updated_at, changed_at) "
        "VALUES (1, 1, 'Test Movie', 'test movie', 'Original', "
        "'Studio', 8.5, 'A summary', 'A tagline', 2024, NULL, "
        "1, 1234567890, 1234567890, 1234567890)");

    if (result &&
        contains(result, "ON CONFLICT (id) DO UPDATE SET") &&
        contains(result, "title = EXCLUDED.title") &&
        contains(result, "COALESCE(EXCLUDED.updated_at") &&
        contains(result, "COALESCE(EXCLUDED.changed_at") &&
        !contains_exact(result, "id = EXCLUDED.id")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "Got: %.200s", result ? result : "NULL");
        FAIL(msg);
    }
    free(result);
}

// ============================================================================
// Full pipeline test via sql_translate()
// ============================================================================

static void test_full_pipeline(void) {
    printf("\n\033[1mFull pipeline (sql_translate):\033[0m\n");

    TEST("Full pipeline: INSERT OR REPLACE → complete translation");
    sql_translation_t t = sql_translate(
        "INSERT OR REPLACE INTO tags (id, tag, tag_type) VALUES (1, 'Action', 0)");

    if (t.success && t.sql &&
        contains(t.sql, "ON CONFLICT") &&
        !contains(t.sql, "OR REPLACE")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "success=%d sql=%.200s", t.success, t.sql ? t.sql : "NULL");
        FAIL(msg);
    }
    sql_translation_free(&t);
}

static void test_full_pipeline_metadata_settings(void) {
    TEST("Full pipeline: metadata_item_settings → handled by custom logic");
    sql_translation_t t = sql_translate(
        "INSERT OR REPLACE INTO metadata_item_settings "
        "(id, account_id, guid, rating) VALUES (1, 1, 'guid', 5.0)");

    if (t.success && t.sql &&
        !contains(t.sql, "OR REPLACE")) {
        PASS();
    } else {
        char msg[256];
        snprintf(msg, sizeof(msg), "success=%d sql=%.200s", t.success, t.sql ? t.sql : "NULL");
        FAIL(msg);
    }
    sql_translation_free(&t);
}

// ============================================================================
// Main
// ============================================================================

int main(void) {
    printf("==========================================================\n");
    printf("INSERT OR REPLACE → ON CONFLICT Translation Tests\n");
    printf("==========================================================\n");

    sql_translator_init();

    // Edge cases
    printf("\n\033[1mEdge cases:\033[0m\n");
    test_null_input();
    test_non_insert();
    test_plain_insert();
    test_no_column_list();

    // All 23 id-based tables
    test_all_id_tables();

    // 5 composite/unique tables
    test_statistics_bandwidth();
    test_locatables();
    test_location_places();
    test_media_stream_settings();
    test_preferences();

    // metadata_item_settings special case
    test_metadata_item_settings_strips_or_replace();
    test_metadata_item_settings_preserves_values();

    // Schema prefix
    test_schema_prefix();
    test_schema_prefix_composite();
    test_schema_prefix_metadata_item_settings();

    // Special column handling
    test_updated_at_coalesce();
    test_changed_at_coalesce();
    test_view_count_greatest();
    test_regular_column_excluded();
    test_id_column_skipped_in_set();
    test_conflict_columns_skipped_in_set();

    // RETURNING id
    test_returning_id_for_id_conflict();
    test_no_returning_for_non_id_conflict();
    test_returning_for_composite_with_id();

    // Unknown table default
    test_unknown_table_default();
    test_unknown_table_set_clause();

    // Case handling
    test_case_insensitive_keyword();
    test_case_insensitive_table();

    // Quoted columns
    test_quoted_columns();
    test_mixed_quoted_columns();

    // Trailing content
    test_trailing_semicolon();
    test_trailing_whitespace();

    // Realistic Plex SQL
    test_real_plex_media_items();
    test_real_plex_statistics_bandwidth();
    test_real_plex_metadata_items();

    // Full pipeline
    test_full_pipeline();
    test_full_pipeline_metadata_settings();

    sql_translator_cleanup();

    printf("\n\033[1m=== Results ===\033[0m\n");
    printf("Passed: \033[32m%d\033[0m\n", tests_passed);
    printf("Failed: \033[31m%d\033[0m\n", tests_failed);

    return tests_failed > 0 ? 1 : 0;
}
