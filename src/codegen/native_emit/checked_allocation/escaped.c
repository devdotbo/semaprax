/* Private checked source entry only. Host retain pins the exact allocator
 * provider AND this module; the caller pins executing code through close. No
 * decoded plan or handle can acquire compiler authority from these functions. */
static unsigned char spx_checked_string_result_module_v1;
struct spx_checked_string_result_handle_v1 { const struct spx_checked_string_result_lease_v1 *identity; uint64_t generation; };
struct spx_checked_string_result_borrow_v1 { const struct spx_checked_string_result_lease_v1 *identity; uint64_t generation, epoch; };
struct spx_checked_string_result_lease_v1 {
    const struct spx_checked_string_result_lease_v1 *identity;
    const void *module;
    void *retained;
    struct spx_context *context;
    uint64_t context_generation, context_nonce, generation, borrow_epoch;
    void *(*allocate)(void *, size_t);
    void (*release)(void *, void *);
    void (*release_state)(void *);
    char *text;
    spx_status_token selected;
    bool used, live, busy, invoked, borrowed;
};
static __attribute__((unused)) bool spx_checked_string_result_valid_v1(const struct spx_checked_string_result_lease_v1 *lease) {
    return lease != NULL && lease->used && lease->live && lease->identity == lease &&
        lease->module == &spx_checked_string_result_module_v1 && lease->retained != NULL &&
        lease->context != NULL && lease->context->state == SPX_CONTEXT_INITIALIZED &&
        lease->context->generation == lease->context_generation && lease->context->invocation_nonce == lease->context_nonce &&
        lease->allocate != NULL && lease->release != NULL && lease->release_state != NULL;
}
static __attribute__((unused)) bool spx_checked_string_result_init_v1(
    struct spx_checked_string_result_lease_v1 *lease, void *state,
    void *(*retain)(void *), struct spx_context *(*held_context)(void *), void (*release_state)(void *),
    void *(*allocate)(void *, size_t), void (*release)(void *, void *)
) {
    if (lease == NULL || lease->used || lease->live || lease->busy || lease->identity != NULL ||
        lease->retained != NULL || lease->context != NULL || lease->text != NULL ||
        retain == NULL || held_context == NULL || release_state == NULL || allocate == NULL || release == NULL) return false;
    /* Retain may call host code: retire the fresh initialization epoch first. */
    lease->used = true;
    void *retained = retain(state);
    if (retained == NULL) return false;
    struct spx_context *context = held_context(retained);
    if (context == NULL || context->state != SPX_CONTEXT_INITIALIZED || context->checked_allocation != NULL ||
        context->status_arena.entries == NULL || context->status_arena.length != UINT32_C(0) || context->status_arena.capacity == UINT32_C(0)) {
        release_state(retained); return false;
    }
    *lease = (struct spx_checked_string_result_lease_v1){ .identity = lease, .module = &spx_checked_string_result_module_v1,
        .retained = retained, .context = context, .context_generation = context->generation,
        .context_nonce = context->invocation_nonce, .generation = UINT64_C(1), .allocate = allocate,
        .release = release, .release_state = release_state, .used = true, .live = true };
    return true;
}
static __attribute__((unused)) bool spx_checked_string_result_borrow_v1(
    struct spx_checked_string_result_lease_v1 *lease, struct spx_checked_string_result_handle_v1 handle,
    struct spx_checked_string_result_borrow_v1 *borrow, const uint8_t **data, uint64_t *length
) {
    if (!spx_checked_string_result_valid_v1(lease) || lease->busy || lease->borrowed || lease->text == NULL ||
        handle.identity != lease || handle.generation != lease->generation || borrow == NULL || data == NULL || length == NULL ||
        lease->borrow_epoch == UINT64_MAX) return false;
    ++lease->borrow_epoch; lease->borrowed = true;
    *borrow = (struct spx_checked_string_result_borrow_v1){lease, lease->generation, lease->borrow_epoch};
    *data = (const uint8_t *)lease->text; *length = spx_string_length_v10(lease->text);
    return true;
}
static __attribute__((unused)) bool spx_checked_string_result_end_borrow_v1(
    struct spx_checked_string_result_lease_v1 *lease, struct spx_checked_string_result_borrow_v1 borrow
) {
    if (!spx_checked_string_result_valid_v1(lease) || lease->busy || !lease->borrowed ||
        borrow.identity != lease || borrow.generation != lease->generation || borrow.epoch != lease->borrow_epoch) return false;
    lease->borrowed = false; return true;
}
static __attribute__((unused)) bool spx_checked_string_result_drop_v1(
    struct spx_checked_string_result_lease_v1 *lease, struct spx_checked_string_result_handle_v1 handle
) {
    if (!spx_checked_string_result_valid_v1(lease) || lease->busy || lease->borrowed || lease->text == NULL ||
        handle.identity != lease || handle.generation != lease->generation) return false;
    struct spx_string_v10 *header = spx_string_header_v10(lease->text);
    if (header->release_state != lease->retained || header->release != lease->release) return false;
    char *text = lease->text; lease->text = NULL; ++lease->generation;
    lease->busy = true; spx_checked_native_drop_v1(text); lease->busy = false;
    return true;
}
static __attribute__((unused)) bool spx_checked_string_result_close_v1(struct spx_checked_string_result_lease_v1 *lease) {
    if (!spx_checked_string_result_valid_v1(lease) || lease->busy || lease->borrowed || lease->text != NULL) return false;
    void *retained = lease->retained; void (*release_state)(void *) = lease->release_state;
    *lease = (struct spx_checked_string_result_lease_v1){ .used = true };
    release_state(retained); return true;
}
