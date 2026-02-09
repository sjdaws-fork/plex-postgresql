/*
 * Unit Tests for pg_statement.c helper functions
 * Tests: convert_metadata_settings_insert_to_upsert(), extract_metadata_id_from_generator_sql()
 *
 * These are pure string transformation functions critical for:
 * - metadata_item_settings UPSERT (watch state, ratings, play positions)
 * - play_queue metadata ID extraction
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

// We need sqlite3_int64 type
#include <stdint.h>
typedef int64_t sqlite3_int64;

// ============================================================================
// Copy of functions from pg_statement.c
// Copied here to avoid pulling in libpq/sqlite3/pthread dependencies
// ============================================================================

char* convert_metadata_settings_insert_to_upsert(const char *sql) {
    if (!sql) return NULL;
    if (!strcasestr(sql, "INSERT INTO")) return NULL;
    if (!strcasestr(sql, "metadata_item_settings")) return NULL;
    if (strcasestr(sql, "ON CONFLICT")) return NULL;
    if (strcasestr(sql, "RETURNING")) return NULL;

    static const char *on_conflict =
        " ON CONFLICT (account_id, guid) DO UPDATE SET "
        "rating = COALESCE(EXCLUDED.rating, plex.metadata_item_settings.rating), "
        "view_offset = EXCLUDED.view_offset, "
        "view_count = CASE WHEN plex.metadata_item_settings.view_count > 0 AND EXCLUDED.view_count = 0 "
                     "THEN 0 ELSE GREATEST(EXCLUDED.view_count, plex.metadata_item_settings.view_count, 1) END, "
        "last_viewed_at = CASE WHEN plex.metadata_item_settings.view_count > 0 AND EXCLUDED.view_count = 0 "
                         "THEN NULL ELSE COALESCE(EXCLUDED.last_viewed_at, EXTRACT(EPOCH FROM NOW())::bigint) END, "
        "updated_at = COALESCE(EXCLUDED.updated_at, EXTRACT(EPOCH FROM NOW())::bigint), "
        "skip_count = EXCLUDED.skip_count, "
        "last_skipped_at = EXCLUDED.last_skipped_at, "
        "changed_at = COALESCE(EXCLUDED.changed_at, EXTRACT(EPOCH FROM NOW())::bigint), "
        "extra_data = COALESCE(EXCLUDED.extra_data, plex.metadata_item_settings.extra_data), "
        "last_rated_at = COALESCE(EXCLUDED.last_rated_at, plex.metadata_item_settings.last_rated_at) "
        "RETURNING id";

    size_t len = strlen(sql) + strlen(on_conflict) + 1;
    char *result = malloc(len);
    if (result) {
        snprintf(result, len, "%s%s", sql, on_conflict);
    }
    return result;
}

sqlite3_int64 extract_metadata_id_from_generator_sql(const char *sql) {
    if (!sql) return 0;
    if (!strcasestr(sql, "play_queue_generators")) return 0;
    if (!strcasestr(sql, "INSERT")) return 0;

    const char *pattern = "%2Fmetadata%2F";
    const char *pos = strstr(sql, pattern);
    if (!pos) {
        pattern = "/metadata/";
        pos = strstr(sql, pattern);
    }
    if (!pos) return 0;

    pos += strlen(pattern);
    sqlite3_int64 id = 0;
    while (*pos >= '0' && *pos <= '9') {
        id = id * 10 + (*pos - '0');
        pos++;
    }
    return id;
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
    printf(BOLD "=== Statement Helper Function Tests ===" RESET "\n\n");

    // ========================================================================
    // convert_metadata_settings_insert_to_upsert()
    // ========================================================================
    printf(BOLD "convert_metadata_settings_insert_to_upsert():" RESET "\n");

    char *result;

    // NULL
    test("NULL -> NULL",
         convert_metadata_settings_insert_to_upsert(NULL) == NULL);

    // Not an INSERT
    test("SELECT -> NULL",
         convert_metadata_settings_insert_to_upsert("SELECT * FROM metadata_item_settings") == NULL);

    // INSERT into wrong table
    test("INSERT into other table -> NULL",
         convert_metadata_settings_insert_to_upsert("INSERT INTO metadata_items (id) VALUES (1)") == NULL);

    // Already has ON CONFLICT
    test("Already has ON CONFLICT -> NULL",
         convert_metadata_settings_insert_to_upsert(
             "INSERT INTO metadata_item_settings (id) VALUES (1) ON CONFLICT DO NOTHING"
         ) == NULL);

    // Already has RETURNING
    test("Already has RETURNING -> NULL",
         convert_metadata_settings_insert_to_upsert(
             "INSERT INTO metadata_item_settings (id) VALUES (1) RETURNING id"
         ) == NULL);

    // Valid INSERT - should get ON CONFLICT appended
    result = convert_metadata_settings_insert_to_upsert(
        "INSERT INTO metadata_item_settings (id, account_id, guid, rating) VALUES ($1, $2, $3, $4)"
    );
    test("Valid INSERT gets ON CONFLICT clause",
         result != NULL && strstr(result, "ON CONFLICT (account_id, guid) DO UPDATE SET") != NULL);
    test("Result contains RETURNING id",
         result != NULL && strstr(result, "RETURNING id") != NULL);
    test("Result preserves original SQL prefix",
         result != NULL && strstr(result, "INSERT INTO metadata_item_settings") != NULL);
    test("Result handles rating with COALESCE",
         result != NULL && strstr(result, "rating = COALESCE(EXCLUDED.rating") != NULL);
    test("Result handles view_count with GREATEST",
         result != NULL && strstr(result, "GREATEST(EXCLUDED.view_count") != NULL);
    test("Result handles last_viewed_at with CASE",
         result != NULL && strstr(result, "CASE WHEN") != NULL);
    free(result);

    // Real Plex INSERT with all columns
    result = convert_metadata_settings_insert_to_upsert(
        "INSERT INTO metadata_item_settings "
        "(id, account_id, guid, rating, view_offset, view_count, last_viewed_at, "
        "created_at, updated_at, skip_count, last_skipped_at, changed_at, extra_data, last_rated_at) "
        "VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"
    );
    test("Real Plex INSERT -> non-NULL result",
         result != NULL);
    test("Real Plex INSERT -> has ON CONFLICT",
         result != NULL && strcasestr(result, "ON CONFLICT") != NULL);
    free(result);

    // ========================================================================
    // extract_metadata_id_from_generator_sql()
    // ========================================================================
    printf("\n" BOLD "extract_metadata_id_from_generator_sql():" RESET "\n");

    // NULL
    test("NULL -> 0",
         extract_metadata_id_from_generator_sql(NULL) == 0);

    // Not a play_queue_generators query
    test("Regular INSERT -> 0",
         extract_metadata_id_from_generator_sql("INSERT INTO metadata_items (id) VALUES (1)") == 0);

    // Not an INSERT
    test("SELECT from play_queue_generators -> 0",
         extract_metadata_id_from_generator_sql("SELECT * FROM play_queue_generators") == 0);

    // URL-encoded metadata pattern
    test("URL-encoded %2Fmetadata%2F123 -> 123",
         extract_metadata_id_from_generator_sql(
             "INSERT INTO play_queue_generators (uri) VALUES ('server://host/com.plexapp.plugins.library/library/metadata%2Fmetadata%2F123')"
         ) == 123);

    // Regular path metadata pattern
    test("Regular /metadata/456 -> 456",
         extract_metadata_id_from_generator_sql(
             "INSERT INTO play_queue_generators (uri) VALUES ('server://host/library/metadata/456')"
         ) == 456);

    // Larger metadata ID
    test("Large ID /metadata/98765 -> 98765",
         extract_metadata_id_from_generator_sql(
             "INSERT INTO play_queue_generators (uri) VALUES ('/metadata/98765')"
         ) == 98765);

    // URL-encoded with full Plex URI
    test("Full Plex URI with encoded path -> correct ID",
         extract_metadata_id_from_generator_sql(
             "INSERT INTO play_queue_generators (play_queue_id, uri, type) "
             "VALUES ($1, 'server://abc123/com.plexapp.plugins.library/library/metadata%2Fmetadata%2F42', $2)"
         ) == 42);

    // No metadata pattern
    test("INSERT play_queue_generators without /metadata/ -> 0",
         extract_metadata_id_from_generator_sql(
             "INSERT INTO play_queue_generators (play_queue_id, uri) VALUES (1, 'server://host/library/sections/1')"
         ) == 0);

    // ID at end of string
    test("Metadata ID at end of string -> correct",
         extract_metadata_id_from_generator_sql(
             "INSERT INTO play_queue_generators (uri) VALUES ('/metadata/999')"
         ) == 999);

    // ID followed by non-digit
    test("Metadata ID followed by quote -> correct",
         extract_metadata_id_from_generator_sql(
             "INSERT INTO play_queue_generators (uri) VALUES ('/metadata/777')"
         ) == 777);

    // Summary
    printf("\n" BOLD "=== Results ===" RESET "\n");
    printf("Passed: " GREEN "%d" RESET "\n", passed);
    printf("Failed: " RED "%d" RESET "\n", failed);
    printf("\n");

    return failed > 0 ? 1 : 0;
}
