/* Bytes retain the same immutable payload-release provenance as Strings.
 * Empty Bytes keep the established null/zero carrier and perform no request. */
struct spx_checked_bytes_header_v1 { void *release_state; void (*release)(void *, void *); uint64_t len; uint8_t data[]; };
static __attribute__((unused)) spx_status_token spx_checked_native_bytes_allocate_v1(
    struct spx_context *ctx, const uint8_t *source, uint64_t length, bool zeroed, spx_bytes_v1 *out
) {
    struct spx_checked_string_adapter_v1 *adapter = spx_checked_native_adapter_v1(ctx);
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL || length > SPX_OWNED_BYTES_MAX_BYTES || (!zeroed && length != UINT64_C(0) && source == NULL))
        spx_runtime_invariant_failure("checked Bytes current extent");
    if (length == UINT64_C(0)) { *out = (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) }; return SPX_STATUS_SUCCESS; }
    if (length > (uint64_t)SIZE_MAX - (uint64_t)offsetof(struct spx_checked_bytes_header_v1, data)) {
        spx_checked_string_fail_v1(&adapter->kernel, "semaprax.allocation.v1", UINT32_C(2));
        return spx_checked_string_adapter_failure_v1(adapter);
    }
    struct spx_checked_bytes_header_v1 *header = adapter->kernel.allocate(adapter->kernel.allocator_state,
        offsetof(struct spx_checked_bytes_header_v1, data) + (size_t)length);
    if (header == NULL) {
        spx_checked_string_fail_v1(&adapter->kernel, "semaprax.allocation.v1", UINT32_C(1));
        return spx_checked_string_adapter_failure_v1(adapter);
    }
    header->release_state = adapter->kernel.allocator_state; header->release = adapter->kernel.release; header->len = length;
    if (zeroed) memset(header->data, 0, (size_t)length); else memcpy(header->data, source, (size_t)length);
    *out = (spx_bytes_v1){ .ptr = header->data, .len = length };
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_checked_native_bytes_copy_v1(struct spx_context *ctx, spx_slice_u8_v1 source, spx_bytes_v1 *out) {
    spx_slice_u8_require_owned_view_valid(source);
    return spx_checked_native_bytes_allocate_v1(ctx, source.ptr, source.len, false, out);
}
static __attribute__((unused)) spx_status_token spx_checked_native_bytes_zeroed_v1(struct spx_context *ctx, uint64_t length, spx_bytes_v1 *out) {
    return spx_checked_native_bytes_allocate_v1(ctx, NULL, length, true, out);
}
static __attribute__((unused)) void spx_checked_native_bytes_drop_v1(spx_bytes_v1 *value) {
    if (value == NULL) spx_runtime_invariant_failure("checked Bytes null carrier");
    spx_bytes_require_valid(*value);
    if (value->ptr != NULL) {
        struct spx_checked_bytes_header_v1 *header = (struct spx_checked_bytes_header_v1 *)((uint8_t *)value->ptr - offsetof(struct spx_checked_bytes_header_v1, data));
        if (header->release == NULL || header->len != value->len) spx_runtime_invariant_failure("checked Bytes release extent");
        void *state = header->release_state; void (*release)(void *, void *) = header->release;
        value->ptr = NULL; value->len = UINT64_C(0); release(state, header);
    }
}
#define spx_bytes_drop spx_checked_native_bytes_drop_v1
