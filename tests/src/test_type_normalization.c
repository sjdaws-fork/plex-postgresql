/*
 * Unit tests for Type Normalization (normalize_sqlite_decltype)
 *
 * Tests db_interpose_column.c that normalizes Plex's custom
 * SQLite type annotations (like DT_INTEGER(8), BOOLEAN, BIGINT, VARCHAR)
 * to standard SQLite types for SOCI compatibility.
 *
 * The normalize_sqlite_decltype() function is static in db_interpose_column.c,
 * so we duplicate its logic here for testing purposes.
 *
 * Also tests decltype_hash() for cache correctness.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <ctype.h>

// Test framework
static int passed = 0;
static int failed = 0;

#define BOLD  "\033[1m"
#define GREEN "\033[32m"
#define RED   "\033[31m"
#define RESET "\033[0m"

#define LOG_DEBUG(...) do {} while(0)

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
// Copy of normalize_sqlite_decltype() from db_interpose_column.c
// Must be kept in sync with the real implementation
// ============================================================================

static const char* normalize_sqlite_decltype(const char *plex_type) {
    if (!plex_type || !plex_type[0]) {
        LOG_DEBUG("NORMALIZE_TYPE: NULL/empty input, returning TEXT");
        return "TEXT";
    }

    // Check for Plex's DT_INTEGER(n) format
    if (strncasecmp(plex_type, "DT_INTEGER", 10) == 0) {
        if ((plex_type[10] == '(' && plex_type[11] == '8' && plex_type[12] == ')')) {
            return "dt_integer(8)";
        }
        return "INTEGER";
    }

    // Check for INTEGER(n) format
    if (strncasecmp(plex_type, "INTEGER", 7) == 0 && 
        (plex_type[7] == '\0' || plex_type[7] == '(' || isspace(plex_type[7]))) {
        if (plex_type[7] == '(' && plex_type[8] == '8' && plex_type[9] == ')') {
            return "dt_integer(8)";
        }
        return "INTEGER";
    }

    // BIGINT -> dt_integer(8)
    if (strncasecmp(plex_type, "BIGINT", 6) == 0 && 
        (plex_type[6] == '\0' || plex_type[6] == '(' || isspace(plex_type[6]))) {
        return "dt_integer(8)";
    }

    // INT8 -> dt_integer(8)
    if (strcasecmp(plex_type, "INT8") == 0) return "dt_integer(8)";
    
    // INT64 -> dt_integer(8)
    if (strcasecmp(plex_type, "INT64") == 0) return "dt_integer(8)";
    
    // LONG -> dt_integer(8)
    if (strcasecmp(plex_type, "LONG") == 0) return "dt_integer(8)";
    
    // dt_integer(8) stays as-is
    if (strcasecmp(plex_type, "dt_integer(8)") == 0) return "dt_integer(8)";

    // Boolean -> INTEGER
    if (strcasecmp(plex_type, "boolean") == 0) return "INTEGER";

    // TIMESTAMP -> INTEGER (unix epoch)
    if (strcasecmp(plex_type, "TIMESTAMP") == 0) return "INTEGER";

    // Float types -> REAL
    if (strcasecmp(plex_type, "FLOAT") == 0) return "REAL";
    if (strcasecmp(plex_type, "DOUBLE") == 0) return "REAL";

    // String types -> TEXT
    if (strncasecmp(plex_type, "VARCHAR", 7) == 0 && 
        (plex_type[7] == '\0' || plex_type[7] == '(' || isspace(plex_type[7]))) {
        return "TEXT";
    }
    if (strcasecmp(plex_type, "STRING") == 0) return "TEXT";
    if (strcasecmp(plex_type, "CHAR") == 0) return "TEXT";

    // Standard SQLite types
    if (strcasecmp(plex_type, "REAL") == 0) return "REAL";
    if (strcasecmp(plex_type, "TEXT") == 0) return "TEXT";
    if (strcasecmp(plex_type, "BLOB") == 0) return "BLOB";
    if (strcasecmp(plex_type, "NUMERIC") == 0) return "NUMERIC";

    // Unknown -> TEXT
    return "TEXT";
}

// ============================================================================
// Copy of decltype_hash() from db_interpose_column.c
// ============================================================================

static unsigned int decltype_hash(const char *str) {
    unsigned int hash = 5381;
    int c;
    while ((c = *str++)) {
        hash = ((hash << 5) + hash) + (unsigned char)c;
    }
    return hash;
}

// ============================================================================
// Tests
// ============================================================================

int main(void) {
    printf(BOLD "\n=== Type Normalization Tests ===" RESET "\n\n");

    // ====================================================================
    // DT_INTEGER variants (Plex custom type)
    // ====================================================================
    printf(BOLD "DT_INTEGER Variants:" RESET "\n");

    test("DT_INTEGER(8) -> dt_integer(8)",
         strcmp(normalize_sqlite_decltype("DT_INTEGER(8)"), "dt_integer(8)") == 0);

    test("DT_INTEGER(4) -> INTEGER",
         strcmp(normalize_sqlite_decltype("DT_INTEGER(4)"), "INTEGER") == 0);

    test("DT_INTEGER(2) -> INTEGER",
         strcmp(normalize_sqlite_decltype("DT_INTEGER(2)"), "INTEGER") == 0);

    test("DT_INTEGER -> INTEGER (no parens)",
         strcmp(normalize_sqlite_decltype("DT_INTEGER"), "INTEGER") == 0);

    test("dt_integer(8) lowercase passthrough",
         strcmp(normalize_sqlite_decltype("dt_integer(8)"), "dt_integer(8)") == 0);

    // ====================================================================
    // INTEGER variants
    // ====================================================================
    printf("\n" BOLD "INTEGER Variants:" RESET "\n");

    test("INTEGER -> INTEGER",
         strcmp(normalize_sqlite_decltype("INTEGER"), "INTEGER") == 0);

    test("integer -> INTEGER (case insensitive)",
         strcmp(normalize_sqlite_decltype("integer"), "INTEGER") == 0);

    test("INTEGER(8) -> dt_integer(8)",
         strcmp(normalize_sqlite_decltype("INTEGER(8)"), "dt_integer(8)") == 0);

    test("INTEGER(4) -> INTEGER",
         strcmp(normalize_sqlite_decltype("INTEGER(4)"), "INTEGER") == 0);

    // ====================================================================
    // 64-bit integer type aliases
    // ====================================================================
    printf("\n" BOLD "64-bit Integer Aliases:" RESET "\n");

    test("BIGINT -> dt_integer(8)",
         strcmp(normalize_sqlite_decltype("BIGINT"), "dt_integer(8)") == 0);

    test("bigint -> dt_integer(8) (case insensitive)",
         strcmp(normalize_sqlite_decltype("bigint"), "dt_integer(8)") == 0);

    test("BIGINT(8) -> dt_integer(8)",
         strcmp(normalize_sqlite_decltype("BIGINT(8)"), "dt_integer(8)") == 0);

    test("INT8 -> dt_integer(8)",
         strcmp(normalize_sqlite_decltype("INT8"), "dt_integer(8)") == 0);

    test("INT64 -> dt_integer(8)",
         strcmp(normalize_sqlite_decltype("INT64"), "dt_integer(8)") == 0);

    test("LONG -> dt_integer(8)",
         strcmp(normalize_sqlite_decltype("LONG"), "dt_integer(8)") == 0);

    // ====================================================================
    // Boolean
    // ====================================================================
    printf("\n" BOLD "Boolean Normalization:" RESET "\n");

    test("boolean -> INTEGER",
         strcmp(normalize_sqlite_decltype("boolean"), "INTEGER") == 0);

    test("BOOLEAN -> INTEGER",
         strcmp(normalize_sqlite_decltype("BOOLEAN"), "INTEGER") == 0);

    // ====================================================================
    // Timestamp
    // ====================================================================
    printf("\n" BOLD "Timestamp:" RESET "\n");

    test("TIMESTAMP -> INTEGER (unix epoch)",
         strcmp(normalize_sqlite_decltype("TIMESTAMP"), "INTEGER") == 0);

    // ====================================================================
    // Float/Double -> REAL
    // ====================================================================
    printf("\n" BOLD "Float/Double:" RESET "\n");

    test("FLOAT -> REAL",
         strcmp(normalize_sqlite_decltype("FLOAT"), "REAL") == 0);

    test("DOUBLE -> REAL",
         strcmp(normalize_sqlite_decltype("DOUBLE"), "REAL") == 0);

    test("REAL -> REAL (passthrough)",
         strcmp(normalize_sqlite_decltype("REAL"), "REAL") == 0);

    // ====================================================================
    // String types -> TEXT
    // ====================================================================
    printf("\n" BOLD "String Types:" RESET "\n");

    test("VARCHAR -> TEXT",
         strcmp(normalize_sqlite_decltype("VARCHAR"), "TEXT") == 0);

    test("VARCHAR(255) -> TEXT",
         strcmp(normalize_sqlite_decltype("VARCHAR(255)"), "TEXT") == 0);

    test("VARCHAR(50) -> TEXT",
         strcmp(normalize_sqlite_decltype("VARCHAR(50)"), "TEXT") == 0);

    test("STRING -> TEXT",
         strcmp(normalize_sqlite_decltype("STRING"), "TEXT") == 0);

    test("CHAR -> TEXT",
         strcmp(normalize_sqlite_decltype("CHAR"), "TEXT") == 0);

    test("TEXT -> TEXT (passthrough)",
         strcmp(normalize_sqlite_decltype("TEXT"), "TEXT") == 0);

    test("text -> TEXT (case insensitive)",
         strcmp(normalize_sqlite_decltype("text"), "TEXT") == 0);

    // ====================================================================
    // Other standard types
    // ====================================================================
    printf("\n" BOLD "Standard Types:" RESET "\n");

    test("BLOB -> BLOB",
         strcmp(normalize_sqlite_decltype("BLOB"), "BLOB") == 0);

    test("NUMERIC -> NUMERIC",
         strcmp(normalize_sqlite_decltype("NUMERIC"), "NUMERIC") == 0);

    // ====================================================================
    // Edge cases
    // ====================================================================
    printf("\n" BOLD "Edge Cases:" RESET "\n");

    test("NULL -> TEXT (safety default)",
         strcmp(normalize_sqlite_decltype(NULL), "TEXT") == 0);

    test("Empty string -> TEXT",
         strcmp(normalize_sqlite_decltype(""), "TEXT") == 0);

    test("Unknown type -> TEXT (default)",
         strcmp(normalize_sqlite_decltype("CUSTOM_TYPE"), "TEXT") == 0);

    test("MONEY -> TEXT (unknown)",
         strcmp(normalize_sqlite_decltype("MONEY"), "TEXT") == 0);

    // ====================================================================
    // Boundary checks - avoid false prefix matches
    // ====================================================================
    printf("\n" BOLD "Boundary Checks:" RESET "\n");

    // "INTEGERS" should not match "INTEGER" prefix
    // The implementation checks for terminator, ( or space after "INTEGER"
    test("INTEGERS -> TEXT (not INTEGER)",
         strcmp(normalize_sqlite_decltype("INTEGERS"), "TEXT") == 0);

    // "VARCHARS" should not match
    test("VARCHARS -> TEXT (not VARCHAR)",
         strcmp(normalize_sqlite_decltype("VARCHARS"), "TEXT") == 0);

    // ====================================================================
    // decltype_hash tests
    // ====================================================================
    printf("\n" BOLD "decltype_hash():" RESET "\n");

    test("Hash - same string gives same hash",
         decltype_hash("metadata_items_id") == decltype_hash("metadata_items_id"));

    test("Hash - different strings give different hash",
         decltype_hash("metadata_items_id") != decltype_hash("metadata_items_title"));

    test("Hash - empty string returns consistent value",
         decltype_hash("") == decltype_hash(""));

    // Distribution check
    unsigned int h1 = decltype_hash("table_a") % 1024;
    unsigned int h2 = decltype_hash("table_b") % 1024;
    unsigned int h3 = decltype_hash("table_c") % 1024;
    test("Hash - distributes across buckets",
         !(h1 == h2 && h2 == h3));

    // Real Plex table/column combos
    test("Hash - metadata_items_title is deterministic",
         decltype_hash("metadata_items_title") == decltype_hash("metadata_items_title"));

    test("Hash - similar keys produce different hashes",
         decltype_hash("metadata_items_id") != decltype_hash("media_items_id"));

    // Summary
    printf("\n" BOLD "=== Results ===" RESET "\n");
    printf("Passed: " GREEN "%d" RESET "\n", passed);
    printf("Failed: " RED "%d" RESET "\n", failed);
    printf("\n");

    return failed > 0 ? 1 : 0;
}
