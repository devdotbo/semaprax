int main(void) {
    for (unsigned iteration = 0u; iteration < 4u; iteration += 1u) {
        struct spx_status_entry entries[UINT32_C(16)];
        struct spx_context context = {0};
        if (!spx_context_init(&context, UINT64_C(100000), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
        int64_t result = INT64_C(-7);
        if (spx_decl_6170702e6d61696e(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(42)) return 11;
        if (spx_probe_allocations != 0u || spx_probe_frees != 0u) return 12;
    }
    return 0;
}
