/*
 * shim_alloc.c — Lightweight shim-only memory tracker
 *
 * Tracks all malloc/free/realloc/calloc/strdup calls made by the shim.
 * Uses a small hash table to record allocation sizes so free() can
 * accurately subtract bytes. The hash table uses open addressing with
 * linear probing — simple, cache-friendly, and lock-free for reads.
 *
 * Overhead: ~1MB for the tracking table + one atomic add per alloc/free.
 * Logging: one summary line every 60s at INFO level.
 */

/* Must be before shim_alloc.h to prevent macro redefinition of malloc/free */
#define SHIM_ALLOC_NO_OVERRIDE
#include "shim_alloc.h"
#include "pg_logging.h"

#include <stdlib.h>
#include <string.h>
#include <stdatomic.h>
#include <time.h>
#include <pthread.h>

/* ---- Opt-in via PLEX_PG_ALLOC_TRACK=1 / PLEX_PG_ALLOC_TRACE=1 ---- */
static atomic_int g_enabled = -1;  /* -1 = not checked, 0 = off, 1 = track, 2 = track+trace */

static inline int shim_alloc_enabled(void) {
    int v = atomic_load(&g_enabled);
    if (__builtin_expect(v >= 0, 1)) return v;
    const char *env_track = getenv("PLEX_PG_ALLOC_TRACK");
    const char *env_trace = getenv("PLEX_PG_ALLOC_TRACE");
    if (env_trace && env_trace[0] == '1') {
        v = 2;  /* trace implies track */
    } else if (env_track && env_track[0] == '1') {
        v = 1;
    } else {
        v = 0;
    }
    atomic_store(&g_enabled, v);
    return v;
}

static inline int shim_alloc_trace_enabled(void) {
    return shim_alloc_enabled() >= 2;
}

/* ---- Atomic counters ---- */
static atomic_ullong g_total_allocs   = 0;
static atomic_ullong g_total_frees    = 0;
static atomic_ullong g_total_reallocs = 0;
static atomic_ullong g_bytes_alloc    = 0;
static atomic_ullong g_bytes_freed    = 0;
static atomic_llong  g_bytes_live     = 0;
static atomic_ullong g_peak_live      = 0;
static atomic_ullong g_last_log_ts    = 0;

/* ---- Size tracking hash table ----
 * We need to know the size of each allocation so free() can subtract it.
 * Using a simple open-addressing hash table with pointer keys.
 * 64K entries = 1MB, should be more than enough for shim allocations.
 */
#define ALLOC_TABLE_SIZE  (1 << 16)  /* 65536 entries */
#define ALLOC_TABLE_MASK  (ALLOC_TABLE_SIZE - 1)

typedef struct {
    _Atomic(void *)       ptr;    /* NULL = empty slot */
    _Atomic(size_t)       size;
    const char           *file;   /* source file (static string, no alloc) */
    int                   line;   /* source line */
} alloc_entry_t;

static alloc_entry_t g_alloc_table[ALLOC_TABLE_SIZE];

static inline unsigned int ptr_hash(const void *p) {
    unsigned long long v = (unsigned long long)p;
    v = (v >> 4) ^ (v >> 16) ^ (v >> 28);
    return (unsigned int)(v & ALLOC_TABLE_MASK);
}

/* Record an allocation. Returns 1 if stored, 0 if table full (size lost). */
static int alloc_table_put(void *ptr, size_t size, const char *file, int line) {
    if (!ptr) return 0;
    unsigned int idx = ptr_hash(ptr);
    for (int i = 0; i < 64; i++) {  /* max 64 probes */
        unsigned int slot = (idx + i) & ALLOC_TABLE_MASK;
        void *expected = NULL;
        if (atomic_compare_exchange_strong(&g_alloc_table[slot].ptr, &expected, ptr)) {
            atomic_store(&g_alloc_table[slot].size, size);
            g_alloc_table[slot].file = file;
            g_alloc_table[slot].line = line;
            return 1;
        }
        /* Slot taken by same ptr? (realloc scenario) */
        if (expected == ptr) {
            atomic_store(&g_alloc_table[slot].size, size);
            g_alloc_table[slot].file = file;
            g_alloc_table[slot].line = line;
            return 1;
        }
    }
    return 0;  /* table full in this neighborhood */
}

/* Remove and return the size of an allocation. Returns 0 if not found. */
static size_t alloc_table_remove(void *ptr) {
    if (!ptr) return 0;
    unsigned int idx = ptr_hash(ptr);
    for (int i = 0; i < 64; i++) {
        unsigned int slot = (idx + i) & ALLOC_TABLE_MASK;
        void *stored = atomic_load(&g_alloc_table[slot].ptr);
        if (stored == ptr) {
            size_t sz = atomic_load(&g_alloc_table[slot].size);
            atomic_store(&g_alloc_table[slot].ptr, (void *)NULL);
            atomic_store(&g_alloc_table[slot].size, (size_t)0);
            return sz;
        }
        if (stored == NULL) return 0;  /* empty = not in table */
    }
    return 0;
}

/* Update peak if current live exceeds it */
static void update_peak(void) {
    long long live = atomic_load(&g_bytes_live);
    if (live < 0) return;
    unsigned long long ulive = (unsigned long long)live;
    unsigned long long peak = atomic_load(&g_peak_live);
    while (ulive > peak) {
        if (atomic_compare_exchange_weak(&g_peak_live, &peak, ulive)) break;
    }
}

/* ---- Public API ---- */

void *shim_malloc_tracked(size_t size, const char *file, int line) {
    void *ptr = malloc(size);
    if (ptr && shim_alloc_enabled()) {
        atomic_fetch_add(&g_total_allocs, 1);
        atomic_fetch_add(&g_bytes_alloc, (unsigned long long)size);
        atomic_fetch_add(&g_bytes_live, (long long)size);
        alloc_table_put(ptr, size, file, line);
        update_peak();
    }
    return ptr;
}

void *shim_calloc_tracked(size_t count, size_t size, const char *file, int line) {
    void *ptr = calloc(count, size);
    size_t total = count * size;
    if (ptr && shim_alloc_enabled()) {
        atomic_fetch_add(&g_total_allocs, 1);
        atomic_fetch_add(&g_bytes_alloc, (unsigned long long)total);
        atomic_fetch_add(&g_bytes_live, (long long)total);
        alloc_table_put(ptr, total, file, line);
        update_peak();
    }
    return ptr;
}

void *shim_realloc_tracked(void *old_ptr, size_t new_size, const char *file, int line) {
    if (!shim_alloc_enabled()) return realloc(old_ptr, new_size);
    size_t old_size = alloc_table_remove(old_ptr);
    void *ptr = realloc(old_ptr, new_size);
    if (ptr) {
        atomic_fetch_add(&g_total_reallocs, 1);
        atomic_fetch_sub(&g_bytes_live, (long long)old_size);
        atomic_fetch_add(&g_bytes_live, (long long)new_size);
        atomic_fetch_add(&g_bytes_alloc, (unsigned long long)new_size);
        atomic_fetch_add(&g_bytes_freed, (unsigned long long)old_size);
        alloc_table_put(ptr, new_size, file, line);
        update_peak();
    }
    return ptr;
}

void shim_free_tracked(void *ptr, const char *file, int line) {
    (void)file; (void)line;
    if (!ptr) return;
    if (shim_alloc_enabled()) {
        size_t size = alloc_table_remove(ptr);
        atomic_fetch_add(&g_total_frees, 1);
        atomic_fetch_add(&g_bytes_freed, (unsigned long long)size);
        atomic_fetch_sub(&g_bytes_live, (long long)size);
    }
    free(ptr);
}

char *shim_strdup_tracked(const char *s, const char *file, int line) {
    if (!s) return NULL;
    size_t len = strlen(s) + 1;
    char *ptr = (char *)shim_malloc_tracked(len, file, line);
    if (ptr) memcpy(ptr, s, len);
    return ptr;
}

void shim_alloc_get_stats(shim_alloc_stats_t *out) {
    if (!out) return;
    out->total_allocs   = atomic_load(&g_total_allocs);
    out->total_frees    = atomic_load(&g_total_frees);
    out->total_reallocs = atomic_load(&g_total_reallocs);
    out->bytes_allocated = atomic_load(&g_bytes_alloc);
    out->bytes_freed    = atomic_load(&g_bytes_freed);
    out->bytes_live     = atomic_load(&g_bytes_live);
    out->peak_live      = atomic_load(&g_peak_live);
}

void shim_alloc_log_summary(void) {
    shim_alloc_stats_t s;
    shim_alloc_get_stats(&s);
    LOG_ERROR("SHIM_ALLOC: live=%lldKB peak=%lluKB allocs=%llu frees=%llu reallocs=%llu total_alloc=%lluKB total_freed=%lluKB",
             s.bytes_live / 1024, s.peak_live / 1024,
             s.total_allocs, s.total_frees, s.total_reallocs,
             s.bytes_allocated / 1024, s.bytes_freed / 1024);
}

void shim_alloc_dump_leaks(void) {
    /* Aggregate live allocations by file:line */
    typedef struct {
        const char *file;
        int         line;
        size_t      total_bytes;
        int         count;
    } leak_site_t;

    #define MAX_SITES 256
    leak_site_t sites[MAX_SITES];
    int num_sites = 0;

    for (int i = 0; i < ALLOC_TABLE_SIZE; i++) {
        void *ptr = atomic_load(&g_alloc_table[i].ptr);
        if (!ptr) continue;
        size_t sz = atomic_load(&g_alloc_table[i].size);
        const char *file = g_alloc_table[i].file;
        int line = g_alloc_table[i].line;
        if (!file) continue;

        /* Find or insert site */
        int found = -1;
        for (int j = 0; j < num_sites; j++) {
            if (sites[j].file == file && sites[j].line == line) {
                found = j;
                break;
            }
        }
        if (found >= 0) {
            sites[found].total_bytes += sz;
            sites[found].count++;
        } else if (num_sites < MAX_SITES) {
            sites[num_sites].file = file;
            sites[num_sites].line = line;
            sites[num_sites].total_bytes = sz;
            sites[num_sites].count = 1;
            num_sites++;
        }
    }

    if (num_sites == 0) return;

    /* Sort by total_bytes descending (simple insertion sort) */
    for (int i = 1; i < num_sites; i++) {
        leak_site_t tmp = sites[i];
        int j = i - 1;
        while (j >= 0 && sites[j].total_bytes < tmp.total_bytes) {
            sites[j + 1] = sites[j];
            j--;
        }
        sites[j + 1] = tmp;
    }

    /* Log top 15 sites */
    int top = num_sites < 15 ? num_sites : 15;
    LOG_ERROR("SHIM_ALLOC_TRACE: %d leak sites, top %d:", num_sites, top);
    for (int i = 0; i < top; i++) {
        /* Strip path to just filename */
        const char *name = sites[i].file;
        const char *slash = strrchr(name, '/');
        if (slash) name = slash + 1;
        LOG_ERROR("  #%d %s:%d — %zu bytes in %d allocs",
                  i + 1, name, sites[i].line,
                  sites[i].total_bytes, sites[i].count);
    }
    #undef MAX_SITES
}

void shim_alloc_maybe_log(void) {
    if (!shim_alloc_enabled()) return;
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    unsigned long long now = (unsigned long long)ts.tv_sec;
    unsigned long long prev = atomic_load(&g_last_log_ts);
    if (now - prev < 60) return;
    /* CAS: only one thread logs per interval */
    if (!atomic_compare_exchange_strong(&g_last_log_ts, &prev, now)) return;
    shim_alloc_log_summary();
    if (shim_alloc_trace_enabled()) {
        shim_alloc_dump_leaks();
    }
}

void shim_alloc_reset(void) {
    atomic_store(&g_total_allocs, 0);
    atomic_store(&g_total_frees, 0);
    atomic_store(&g_total_reallocs, 0);
    atomic_store(&g_bytes_alloc, 0);
    atomic_store(&g_bytes_freed, 0);
    atomic_store(&g_bytes_live, 0);
    atomic_store(&g_peak_live, 0);
    for (int i = 0; i < ALLOC_TABLE_SIZE; i++) {
        atomic_store(&g_alloc_table[i].ptr, (void *)NULL);
        atomic_store(&g_alloc_table[i].size, (size_t)0);
        g_alloc_table[i].file = NULL;
        g_alloc_table[i].line = 0;
    }
}
