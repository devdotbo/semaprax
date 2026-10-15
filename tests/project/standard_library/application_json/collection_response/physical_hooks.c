/* OPT-737 full-response row sweep hooks. Include after the shared allocation/
 * copy observers and before the exact generated provider; only generated code
 * is wrapped. Preparation, oracle assembly and reporting use unwrapped libc.
 * Nothing resets an ordinary runtime cap or changes a production ABI. */
static bool phase_on;
static size_t phase_reservations, phase_reserved_bytes, phase_strings, phase_string_bytes;
static size_t phase_final_strings, phase_final_length, phase_peak;
#undef calloc
static void *phase_calloc(size_t count, size_t size) {
    void *result = fixture_calloc(count, size);
    if (phase_on) {
        ++phase_reservations;
        phase_reserved_bytes += count * size;
        /* Copies whose source lies in this exact buffer are final copies. */
        fixture_input_start = result;
        fixture_input_length = count * size;
        if (fixture_live > phase_peak) phase_peak = fixture_live;
    }
    return result;
}
#undef malloc
static void *phase_malloc(size_t size) {
    void *result = fixture_bulk_malloc(size);
    if (phase_on) {
        ++phase_strings;
        phase_string_bytes += size;
        if (size == phase_final_length) ++phase_final_strings;
        if (fixture_live > phase_peak) phase_peak = fixture_live;
    }
    return result;
}
/* Growth would be unobserved by the inventory above; refuse it outright. */
static __attribute__((unused)) void *phase_realloc(void *pointer, size_t size) {
    (void)pointer;
    (void)size;
    fixture_require(false, "generated realloc is outside this witness", __LINE__);
    return NULL;
}
#define calloc phase_calloc
#define malloc phase_malloc
#define realloc phase_realloc
#define SPX_VEC_REALLOC phase_realloc
/* The caller supplies the ordinary String header+payload+NUL request size. */
static void phase_begin(size_t final_string_request) {
    phase_reservations = phase_reserved_bytes = phase_strings = phase_string_bytes = 0;
    phase_final_strings = 0;
    phase_final_length = final_string_request;
    phase_peak = fixture_live;
    fixture_watch(NULL, 0);
    phase_on = true;
}
