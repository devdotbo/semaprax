/* Phases per row count: complete preflight, one-short refusal, exact encode
 * against the independent byte oracle, then identical reentry on the same
 * input and context. Inputs are prepared before observation and stay live.
 * Payload stores are ordinary bytes_set work and are never reported as copies;
 * String header/NUL bytes are ordinary allocator requests, not payload. */
static void phase_emit(size_t rows, const char *phase, size_t allocated, size_t freed,
                       size_t baseline) {
    phase_on = false;
    REQUIRE(fixture_live == baseline);
    (void)printf("{\"implementation\":\"%s\",\"rows\":%zu,\"phase\":\"%s\","
                 "\"allocation_events\":%zu,\"frees\":%zu,\"bytes_reservations\":%zu,"
                 "\"reserved_payload_bytes\":%zu,\"string_allocations\":%zu,"
                 "\"string_request_bytes\":%zu,\"final_size_string_allocations\":%zu,"
                 "\"peak_live_above_input\":%zu,\"explicit_memcpy_calls\":%zu,"
                 "\"explicit_memcpy_bytes\":%zu,\"final_buffer_copy_calls\":%zu,"
                 "\"final_buffer_copy_bytes\":%zu}\n",
                 IMPLEMENTATION, rows, phase, fixture_allocations - allocated,
                 fixture_frees - freed, phase_reservations, phase_reserved_bytes,
                 phase_strings, phase_string_bytes, phase_final_strings,
                 phase_peak - baseline, fixture_copy_calls, fixture_copy_bytes,
                 fixture_input_copy_calls, fixture_input_copy_bytes);
}

static void case_run(struct spx_context *context, size_t rows, const uint8_t *head,
                     size_t head_length, const uint8_t *row, size_t row_length,
                     size_t length) {
    /* Assemble the Rust oracle pieces with unwrapped libc, outside every phase. */
    REQUIRE(length == head_length + rows * row_length + (rows ? rows - 1 : 0) + 2);
    uint8_t *expected = malloc(length);
    REQUIRE(expected != NULL);
    size_t at = head_length;
    memcpy(expected, head, head_length);
    for (size_t index = 0; index < rows; ++index) {
        if (index) expected[at++] = ',';
        memcpy(expected + at, row, row_length);
        at += row_length;
    }
    expected[at++] = ']';
    expected[at++] = '}';
    REQUIRE(at == length);
    struct REPORT input = {0};
    REQUIRE(PREPARE(context, rows, &input) == SPX_STATUS_SUCCESS);
    size_t baseline = fixture_live;
    spx_slice_u8_v1 oracle = {.ptr = expected, .len = length};
    size_t final_request = offsetof(struct spx_string_v10, data) + length + 1;
    size_t allocated = fixture_allocations, freed = fixture_frees;
    uint64_t required = 0;
    phase_begin(final_request);
    REQUIRE(REQUIRED(context, &input, &required) == SPX_STATUS_SUCCESS && required == length);
    phase_emit(rows, "preflight", allocated, freed, baseline);
    int64_t result = -1;
    allocated = fixture_allocations;
    freed = fixture_frees;
    phase_begin(final_request);
    REQUIRE(RUN(context, &input, required - 1, oracle, &result) == SPX_STATUS_SUCCESS
            && result == 1);
    phase_emit(rows, "one-short-refusal", allocated, freed, baseline);
    for (int pass = 0; pass < 2; ++pass) {
        allocated = fixture_allocations;
        freed = fixture_frees;
        result = -1;
        phase_begin(final_request);
        REQUIRE(RUN(context, &input, required, oracle, &result) == SPX_STATUS_SUCCESS
                && result == 2);
        REQUIRE(context->call_depth == 0 && context->borrowed_str_depth == 0);
        phase_emit(rows, pass ? "reentry" : "exact", allocated, freed, baseline);
    }
    REQUIRE(CONSUME(context, &input, &result) == SPX_STATUS_SUCCESS && result == 0);
    REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
    REQUIRE(context->call_depth == 0 && context->borrowed_str_depth == 0);
    free(expected);
}
