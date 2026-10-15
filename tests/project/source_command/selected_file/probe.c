/* Generated adapter is private; this harness supplies a private root to isolate
 * descriptor lifetime. Checked language-root behavior has separate process cases. */
#undef open
#undef close
static void selected_status(struct spx_context *ctx, spx_status_token token, uint32_t code) {
    const struct spx_normalized_status *value = spx_status_resolve(ctx, token);
    assert(value != NULL && strcmp(value->domain_id, "semaprax.selected-file.v1") == 0);
    assert(value->code == code && value->status_class == SPX_STATUS_CLASS_ADAPTER);
    assert(value->retryability == SPX_RETRYABILITY_FALSE);
}
static spx_status_token probe_root(struct spx_context *ctx, int64_t *result) {
    struct spx_source_command_state_v1 *state = ctx->target_state;
    const char *scenario = (const char *)state->command.input->arguments[2].data;
    char *out = (char *)(uintptr_t)1;
    spx_status_token first = spx_host_file_read_text_v1(ctx, state->selected_path, &out);
    if (strcmp(scenario, "acquire-fail") == 0) {
        selected_status(ctx, first, 5);
        assert(out == (char *)(uintptr_t)1 && observed_opens == 1);
        spx_status_token again = spx_host_file_read_text_v1(ctx, state->selected_path, &out);
        assert(again == first && observed_opens == 1 && out == (char *)(uintptr_t)1);
        assert(state->selected_file == -1);
        return first;
    }
    assert(first == SPX_STATUS_SUCCESS && out != (char *)(uintptr_t)1);
    assert(memcmp(out, "original", 8) == 0);
    spx_string_drop(out);
    held_fd = state->selected_file;
    assert(held_fd >= 0 && observed_opens == 1 && fcntl(held_fd, F_GETFD) >= 0);
    spx_slice_u8_v1 publication = {.ptr = (const uint8_t *)"published", .len = 9};
    assert(spx_host_command_stdout_write_v1(ctx, publication) == 9);
    if (strcmp(scenario, "held") == 0) {
        assert(unlink((const char *)state->selected_path.data) == 0);
        assert(symlink((const char *)state->command.input->arguments[1].data,
            (const char *)state->selected_path.data) == 0);
        out = (char *)(uintptr_t)1;
        assert(spx_host_file_read_text_v1(ctx, state->selected_path, &out) == SPX_STATUS_SUCCESS);
        assert(memcmp(out, "original", 8) == 0 && observed_opens == 1);
        spx_string_drop(out);
        *result = 0;
        return SPX_STATUS_SUCCESS;
    }
    if (strcmp(scenario, "drift") == 0) {
        FILE *file = fopen((const char *)state->selected_path.data, "wb");
        assert(file != NULL && fwrite("changed-size", 1, 12, file) == 12);
        assert(fclose(file) == 0);
        out = (char *)(uintptr_t)1;
        first = spx_host_file_read_text_v1(ctx, state->selected_path, &out);
        selected_status(ctx, first, 7);
        assert(out == (char *)(uintptr_t)1 && observed_opens == 1);
        return first;
    }
    if (strcmp(scenario, "budget") == 0) {
        for (unsigned index = 1; index < 16; ++index) {
            out = (char *)(uintptr_t)1;
            assert(spx_host_file_read_text_v1(ctx, state->selected_path, &out) == SPX_STATUS_SUCCESS);
            spx_string_drop(out);
        }
        out = (char *)(uintptr_t)1;
        first = spx_host_file_read_text_v1(ctx, state->selected_path, &out);
        selected_status(ctx, first, 3);
        assert(state->reserved == UINT64_C(1048576) && state->operations == 17);
        assert(out == (char *)(uintptr_t)1 && observed_opens == 1);
        return first;
    }
    fail_close = 1;
    if (strcmp(scenario, "sticky-root") == 0) {
        first = spx_selected_file_status_v1(ctx, 2);
        selected_status(ctx, first, 2);
        return first;
    }
    assert(strcmp(scenario, "close-fail") == 0);
    *result = 0;
    return SPX_STATUS_SUCCESS;
}
int main(int argc, char **argv) {
    assert(argc == 7);
    int result = spx_selected_file_launch_v1(argc, argv, probe_root);
    const char *scenario = argv[6];
    if (strcmp(scenario, "held") == 0) assert(result == 0);
    else assert(result == 1);
    if (strcmp(scenario, "acquire-fail") == 0) assert(observed_closes == 0);
    else {
        assert(observed_closes == 1 && observed_opens == 1);
        assert(fcntl(held_fd, F_GETFD) == -1 && errno == EBADF);
    }
    return 0;
}
