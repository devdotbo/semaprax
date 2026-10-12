/* Copy-record key: the authenticated scalar word under the existing Vec total
   order. Floating tags 6/7 are outside the v1 key profile. */
typedef struct { const uint64_t *words; uint64_t width; uint32_t field, tag; } spx_copy_unique_v1;
static __attribute__((unused)) int spx_copy_unique_compare_v1(const void *x, uint32_t a, uint32_t b) {
    const spx_copy_unique_v1 *u = (const spx_copy_unique_v1 *)x;
    uint64_t av = spx_copy_vec_key(u->tag, u->words[(uint64_t)a * u->width + u->field]);
    uint64_t bv = spx_copy_vec_key(u->tag, u->words[(uint64_t)b * u->width + u->field]);
    return (av > bv) - (av < bv);
}
static __attribute__((unused)) spx_status_token spx_copy_vec_unique_by(
    struct spx_context *c, const spx_vec_v1 *s, uint64_t width, uint32_t field, uint32_t tag, bool *r
) {
    (void)spx_copy_vec_check(c, s, width);
    if (field >= width || !tag || tag > 8 || tag == 6 || tag == 7)
        spx_runtime_invariant_failure("invalid Copy record field uniqueness key");
    spx_copy_unique_v1 u = { (const uint64_t *)(const void *)s->ptr, width, field, tag };
    return spx_unique_ordinals_v1(c, s->len / width, &u, spx_copy_unique_compare_v1, r);
}
