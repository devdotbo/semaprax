struct escaped_probe {
    struct spx_context context;
    struct spx_status_entry entries[16];
    size_t refuse, requests, live, releases, references, retain_calls, close_calls;
};
static void *escaped_retain(void *state) {
    struct escaped_probe *probe = state;
    ++probe->references; ++probe->retain_calls; return probe;
}
static struct spx_context *escaped_context(void *state) { return &((struct escaped_probe *)state)->context; }
static void escaped_close(void *state) {
    struct escaped_probe *probe = state;
    if (probe->references != 1 || probe->live != 0) abort();
    --probe->references; ++probe->close_calls;
}
static void *escaped_allocate(void *state, size_t bytes) {
    struct escaped_probe *probe = state;
    if (probe->references != 1) abort();
    ++probe->requests;
    if (probe->requests == probe->refuse) return NULL;
    void *pointer = malloc(bytes); if (pointer != NULL) ++probe->live; return pointer;
}
static void escaped_release(void *state, void *pointer) {
    struct escaped_probe *probe = state;
    if (probe->references != 1 || probe->live == 0 || pointer == NULL) abort();
    --probe->live; ++probe->releases; free(pointer);
}
int main(void) {
    for (size_t refused = 0; refused <= ESCAPED_REQUESTS; ++refused) {
        for (size_t repetition = 0; repetition < 3; ++repetition) {
            struct escaped_probe probe = { .refuse = refused };
            struct spx_checked_string_result_lease_v1 lease = {0};
            if (!spx_context_init(&probe.context, UINT64_C(1), probe.entries, UINT32_C(16), NULL, NULL, NULL)) return 1;
            if (!spx_checked_string_result_init_v1(&lease, &probe, escaped_retain, escaped_context, escaped_close, escaped_allocate, escaped_release)) return 2;
            if (probe.references != 1 || probe.retain_calls != 1) return 3;
            struct spx_checked_string_result_handle_v1 handle = {NULL, UINT64_C(919)};
            spx_status_token selected = spx_checked_string_result_invoke_v1(&lease, &handle);
            if (probe.context.checked_allocation != NULL || probe.references != 1 || lease.selected != selected) return 4;
            if (refused != 0) {
                if (selected == SPX_STATUS_SUCCESS || handle.identity != NULL || handle.generation != UINT64_C(919) ||
                    probe.requests != refused || probe.releases != refused - 1 || probe.live != 0) return 5;
                const struct spx_status_entry *entry = &probe.entries[selected - 1];
                if (strcmp(entry->status.domain_id, "semaprax.allocation.v1") != 0 || entry->status.code != UINT32_C(1)) return 6;
            } else {
                if (selected != SPX_STATUS_SUCCESS || probe.requests != ESCAPED_REQUESTS || probe.live != 1 || probe.releases != ESCAPED_REQUESTS - 1) return 7;
                if (spx_checked_string_result_close_v1(&lease)) return 8;
                struct spx_checked_string_result_lease_v1 copied = lease;
                struct spx_checked_string_result_handle_v1 wrong = {&copied, handle.generation};
                struct spx_checked_string_result_borrow_v1 borrow = {0};
                const uint8_t *data = NULL; uint64_t length = UINT64_C(919);
                if (spx_checked_string_result_borrow_v1(&copied, handle, &borrow, &data, &length) ||
                    spx_checked_string_result_drop_v1(&lease, wrong)) return 9;
                ++probe.context.invocation_nonce;
                if (spx_checked_string_result_borrow_v1(&lease, handle, &borrow, &data, &length) ||
                    spx_checked_string_result_drop_v1(&lease, handle)) return 10;
                --probe.context.invocation_nonce;
                if (!spx_checked_string_result_borrow_v1(&lease, handle, &borrow, &data, &length) ||
                    length != ESCAPED_LENGTH || memcmp(data, ESCAPED_TEXT, (size_t)length) != 0) return 11;
                if (spx_checked_string_result_drop_v1(&lease, handle) || spx_checked_string_result_close_v1(&lease)) return 12;
                struct spx_checked_string_result_borrow_v1 wrong_borrow = borrow; ++wrong_borrow.epoch;
                if (spx_checked_string_result_end_borrow_v1(&lease, wrong_borrow) ||
                    !spx_checked_string_result_end_borrow_v1(&lease, borrow) ||
                    spx_checked_string_result_end_borrow_v1(&lease, borrow)) return 13;
                if (!spx_checked_string_result_drop_v1(&lease, handle) ||
                    spx_checked_string_result_drop_v1(&lease, handle) ||
                    spx_checked_string_result_borrow_v1(&lease, handle, &borrow, &data, &length) || probe.live != 0 || probe.releases != ESCAPED_REQUESTS) return 14;
            }
            if (!spx_checked_string_result_close_v1(&lease) || probe.references != 0 || probe.close_calls != 1 ||
                spx_checked_string_result_close_v1(&lease) ||
                spx_checked_string_result_init_v1(&lease, &probe, escaped_retain, escaped_context, escaped_close, escaped_allocate, escaped_release) || probe.retain_calls != 1) return 15;
        }
    }
    puts("checked-native-allocation: ok");
    return 0;
}
