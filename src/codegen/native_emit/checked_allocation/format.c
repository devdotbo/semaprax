/* The ordinary formatter worker retains its exact staging, rendering order,
 * partial prefix cleanup and CallCommit. Only its allocating borrowed helpers
 * are rebound in the exact checked translation unit, after the old prelude. */
static __attribute__((unused)) spx_status_token spx_checked_native_format_copy_v1(
    struct spx_context *ctx, const char *source, uint64_t length, char **out
) {
    return spx_checked_string_adapter_from_literal_v1(spx_checked_native_adapter_v1(ctx), source, length, out);
}
static __attribute__((unused)) spx_status_token spx_checked_native_format_join_v1(
    struct spx_context *ctx, const char *left, const char *right, char **out
) {
    /* This is borrowed concat: the worker owns and drops both inputs after
     * success, or its failure label settles them along with every staged field. */
    return spx_checked_string_adapter_concat_v1(spx_checked_native_adapter_v1(ctx), left, right, out);
}
static __attribute__((unused)) spx_status_token spx_checked_native_format_i64_v1(struct spx_context *ctx, int64_t value, char **out) {
    return spx_checked_string_adapter_from_i64_v1(spx_checked_native_adapter_v1(ctx), value, out);
}
static __attribute__((unused)) spx_status_token spx_checked_native_format_u64_v1(struct spx_context *ctx, uint64_t value, char **out) {
    return spx_checked_string_adapter_from_usize_v1(spx_checked_native_adapter_v1(ctx), value, out);
}
#define spx_format_copy_v1 spx_checked_native_format_copy_v1
#define spx_format_join_v1 spx_checked_native_format_join_v1
#define spx_format_i64_v1 spx_checked_native_format_i64_v1
#define spx_format_u64_v1 spx_checked_native_format_u64_v1
