/* Test-owned physical census and fail-stop observer, outside selected policy.
   Included before the runtime (and before emitted programs) so every
   calloc/malloc/free in the measured translation unit is observed in order.
   Emitted-program drivers define PROBE_KEEP_FAIL_STOP to keep the emitted
   invariant-failure boundary instead of the runtime probe's longjmp. */
#include <assert.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define PROBE_EVENTS 64u
#define PROBE_FN static __attribute__((unused))
typedef struct {
    size_t calls, frees, live, fail_at, mallocs, events;
    size_t bytes[PROBE_EVENTS];
    char order[PROBE_EVENTS + 1];
} probe_census;
static probe_census census;
static jmp_buf *stop_target;
static const char *stop_message;
PROBE_FN void census_reset(void) { memset(&census, 0, sizeof(census)); stop_message = NULL; }
PROBE_FN void census_note(char kind) {
    if (census.events < PROBE_EVENTS) census.order[census.events] = kind;
    census.events++;
}
PROBE_FN void *probe_calloc(size_t n, size_t size) {
    census.calls++;
    census_note('c');
    if (census.calls == census.fail_at) return NULL;
    void *p = calloc(n, size);
    if (p) {
        census.live++;
        if (census.calls <= PROBE_EVENTS) census.bytes[census.calls - 1] = n * size;
    }
    return p;
}
PROBE_FN void *probe_malloc(size_t size) {
    census_note('m');
    void *p = malloc(size);
    if (p) { census.live++; census.mallocs++; }
    return p;
}
PROBE_FN void probe_free(void *p) {
    if (!p) return;
    assert(census.live);
    census.live--; census.frees++; free(p);
}
PROBE_FN _Noreturn void probe_stop(const char *message) {
    stop_message = message;
    if (stop_target) longjmp(*stop_target, 1);
    abort();
}
#define calloc probe_calloc
#define malloc probe_malloc
#define free probe_free
#ifndef PROBE_KEEP_FAIL_STOP
#define SPX_FUSED_FAIL_STOP(message) probe_stop(message)
#endif
