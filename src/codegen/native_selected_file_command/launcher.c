typedef spx_status_token (*spx_selected_file_root_v1)(struct spx_context *, int64_t *);

static int spx_selected_file_usage_v1(void) {
    fputs("SEMAPRAX selected-file command: optional --spx-selected-file-v1 INDEX -- precedes unchanged application arguments (INDEX 0..15)\n", stderr);
    (void)fflush(stderr);
    return 2;
}
static bool spx_selected_file_index_v1(const char *text, uint64_t *out) {
    if (text == NULL || text[0] == '\0' || (text[0] == '0' && text[1] != '\0')) return false;
    uint64_t value = UINT64_C(0);
    for (size_t index = 0; text[index] != '\0'; ++index) {
        if (index >= 2u || text[index] < '0' || text[index] > '9') return false;
        value = value * UINT64_C(10) + (uint64_t)(text[index] - '0');
        if (value >= SPX_COMMAND_ARGUMENT_LIMIT_V1) return false;
    }
    *out = value;
    return true;
}
static bool spx_selected_file_flush_v1(FILE *stream, const uint8_t *bytes, uint64_t length) {
    if (length != UINT64_C(0) && fwrite(bytes, sizeof(uint8_t), (size_t)length, stream) != (size_t)length) return false;
    return fflush(stream) == 0;
}
static __attribute__((unused)) int spx_selected_file_launch_v1(
    int argc, char **argv, spx_selected_file_root_v1 root
) {
    if (argc < 1 || argv == NULL || root == NULL) return spx_selected_file_usage_v1();
    int first = 1;
    bool granted = false;
    uint64_t selected = UINT64_C(0);
    if (argc > 1 && argv[1] != NULL && strcmp(argv[1], "--spx-selected-file-v1") == 0) {
        if (argc < 5 || argv[3] == NULL || strcmp(argv[3], "--") != 0 ||
            !spx_selected_file_index_v1(argv[2], &selected)) return spx_selected_file_usage_v1();
        first = 4;
        granted = true;
    }
    if (argc - first > (int)SPX_COMMAND_ARGUMENT_LIMIT_V1 ||
        (granted && selected >= (uint64_t)(argc - first))) return spx_selected_file_usage_v1();
    struct spx_language_command_input_v1 input = {0};
    uint64_t total = UINT64_C(0);
    for (int index = first; index < argc; ++index) {
        const char *argument = argv[index];
        uint64_t length = UINT64_C(0);
        while (argument != NULL && length <= SPX_COMMAND_INPUT_CAPACITY_V1 && argument[length] != '\0') ++length;
        if (argument == NULL || length > SPX_COMMAND_INPUT_CAPACITY_V1 - total ||
            !spx_command_utf8_v1(length == UINT64_C(0) ? NULL : (const uint8_t *)argument, length))
            return spx_selected_file_usage_v1();
        input.arguments[input.argument_count++] = (spx_str_v1){
            .data = length == UINT64_C(0) ? NULL : (const uint8_t *)argument, .len = length
        };
        total += length;
    }
    if (!spx_language_command_input_is_valid_v1(&input)) return spx_selected_file_usage_v1();
    /* No descriptor is acquired until checked source requests the selected
       key. In particular app usage/help cannot be masked by a missing file. */
    struct spx_source_command_state_v1 state = {0};
    state.command.input = &input;
    state.granted = granted;
    state.selected_file = -1;
    if (granted) {
        state.selected_path = input.arguments[selected];
        /* Empty and oversized path spellings fail at the read boundary, after
           application usage. They never grant authority to another string. */
        if (state.selected_path.len == UINT64_C(0) || state.selected_path.len > UINT64_C(4096)) {
            state.attempted = true;
            state.acquisition_failure = UINT32_C(5);
        }
    }
#if !defined(_WIN32)
    (void)signal(SIGPIPE, SIG_IGN);
#endif
    struct spx_status_entry entries[UINT32_C(1)];
    struct spx_context ctx = {0};
    if (!spx_context_init(&ctx, UINT64_C(1), entries, UINT32_C(1), NULL, NULL, &state))
        spx_runtime_invariant_failure("selected-file context initialization");
    int64_t result = INT64_C(0);
    spx_status_token status = root(&ctx, &result);
#if !defined(_WIN32)
    if (state.selected_file >= 0) {
        int descriptor = state.selected_file;
        state.selected_file = -1;
        /* Never retry close: a reused descriptor must not be closed twice. */
        if (close(descriptor) != 0 && status == SPX_STATUS_SUCCESS)
            status = spx_selected_file_status_v1(&ctx, UINT32_C(5));
    }
#endif
    if (status != SPX_STATUS_SUCCESS) {
        (void)spx_public_failure(&ctx, status);
        return 1;
    }
    if (result < INT64_C(0) || result > INT64_C(255)) {
        fputs("SEMAPRAX selected-file command: main result outside 0..255\n", stderr);
        return 1;
    }
    if (!spx_selected_file_flush_v1(stderr, state.command.output.stderr_bytes, state.command.output.stderr_length) ||
        !spx_selected_file_flush_v1(stdout, state.command.output.stdout_bytes, state.command.output.stdout_length)) return 1;
    return (int)result;
}
