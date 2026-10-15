/* Borrowed Record-Field Uniqueness v1. One paid ordinal array is the only
   allocation; a deterministic in-place heapsort orders ordinals by the selected
   borrowed key and adjacent keys are compared for full equality. Rows, payload
   pointers, length, capacity, generation and authority are only read. */
typedef int (*spx_unique_compare_v1)(const void *context, uint32_t a, uint32_t b);
static __attribute__((unused)) void spx_unique_sift_v1(
    uint32_t *o, uint64_t root, uint64_t n, const void *x, spx_unique_compare_v1 compare
) {
    for (;;) {
        uint64_t child = 2 * root + 1;
        if (child >= n) return;
        if (child + 1 < n && compare(x, o[child], o[child + 1]) < 0) ++child;
        if (compare(x, o[root], o[child]) >= 0) return;
        uint32_t t = o[root]; o[root] = o[child]; o[child] = t; root = child;
    }
}
static __attribute__((unused)) spx_status_token spx_unique_ordinals_v1(
    struct spx_context *c, uint64_t n, const void *x, spx_unique_compare_v1 compare, bool *r
) {
    if (!r || !x || !compare || n > UINT64_C(8192))
        spx_runtime_invariant_failure("invalid field uniqueness extent");
    *r = false;
    if (n < 2) { *r = true; return SPX_STATUS_SUCCESS; }
    /* n <= 8192, so the checked extent cannot overflow size_t. */
    uint32_t *o = malloc((size_t)n * sizeof *o);
    if (!o) return spx_vec_failure(c, 3);
    for (uint64_t i = 0; i < n; ++i) o[i] = (uint32_t)i;
    for (uint64_t i = n / 2; i-- > 0;) spx_unique_sift_v1(o, i, n, x, compare);
    for (uint64_t end = n - 1; end > 0; --end) {
        uint32_t t = o[0]; o[0] = o[end]; o[end] = t;
        spx_unique_sift_v1(o, 0, end, x, compare);
    }
    bool unique = true;
    for (uint64_t i = 1; i < n && unique; ++i) unique = compare(x, o[i - 1], o[i]) != 0;
    free(o);
    *r = unique;
    return SPX_STATUS_SUCCESS;
}
