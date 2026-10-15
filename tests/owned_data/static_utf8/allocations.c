#include <stdlib.h>
static unsigned spx_probe_allocations = 0u;
static unsigned spx_probe_frees = 0u;
void *spx_probe_malloc(size_t size) { spx_probe_allocations += 1u; return malloc(size); }
void *spx_probe_calloc(size_t count, size_t size) { spx_probe_allocations += 1u; return calloc(count, size); }
void *spx_probe_realloc(void *value, size_t size) { spx_probe_allocations += 1u; return realloc(value, size); }
void spx_probe_free(void *value) { spx_probe_frees += 1u; free(value); }
#define malloc spx_probe_malloc
#define calloc spx_probe_calloc
#define realloc spx_probe_realloc
#define free spx_probe_free
