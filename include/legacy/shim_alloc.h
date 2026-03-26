/*
 * shim_alloc.h — Lightweight shim-only memory tracker
 *
 * Include this header in shim .c files to track all malloc/free/realloc/calloc/strdup
 * allocations with zero code changes. Uses macro overrides that call tracking wrappers.
 *
 * Environment variables:
 *   PLEX_PG_ALLOC_TRACK=1  — Enable allocation counters + 60s summary logging
 *   PLEX_PG_ALLOC_TRACE=1  — Also log top leak sites (unfreed allocations by file:line)
 *
 * The tracker does NOT intercept Plex's own allocations — only code that
 * includes this header is tracked.
 */

#ifndef SHIM_ALLOC_H
#define SHIM_ALLOC_H

#include <stddef.h>

/* Query current stats (for tests and external reporting) */
typedef struct {
    unsigned long long total_allocs;      /* Number of malloc/calloc/strdup calls */
    unsigned long long total_frees;       /* Number of free() calls */
    unsigned long long total_reallocs;    /* Number of realloc() calls */
    unsigned long long bytes_allocated;   /* Cumulative bytes allocated */
    unsigned long long bytes_freed;       /* Cumulative bytes freed */
    long long          bytes_live;        /* Current live bytes (allocated - freed) */
    unsigned long long peak_live;         /* High-water mark of live bytes */
} shim_alloc_stats_t;

void  shim_alloc_get_stats(shim_alloc_stats_t *out);
void  shim_alloc_log_summary(void);       /* Force log now */
void  shim_alloc_maybe_log(void);         /* Log if 60s elapsed */
void  shim_alloc_dump_leaks(void);        /* Log top unfreed allocations by file:line */
void  shim_alloc_reset(void);             /* Reset all counters (for tests) */

/* Tracking wrappers — call these instead of libc directly */
void *shim_malloc_tracked(size_t size, const char *file, int line);
void *shim_calloc_tracked(size_t count, size_t size, const char *file, int line);
void *shim_realloc_tracked(void *ptr, size_t size, const char *file, int line);
void  shim_free_tracked(void *ptr, const char *file, int line);
char *shim_strdup_tracked(const char *s, const char *file, int line);

/*
 * Macro overrides — redirect malloc/free/etc. to tracked versions.
 * Only active in files that include this header.
 * Define SHIM_ALLOC_NO_OVERRIDE before including to disable.
 */
#ifndef SHIM_ALLOC_NO_OVERRIDE
#define malloc(size)         shim_malloc_tracked(size, __FILE__, __LINE__)
#define calloc(count, size)  shim_calloc_tracked(count, size, __FILE__, __LINE__)
#define realloc(ptr, size)   shim_realloc_tracked(ptr, size, __FILE__, __LINE__)
#define free(ptr)            shim_free_tracked(ptr, __FILE__, __LINE__)
#define strdup(s)            shim_strdup_tracked(s, __FILE__, __LINE__)
#endif

#endif /* SHIM_ALLOC_H */
