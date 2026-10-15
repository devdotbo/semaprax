/* Phases per row count: complete preflight, one-short refusal, exact encode
 * against the independent byte oracle, then identical reentry on the same
 * input and context. Inputs are prepared before observation and stay live.
 * Payload stores are ordinary bytes_set work and are never reported as copies;
 * String header/NUL bytes are ordinary allocator requests, not payload.
 * The exact phase encodes once into one live owned result, compares it with
 * the oracle in external chunks no larger than the ordinary 64 KiB Slice root
 * bound, then hands the same owner to source for its canonical drop. */
#define CHUNK_BYTES ((size_t)65536)

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

/* Every byte and the exact total: consecutive chunks from offset 0 must each
 * match, and together cover exactly `length` bytes. */
static bool held_matches(struct spx_context *context, const struct ENCODED *held,
                         const uint8_t *expected, size_t length) {
    size_t offset = 0;
    do {
        size_t width = length - offset < CHUNK_BYTES ? length - offset : CHUNK_BYTES;
        spx_slice_u8_v1 chunk = {.ptr = width ? expected + offset : NULL, .len = width};
        bool same = false;
        REQUIRE(COMPARE(context, held, offset, length, chunk, &same) == SPX_STATUS_SUCCESS);
        if (!same) return false;
        offset += width;
    } while (offset < length);
    return offset == length;
}

static bool chunk_at(struct spx_context *context, const struct ENCODED *held, size_t offset,
                     size_t total, const uint8_t *expected, size_t width) {
    spx_slice_u8_v1 chunk = {.ptr = width ? expected : NULL, .len = width};
    bool same = false;
    REQUIRE(COMPARE(context, held, offset, total, chunk, &same) == SPX_STATUS_SUCCESS);
    return same;
}

/* Chunk-boundary refusals on one separately encoded owner, outside every
 * observed phase. The oracle is mutated in place and restored. */
static void chunk_controls(struct spx_context *context, const struct REPORT *input,
                           uint8_t *expected, size_t length) {
    struct ENCODED held = {0};
    int64_t result = -1;
    REQUIRE(RENDER(context, input, length, &held) == SPX_STATUS_SUCCESS);
    size_t last = (length - 1) / CHUNK_BYTES * CHUNK_BYTES, width = length - last;
    REQUIRE(width >= 1 && width <= CHUNK_BYTES);
    REQUIRE(chunk_at(context, &held, last, length, expected + last, width));
    REQUIRE(chunk_at(context, &held, length, length, NULL, 0));
    /* Wrong total in either direction refuses an otherwise exact chunk. */
    REQUIRE(!chunk_at(context, &held, last, length - 1, expected + last, width));
    REQUIRE(!chunk_at(context, &held, last, length + 1, expected + last, width));
    /* A final chunk shifted one byte past the end, or an offset beyond it. */
    REQUIRE(!chunk_at(context, &held, last + 1, length, expected + last, width));
    REQUIRE(!chunk_at(context, &held, length + 1, length, NULL, 0));
    /* The first and last byte of every chunk is compared. */
    for (size_t start = 0; start < length; start += CHUNK_BYTES) {
        size_t end = (length - start < CHUNK_BYTES ? length : start + CHUNK_BYTES) - 1;
        size_t edges[2] = {start, end};
        for (int edge = 0; edge < 2; ++edge) {
            expected[edges[edge]] ^= 1u;
            REQUIRE(!held_matches(context, &held, expected, length));
            expected[edges[edge]] ^= 1u;
        }
    }
    /* A one-short oracle refuses even though every compared byte agrees. */
    REQUIRE(!held_matches(context, &held, expected, length - 1));
    REQUIRE(held_matches(context, &held, expected, length));
    REQUIRE(RELEASE(context, &held, &result) == SPX_STATUS_SUCCESS && result == 0);
    REQUIRE(context->call_depth == 0 && context->borrowed_str_depth == 0);
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
    REQUIRE(RUN(context, &input, required - 1, length, &result) == SPX_STATUS_SUCCESS
            && result == 1);
    phase_emit(rows, "one-short-refusal", allocated, freed, baseline);
    for (int pass = 0; pass < 2; ++pass) {
        struct ENCODED held = {0};
        allocated = fixture_allocations;
        freed = fixture_frees;
        result = -1;
        phase_begin(final_request);
        REQUIRE(RENDER(context, &input, required, &held) == SPX_STATUS_SUCCESS);
        REQUIRE(held_matches(context, &held, expected, length));
        REQUIRE(RELEASE(context, &held, &result) == SPX_STATUS_SUCCESS && result == 0);
        REQUIRE(context->call_depth == 0 && context->borrowed_str_depth == 0);
        phase_emit(rows, pass ? "reentry" : "exact", allocated, freed, baseline);
    }
    chunk_controls(context, &input, expected, length);
    REQUIRE(CONSUME(context, &input, &result) == SPX_STATUS_SUCCESS && result == 0);
    REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
    REQUIRE(context->call_depth == 0 && context->borrowed_str_depth == 0);
    free(expected);
}
