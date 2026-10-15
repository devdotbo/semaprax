/* The caller lends one exact invocation allocator. Each published String
 * retains the immutable release pair; cleanup never looks up a later context. */
static __attribute__((unused)) struct spx_checked_string_adapter_v1 *spx_checked_native_adapter_v1(struct spx_context *ctx) {
    if (ctx == NULL || ctx->checked_allocation == NULL ||
        ctx->checked_allocation->semantic != ctx || ctx->state != SPX_CONTEXT_INITIALIZED)
        spx_runtime_invariant_failure("checked String invocation is not bound");
    return ctx->checked_allocation;
}
static __attribute__((unused)) void spx_checked_native_drop_v1(char *text) {
    struct spx_string_v10 *header = spx_string_header_v10(text);
    if (header->release == NULL) spx_runtime_invariant_failure("checked String release is not retained");
    header->release(header->release_state, header);
}
#define spx_string_drop spx_checked_native_drop_v1
static __attribute__((unused)) bool spx_checked_native_physical_v1(struct spx_context *ctx, spx_status_token token) {
    struct spx_checked_string_adapter_v1 *adapter = spx_checked_native_adapter_v1(ctx);
    if (token == SPX_STATUS_SUCCESS || token != adapter->selected || adapter->kernel.selected.domain_id == NULL)
        spx_runtime_invariant_failure("checked allocation status is not selected by this invocation");
    return strcmp(adapter->kernel.selected.domain_id, "semaprax.allocation.v1") == 0;
}
static __attribute__((unused)) spx_status_token spx_checked_native_concat_v1(
    struct spx_context *ctx, char *left, char *right, char **out
) {
    /* These two epochs committed together before the call. The callee owns
     * both on success and failure; canonical caller cleanup cannot reclaim them. */
    spx_status_token selected = spx_checked_string_adapter_concat_v1(spx_checked_native_adapter_v1(ctx), left, right, out);
    spx_checked_native_drop_v1(left);
    spx_checked_native_drop_v1(right);
    return selected;
}
static __attribute__((unused)) spx_status_token spx_checked_native_text_failure_v1(struct spx_context *ctx, uint32_t code) {
    struct spx_checked_string_adapter_v1 *adapter = spx_checked_native_adapter_v1(ctx);
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    adapter->kernel.selected = (struct spx_checked_string_status_v1){SPX_STATUS_SCHEMA_V1, "semaprax.text.v1", code, "adapter", false};
    if (!spx_status_record_adapter(ctx, "semaprax.text.v1", code, SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE, &adapter->selected)) spx_runtime_invariant_failure("checked text status capacity");
    return adapter->selected;
}
static __attribute__((unused)) bool spx_checked_native_boundary_v1(const char *value, uint64_t length, uint64_t at) {
    return at == UINT64_C(0) || at == length || (((const uint8_t *)value)[at] & UINT8_C(0xc0)) != UINT8_C(0x80);
}
static __attribute__((unused)) spx_status_token spx_checked_native_slice_v1(
    struct spx_context *ctx, const char *value, int64_t start, int64_t end, char **out
) {
    struct spx_checked_string_adapter_v1 *adapter = spx_checked_native_adapter_v1(ctx);
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    uint64_t length = spx_string_length_v10(value);
    if (start < INT64_C(0) || start > end || (uint64_t)end > length)
        return spx_checked_native_text_failure_v1(ctx, UINT32_C(1));
    if (!spx_checked_native_boundary_v1(value, length, (uint64_t)start) ||
        !spx_checked_native_boundary_v1(value, length, (uint64_t)end))
        return spx_checked_native_text_failure_v1(ctx, UINT32_C(2));
    return spx_checked_string_adapter_from_literal_v1(adapter, value + start, (uint64_t)(end - start), out);
}
static __attribute__((unused)) bool spx_checked_native_space_v1(uint8_t byte) {
    return byte == UINT8_C(0x20) || (byte >= UINT8_C(0x09) && byte <= UINT8_C(0x0d));
}
static __attribute__((unused)) spx_status_token spx_checked_native_trim_v1(struct spx_context *ctx, const char *value, char **out) {
    struct spx_checked_string_adapter_v1 *adapter = spx_checked_native_adapter_v1(ctx);
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    uint64_t start = UINT64_C(0), end = spx_string_length_v10(value);
    while (start < end && spx_checked_native_space_v1(((const uint8_t *)value)[start])) ++start;
    while (end > start && spx_checked_native_space_v1(((const uint8_t *)value)[end - UINT64_C(1)])) --end;
    return spx_checked_string_adapter_from_literal_v1(adapter, value + start, end - start, out);
}
