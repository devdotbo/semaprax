/* Test-owned physical census and fail-stop observer, outside selected policy.
   Included before the runtime so every payload, metadata and provider
   allocation in the translation unit is observed separately and in order. */
#include <assert.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define PROBE_FN static __attribute__((unused))
typedef struct { const uint8_t *ptr; uint64_t len; } spx_slice_u8_v1;
typedef struct { uint8_t *ptr; uint64_t len; } spx_bytes_v1;
typedef struct {
    size_t calls, live, frees, fail_at;
    uint64_t bytes;
} probe_pool;
static probe_pool payload_pool, metadata_pool, provider_pool;
static jmp_buf *stop_target;
static const char *stop_message;
PROBE_FN void census_reset(void) {
    memset(&payload_pool, 0, sizeof(payload_pool));
    memset(&metadata_pool, 0, sizeof(metadata_pool));
    memset(&provider_pool, 0, sizeof(provider_pool));
    stop_message = NULL;
}
PROBE_FN void *probe_alloc(probe_pool *pool, size_t n, size_t size, bool zero) {
    pool->calls++;
    if (pool->calls == pool->fail_at) return NULL;
    void *p = zero ? calloc(n, size) : malloc(n * size);
    if (p != NULL) { pool->live++; pool->bytes += (uint64_t)(n * size); }
    return p;
}
PROBE_FN void probe_release(probe_pool *pool, void *p) {
    assert(p != NULL && pool->live != 0u);
    pool->live--; pool->frees++; free(p);
}
PROBE_FN void *probe_payload_calloc(size_t n, size_t size) { return probe_alloc(&payload_pool, n, size, true); }
PROBE_FN void probe_payload_free(void *p) { probe_release(&payload_pool, p); }
PROBE_FN void *probe_metadata_calloc(size_t n, size_t size) { return probe_alloc(&metadata_pool, n, size, true); }
PROBE_FN void probe_metadata_free(void *p) { probe_release(&metadata_pool, p); }
PROBE_FN void *probe_provider_malloc(size_t size) { return probe_alloc(&provider_pool, 1u, size, false); }
PROBE_FN void probe_provider_free(void *p) { probe_release(&provider_pool, p); }
_Noreturn PROBE_FN void probe_stop(const char *message) {
    stop_message = message;
    if (stop_target == NULL) { fprintf(stderr, "unexpected fail-stop: %s\n", message); abort(); }
    longjmp(*stop_target, 1);
}
/* Fixtures that cross a longjmp live in static storage, never in registers. */
#define EXPECT_STOP(fragment, statement) do { \
    jmp_buf probe_here; jmp_buf *probe_prior = stop_target; stop_message = NULL; \
    if (setjmp(probe_here) == 0) { \
        stop_target = &probe_here; statement; stop_target = probe_prior; \
        fprintf(stderr, "expected fail-stop: %s\n", fragment); abort(); \
    } \
    stop_target = probe_prior; \
    if (stop_message == NULL || strstr(stop_message, fragment) == NULL) { \
        fprintf(stderr, "wrong fail-stop: wanted %s, got %s\n", fragment, \
            stop_message == NULL ? "(none)" : stop_message); abort(); \
    } \
} while (0)
#define SPX_SHB_FAIL_STOP(message) probe_stop(message)
#define SPX_SHB_PAYLOAD_CALLOC probe_payload_calloc
#define SPX_SHB_PAYLOAD_FREE probe_payload_free
#define SPX_SHB_METADATA_CALLOC probe_metadata_calloc
#define SPX_SHB_METADATA_FREE probe_metadata_free
#define SPX_SHB_PROVIDER_MALLOC probe_provider_malloc
#define SPX_SHB_PROVIDER_FREE probe_provider_free
