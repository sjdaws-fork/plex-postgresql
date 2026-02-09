/*
 * Unit Tests for db_interpose_bind.c helper functions
 * Tests: contains_binary_bytes(), bytes_to_pg_hex()
 *
 * These are pure functions that detect binary data and convert to PostgreSQL hex format.
 * Critical for BLOB data integrity - wrong detection = data corruption.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ============================================================================
// Copy of contains_binary_bytes and bytes_to_pg_hex from db_interpose_bind.c
// Copied here to avoid pulling in libpq/sqlite3 dependencies
// ============================================================================

int contains_binary_bytes(const unsigned char *data, size_t len) {
    if (!data || len == 0) return 0;

    for (size_t i = 0; i < len; i++) {
        unsigned char c = data[i];
        if (c < 0x20 && c != 0x09 && c != 0x0A && c != 0x0D) {
            return 1;
        }
        if (c == 0x7F || c == 0xC0 || c == 0xC1 || c >= 0xF5) {
            return 1;
        }
        if (i == 0 && len >= 2 && c == 0x1f && data[1] == 0x8b) {
            return 1;
        }
    }
    return 0;
}

char* bytes_to_pg_hex(const unsigned char *data, size_t len) {
    if (!data || len == 0) return strdup("");

    size_t hex_len = 2 + (len * 2) + 1;
    char *hex = malloc(hex_len);
    if (!hex) return NULL;

    hex[0] = '\\';
    hex[1] = 'x';

    static const char hex_chars[] = "0123456789abcdef";
    for (size_t i = 0; i < len; i++) {
        hex[2 + i*2] = hex_chars[(data[i] >> 4) & 0x0F];
        hex[2 + i*2 + 1] = hex_chars[data[i] & 0x0F];
    }
    hex[hex_len - 1] = '\0';

    return hex;
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
    printf(BOLD "=== Bind Helper Function Tests ===" RESET "\n\n");

    // ========================================================================
    // contains_binary_bytes()
    // ========================================================================
    printf(BOLD "contains_binary_bytes():" RESET "\n");

    test("NULL data -> 0",
         contains_binary_bytes(NULL, 0) == 0);

    test("Empty data -> 0",
         contains_binary_bytes((const unsigned char *)"", 0) == 0);

    test("ASCII text -> 0",
         contains_binary_bytes((const unsigned char *)"Hello, World!", 13) == 0);

    test("UTF-8 text -> 0",
         contains_binary_bytes((const unsigned char *)"Héllo Wörld", 13) == 0);

    test("Text with tab -> 0",
         contains_binary_bytes((const unsigned char *)"col1\tcol2", 9) == 0);

    test("Text with newline -> 0",
         contains_binary_bytes((const unsigned char *)"line1\nline2", 11) == 0);

    test("Text with carriage return -> 0",
         contains_binary_bytes((const unsigned char *)"line1\r\nline2", 12) == 0);

    // Binary data
    test("NUL byte -> 1",
         contains_binary_bytes((const unsigned char *)"\x00", 1) == 1);

    test("Control char 0x01 -> 1",
         contains_binary_bytes((const unsigned char *)"\x01", 1) == 1);

    test("Bell 0x07 -> 1",
         contains_binary_bytes((const unsigned char *)"\x07test", 5) == 1);

    test("DEL 0x7F -> 1",
         contains_binary_bytes((const unsigned char *)"\x7F", 1) == 1);

    test("Invalid UTF-8 0xC0 -> 1",
         contains_binary_bytes((const unsigned char *)"\xC0", 1) == 1);

    test("Invalid UTF-8 0xC1 -> 1",
         contains_binary_bytes((const unsigned char *)"\xC1", 1) == 1);

    test("Invalid UTF-8 0xF5+ -> 1",
         contains_binary_bytes((const unsigned char *)"\xF5", 1) == 1);

    // Gzip magic bytes
    unsigned char gzip[] = {0x1f, 0x8b, 0x08, 0x00};
    test("Gzip magic bytes -> 1",
         contains_binary_bytes(gzip, 4) == 1);

    // Mixed content
    unsigned char mixed[] = {'H', 'e', 'l', 'l', 'o', 0x01, 'W'};
    test("Text with embedded control char -> 1",
         contains_binary_bytes(mixed, 7) == 1);

    // Binary at specific positions
    unsigned char late_binary[] = {'A', 'B', 'C', 'D', 'E', 0x02};
    test("Binary byte at end of data -> 1",
         contains_binary_bytes(late_binary, 6) == 1);

    // ========================================================================
    // bytes_to_pg_hex()
    // ========================================================================
    printf("\n" BOLD "bytes_to_pg_hex():" RESET "\n");

    // NULL/empty
    char *result;

    result = bytes_to_pg_hex(NULL, 0);
    test("NULL data -> empty string",
         result != NULL && strcmp(result, "") == 0);
    free(result);

    result = bytes_to_pg_hex((const unsigned char *)"", 0);
    test("Empty data -> empty string",
         result != NULL && strcmp(result, "") == 0);
    free(result);

    // Single byte
    unsigned char one_byte[] = {0xAB};
    result = bytes_to_pg_hex(one_byte, 1);
    test("Single byte 0xAB -> \\xab",
         result != NULL && strcmp(result, "\\xab") == 0);
    free(result);

    // Multiple bytes
    unsigned char multi[] = {0xDE, 0xAD, 0xBE, 0xEF};
    result = bytes_to_pg_hex(multi, 4);
    test("0xDEADBEEF -> \\xdeadbeef",
         result != NULL && strcmp(result, "\\xdeadbeef") == 0);
    free(result);

    // All zeros
    unsigned char zeros[] = {0x00, 0x00, 0x00};
    result = bytes_to_pg_hex(zeros, 3);
    test("Three zero bytes -> \\x000000",
         result != NULL && strcmp(result, "\\x000000") == 0);
    free(result);

    // All 0xFF
    unsigned char ffs[] = {0xFF, 0xFF};
    result = bytes_to_pg_hex(ffs, 2);
    test("Two 0xFF bytes -> \\xffff",
         result != NULL && strcmp(result, "\\xffff") == 0);
    free(result);

    // Printable ASCII as hex
    unsigned char ascii[] = {'A', 'B'};  // 0x41, 0x42
    result = bytes_to_pg_hex(ascii, 2);
    test("ASCII 'AB' -> \\x4142",
         result != NULL && strcmp(result, "\\x4142") == 0);
    free(result);

    // Longer data (simulate small blob)
    unsigned char blob[] = {0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A};  // PNG header
    result = bytes_to_pg_hex(blob, 8);
    test("PNG header -> \\x89504e470d0a1a0a",
         result != NULL && strcmp(result, "\\x89504e470d0a1a0a") == 0);
    free(result);

    // Verify round-trip concept: hex always starts with \x
    unsigned char any[] = {0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF};
    result = bytes_to_pg_hex(any, 8);
    test("Hex output starts with \\x prefix",
         result != NULL && result[0] == '\\' && result[1] == 'x');
    test("Hex output length = 2 + 2*input_len",
         result != NULL && strlen(result) == 2 + 2 * 8);
    free(result);

    // Summary
    printf("\n" BOLD "=== Results ===" RESET "\n");
    printf("Passed: " GREEN "%d" RESET "\n", passed);
    printf("Failed: " RED "%d" RESET "\n", failed);
    printf("\n");

    return failed > 0 ? 1 : 0;
}
