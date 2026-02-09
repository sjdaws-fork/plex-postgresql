/*
 * Test Cases for GROUP BY Query Rewriter
 * Tests the complete GROUP BY rewriter functionality
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <assert.h>

// Declare the functions we're testing
char* fix_group_by_strict_complete(const char *sql);
char* add_nulls_first_ordering(const char *sql);

// Test helper
static int test_query(const char *name, const char *input, const char *expected) {
    printf("TEST: %s\n", name);
    printf("  Input: %.100s...\n", input);

    char *result = fix_group_by_strict_complete(input);
    if (!result) {
        printf("  FAIL: Returned NULL\n\n");
        return 0;
    }

    int match = (strcasestr(result, expected) != NULL);
    if (match) {
        printf("  PASS: Contains expected: %s\n\n", expected);
    } else {
        printf("  FAIL: Expected substring not found\n");
        printf("  Expected: %s\n", expected);
        printf("  Got: %.200s\n\n", result);
    }

    free(result);
    return match;
}

int main() {
    int passed = 0;
    int total = 0;

    printf("=== GROUP BY Rewriter Test Suite ===\n\n");

    // Test 1: Simple case - add missing column
    total++;
    if (test_query(
        "Simple missing column",
        "SELECT metadata_items.id, metadata_items.title FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id,metadata_items.title"
    )) passed++;

    // Test 2: Multiple missing columns
    total++;
    if (test_query(
        "Multiple missing columns",
        "SELECT metadata_items.id, metadata_items.library_section_id, metadata_items.title FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id,metadata_items.library_section_id,metadata_items.title"
    )) passed++;

    // Test 3: With aggregate function
    total++;
    if (test_query(
        "With COUNT aggregate",
        "SELECT metadata_items.id, metadata_items.title, COUNT(*) as cnt FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id,metadata_items.title"
    )) passed++;

    // Test 4: GROUP BY with HAVING clause
    total++;
    if (test_query(
        "GROUP BY with HAVING",
        "SELECT metadata_items.id, metadata_items.title, COUNT(*) FROM metadata_items GROUP BY metadata_items.id HAVING COUNT(*) > 1",
        "GROUP BY metadata_items.id,metadata_items.title HAVING"
    )) passed++;

    // Test 5: GROUP BY with ORDER BY
    total++;
    if (test_query(
        "GROUP BY with ORDER BY",
        "SELECT metadata_items.id, metadata_items.title FROM metadata_items GROUP BY metadata_items.id ORDER BY metadata_items.title",
        "GROUP BY metadata_items.id,metadata_items.title ORDER BY"
    )) passed++;

    // Test 6: Quoted column names
    total++;
    if (test_query(
        "Quoted column names",
        "SELECT metadata_items.id, metadata_items.\"index\" FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id,metadata_items.\"index\""
    )) passed++;

    // Test 7: Multiple aggregate functions
    total++;
    if (test_query(
        "Multiple aggregates",
        "SELECT metadata_items.guid, COUNT(DISTINCT views.id) as cnt, group_concat(views.account_id) as ids FROM metadata_items GROUP BY metadata_items.guid",
        "GROUP BY metadata_items.guid"
    )) passed++;

    // Test 8: Real Plex query - metadata_items with views
    total++;
    if (test_query(
        "Real Plex metadata_items query",
        "select metadata_items.id, metadata_items.library_section_id, metadata_items.title, count(distinct metadata_item_views.id) as globalViewCount from metadata_item_views left join metadata_items on metadata_items.guid=metadata_item_views.guid where metadata_items.metadata_type=$1 group by metadata_items.guid order by globalViewCount desc limit 6",
        "GROUP BY metadata_items.guid,metadata_items.id,metadata_items.library_section_id,metadata_items.title"
    )) passed++;

    // Test 9: No GROUP BY - should return unchanged
    total++;
    const char *no_groupby = "SELECT * FROM metadata_items WHERE id = 1";
    char *result = fix_group_by_strict_complete(no_groupby);
    if (result && strcmp(result, no_groupby) == 0) {
        printf("TEST: No GROUP BY\n  PASS: Returned unchanged\n\n");
        passed++;
    } else {
        printf("TEST: No GROUP BY\n  FAIL: Should return unchanged\n\n");
    }
    free(result);

    // Test 10: Already complete GROUP BY - should return unchanged
    total++;
    const char *complete_groupby = "SELECT metadata_items.id, metadata_items.title FROM metadata_items GROUP BY metadata_items.id, metadata_items.title";
    result = fix_group_by_strict_complete(complete_groupby);
    if (result && strcasestr(result, "GROUP BY metadata_items.id") != NULL) {
        printf("TEST: Complete GROUP BY\n  PASS: Preserved GROUP BY\n\n");
        passed++;
    } else {
        printf("TEST: Complete GROUP BY\n  FAIL\n\n");
    }
    free(result);

    // Test 11: CASE expression - should not add to GROUP BY
    total++;
    if (test_query(
        "CASE expression",
        "SELECT metadata_items.id, CASE WHEN metadata_items.rating > 5 THEN 'high' ELSE 'low' END as rating_cat FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id"
    )) passed++;

    // Test 12: Aliased columns with AS
    total++;
    if (test_query(
        "Aliased columns",
        "SELECT metadata_items.id AS item_id, metadata_items.title AS item_title FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id,metadata_items.title"
    )) passed++;

    // Test 13: Table aliases in join
    total++;
    if (test_query(
        "Table aliases in join",
        "SELECT m.id, m.title, COUNT(*) FROM metadata_items m JOIN media_items mi ON mi.metadata_item_id = m.id GROUP BY m.id",
        "GROUP BY m.id,m.title"
    )) passed++;

    // Test 14: Real Plex query - media_items with parents
    total++;
    if (test_query(
        "Complex Plex query with parents",
        "select media_items.id, metadata_items.id, metadata_items.title, parents.title, count(distinct views.id) as cnt from metadata_items left join media_items on media_items.metadata_item_id=metadata_items.id left join metadata_items as parents on parents.id=metadata_items.parent_id left join metadata_item_views as views on views.guid=metadata_items.guid group by metadata_items.guid",
        "GROUP BY metadata_items.guid,media_items.id,metadata_items.id,metadata_items.title,parents.title"
    )) passed++;

    // Test 15: GROUP_CONCAT aggregate
    total++;
    if (test_query(
        "GROUP_CONCAT aggregate",
        "SELECT metadata_items.id, metadata_items.title, group_concat(tags.tag, ',') as tags FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id,metadata_items.title"
    )) passed++;

    // Test 16: Subquery in SELECT - should not add to GROUP BY
    total++;
    if (test_query(
        "Subquery in SELECT",
        "SELECT metadata_items.id, (SELECT COUNT(*) FROM media_items WHERE metadata_item_id = metadata_items.id) as media_count FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id"
    )) passed++;

    // ========================================================================
    // Edge cases for fix_group_by_strict_complete
    // ========================================================================

    // Test 17: NULL input
    total++;
    result = fix_group_by_strict_complete(NULL);
    if (result == NULL) {
        printf("TEST: NULL input\n  PASS: Returned NULL\n\n");
        passed++;
    } else {
        printf("TEST: NULL input\n  FAIL: Should return NULL\n\n");
        free(result);
    }

    // Test 18: GROUP BY with LIMIT
    total++;
    if (test_query(
        "GROUP BY with LIMIT",
        "SELECT metadata_items.id, metadata_items.title FROM metadata_items GROUP BY metadata_items.id LIMIT 10",
        "GROUP BY metadata_items.id,metadata_items.title"
    )) passed++;

    // Test 19: GROUP BY with LIMIT and OFFSET
    total++;
    if (test_query(
        "GROUP BY with LIMIT and OFFSET",
        "SELECT metadata_items.id, metadata_items.title FROM metadata_items GROUP BY metadata_items.id LIMIT 10 OFFSET 5",
        "GROUP BY metadata_items.id,metadata_items.title"
    )) passed++;

    // Test 20: GROUP BY preserves LIMIT clause
    total++;
    result = fix_group_by_strict_complete(
        "SELECT metadata_items.id, metadata_items.title FROM metadata_items GROUP BY metadata_items.id LIMIT 10"
    );
    if (result && strcasestr(result, "LIMIT 10") != NULL) {
        printf("TEST: LIMIT preserved after rewrite\n  PASS: LIMIT 10 present\n\n");
        passed++;
    } else {
        printf("TEST: LIMIT preserved after rewrite\n  FAIL: LIMIT 10 missing\n\n");
    }
    free(result);

    // Test 21: SUM aggregate
    total++;
    if (test_query(
        "SUM aggregate not added to GROUP BY",
        "SELECT metadata_items.id, SUM(metadata_items.rating) as total_rating FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id"
    )) passed++;

    // Test 22: AVG aggregate
    total++;
    if (test_query(
        "AVG aggregate not added to GROUP BY",
        "SELECT metadata_items.guid, metadata_items.title, AVG(views.rating) FROM metadata_items JOIN views ON views.guid = metadata_items.guid GROUP BY metadata_items.guid",
        "GROUP BY metadata_items.guid,metadata_items.title"
    )) passed++;

    // Test 23: string_agg aggregate
    total++;
    if (test_query(
        "string_agg aggregate not added to GROUP BY",
        "SELECT metadata_items.id, string_agg(tags.tag, ',') FROM metadata_items JOIN tags ON tags.metadata_item_id = metadata_items.id GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id"
    )) passed++;

    // Test 24: Multiple tables, multiple missing
    total++;
    if (test_query(
        "Multiple tables multiple missing columns",
        "SELECT a.id, a.name, b.title, COUNT(*) FROM table_a a JOIN table_b b ON b.a_id = a.id GROUP BY a.id",
        "GROUP BY a.id,a.name,b.title"
    )) passed++;

    // Test 25: SELECT with DISTINCT and GROUP BY
    total++;
    if (test_query(
        "DISTINCT with GROUP BY",
        "SELECT DISTINCT metadata_items.id, metadata_items.title, COUNT(*) FROM metadata_items GROUP BY metadata_items.id",
        "GROUP BY metadata_items.id,metadata_items.title"
    )) passed++;

    // ========================================================================
    // add_nulls_first_ordering tests
    // ========================================================================
    printf("\n=== NULLS FIRST Ordering Tests ===\n\n");

    // Test 26: NULL input
    total++;
    result = add_nulls_first_ordering(NULL);
    if (result == NULL) {
        printf("TEST: NULLS FIRST - NULL input\n  PASS: Returned NULL\n\n");
        passed++;
    } else {
        printf("TEST: NULLS FIRST - NULL input\n  FAIL: Should return NULL\n\n");
        free(result);
    }

    // Test 27: No GROUP BY -> return unchanged
    total++;
    {
        const char *no_gb = "SELECT * FROM metadata_items";
        result = add_nulls_first_ordering(no_gb);
        if (result && strcmp(result, no_gb) == 0) {
            printf("TEST: NULLS FIRST - no GROUP BY\n  PASS: Returned unchanged\n\n");
            passed++;
        } else {
            printf("TEST: NULLS FIRST - no GROUP BY\n  FAIL: Should return unchanged\n\n");
        }
        free(result);
    }

    // Test 28: GROUP BY without ORDER BY -> adds ORDER BY 1 NULLS FIRST
    total++;
    {
        result = add_nulls_first_ordering(
            "SELECT id, title FROM metadata_items GROUP BY id"
        );
        if (result && strcasestr(result, "ORDER BY 1 NULLS FIRST") != NULL) {
            printf("TEST: NULLS FIRST - adds ORDER BY\n  PASS: ORDER BY 1 NULLS FIRST present\n\n");
            passed++;
        } else {
            printf("TEST: NULLS FIRST - adds ORDER BY\n  FAIL: ORDER BY 1 NULLS FIRST missing\n  Got: %s\n\n",
                   result ? result : "NULL");
        }
        free(result);
    }

    // Test 29: GROUP BY with existing ORDER BY -> unchanged
    total++;
    {
        const char *with_order = "SELECT id FROM metadata_items GROUP BY id ORDER BY id ASC";
        result = add_nulls_first_ordering(with_order);
        if (result && strcmp(result, with_order) == 0) {
            printf("TEST: NULLS FIRST - existing ORDER BY unchanged\n  PASS: Returned unchanged\n\n");
            passed++;
        } else {
            printf("TEST: NULLS FIRST - existing ORDER BY unchanged\n  FAIL\n\n");
        }
        free(result);
    }

    // Test 30: GROUP BY with LIMIT -> ORDER BY inserted before LIMIT
    total++;
    {
        result = add_nulls_first_ordering(
            "SELECT id, title FROM metadata_items GROUP BY id LIMIT 10"
        );
        if (result && strcasestr(result, "ORDER BY 1 NULLS FIRST") != NULL &&
            strcasestr(result, "LIMIT 10") != NULL) {
            // Verify ORDER BY comes before LIMIT
            const char *order_pos = strcasestr(result, "ORDER BY");
            const char *limit_pos = strcasestr(result, "LIMIT");
            if (order_pos && limit_pos && order_pos < limit_pos) {
                printf("TEST: NULLS FIRST - ORDER BY before LIMIT\n  PASS\n\n");
                passed++;
            } else {
                printf("TEST: NULLS FIRST - ORDER BY before LIMIT\n  FAIL: wrong order\n\n");
            }
        } else {
            printf("TEST: NULLS FIRST - ORDER BY before LIMIT\n  FAIL\n  Got: %s\n\n",
                   result ? result : "NULL");
        }
        free(result);
    }

    // Test 31: GROUP BY with HAVING -> ORDER BY inserted after HAVING
    total++;
    {
        result = add_nulls_first_ordering(
            "SELECT id, COUNT(*) FROM metadata_items GROUP BY id HAVING COUNT(*) > 1"
        );
        if (result && strcasestr(result, "ORDER BY 1 NULLS FIRST") != NULL &&
            strcasestr(result, "HAVING") != NULL) {
            const char *having_pos = strcasestr(result, "HAVING");
            const char *order_pos = strcasestr(result, "ORDER BY");
            if (having_pos && order_pos && having_pos < order_pos) {
                printf("TEST: NULLS FIRST - ORDER BY after HAVING\n  PASS\n\n");
                passed++;
            } else {
                printf("TEST: NULLS FIRST - ORDER BY after HAVING\n  FAIL: wrong order\n\n");
            }
        } else {
            printf("TEST: NULLS FIRST - ORDER BY after HAVING\n  FAIL\n  Got: %s\n\n",
                   result ? result : "NULL");
        }
        free(result);
    }

    // Summary
    printf("=== Test Results ===\n");
    printf("Passed: %d/%d\n", passed, total);
    printf("Failed: %d/%d\n", total - passed, total);

    if (passed == total) {
        printf("\nAll tests PASSED!\n");
        return 0;
    } else {
        printf("\nSome tests FAILED\n");
        return 1;
    }
}
