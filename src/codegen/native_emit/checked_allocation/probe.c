/* Every request in the real generated helper closure is observed. The old
 * kernel-only probes remain independent controls; this invokes checked source. */
struct checked_probe { size_t ordinal, refuse, live, releases, requested; };
static void *checked_allocate(void *state, size_t bytes) {
    struct checked_probe *probe = state;
    ++probe->ordinal;
    probe->requested += bytes;
    if (probe->ordinal == probe->refuse) return NULL;
    void *pointer = malloc(bytes);
    if (pointer != NULL) ++probe->live;
    return pointer;
}
static void checked_release(void *state, void *pointer) {
    struct checked_probe *probe = state;
    if (pointer == NULL || probe->live == 0) abort();
    --probe->live;
    ++probe->releases;
    free(pointer);
}
int main(void) {
    for (size_t refused = 0; refused <= 3; ++refused) {
        for (size_t repetition = 0; repetition < 3; ++repetition) {
            struct checked_probe probe = { .refuse = refused };
            struct spx_context context = {0};
            struct spx_status_entry entries[16];
            if (!spx_context_init(&context, UINT64_C(1), entries, UINT32_C(16), NULL, NULL, NULL)) return 1;
            int64_t out = INT64_C(919);
            spx_status_token selected = spx_checked_allocation_invoke_v1(&context, &probe, checked_allocate, checked_release, &out);
            if (context.checked_allocation != NULL || probe.live != 0) return 2;
            if (refused == 0) {
                if (selected != SPX_STATUS_SUCCESS || out != INT64_C(3) || probe.ordinal != 3 || probe.releases != 3) return 3;
            } else {
                if (selected == SPX_STATUS_SUCCESS || out != INT64_C(919) || probe.ordinal != refused || probe.releases != refused - 1) return 4;
                const struct spx_status_entry *entry = &entries[selected - 1];
                if (strcmp(entry->status.domain_id, "semaprax.allocation.v1") != 0 || entry->status.code != UINT32_C(1)) return 5;
            }
        }
    }
    puts("checked-native-allocation: ok");
    return 0;
}
