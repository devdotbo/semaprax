/* Owned-leaf key: borrowed scalar bits or String bytes compared unsigned with
   explicit lengths, so NUL and UTF-8 prefixes are ordinary data. */
typedef struct { const unsigned char *rows; uint32_t stride, offset, kind; } spx_leaf_unique_v1;
static __attribute__((unused)) int spx_leaf_unique_compare_v1(const void *x, uint32_t a, uint32_t b) {
    const spx_leaf_unique_v1 *u = (const spx_leaf_unique_v1 *)x;
    const unsigned char *p = u->rows + (uint64_t)a * u->stride + u->offset;
    const unsigned char *q = u->rows + (uint64_t)b * u->stride + u->offset;
    if (u->kind < 9) {
        uint64_t av, bv; memcpy(&av, p, 8); memcpy(&bv, q, 8);
        av = spx_leaf_scalar_key(u->kind, av); bv = spx_leaf_scalar_key(u->kind, bv);
        return (av > bv) - (av < bv);
    }
    char *as, *bs; memcpy(&as, p, sizeof(as)); memcpy(&bs, q, sizeof(bs));
    uint64_t an = spx_string_length_v10(as), bn = spx_string_length_v10(bs);
    uint64_t common = an < bn ? an : bn;
    int order = common ? memcmp(as, bs, (size_t)common) : 0;
    if (order) return order > 0 ? 1 : -1;
    return (an > bn) - (an < bn);
}
static __attribute__((unused)) spx_status_token spx_leaf_unique_by(
    struct spx_context *c, const spx_vec_v1 *s, const spx_leaf_layout_v1 *d, uint32_t field, bool *r
) {
    (void)spx_leaf_check(c, s, d);
    if (field >= d->count || !d->kinds[field] || d->kinds[field] > 9
        || d->kinds[field] == 6 || d->kinds[field] == 7)
        spx_runtime_invariant_failure("invalid owned leaf field uniqueness key");
    spx_leaf_unique_v1 u = { (const unsigned char *)(const void *)s->ptr, d->stride, d->offsets[field], d->kinds[field] };
    return spx_unique_ordinals_v1(c, s->len, &u, spx_leaf_unique_compare_v1, r);
}
