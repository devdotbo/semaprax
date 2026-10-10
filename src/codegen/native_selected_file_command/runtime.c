static __attribute__((unused)) void spx_selected_file_output_table_v1(void) {
    (void)&spx_host_command_stdout_write_v1;
    (void)&spx_host_command_stderr_write_v1;
}
#if !defined(_WIN32)
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/stat.h>
#include <unistd.h>
#endif
#define SPX_SELECTED_FILE_TEXT_MAX_V1 UINT64_C(65536)
#define SPX_SELECTED_FILE_OPERATIONS_MAX_V1 UINT64_C(64)
#define SPX_SELECTED_FILE_RESERVED_MAX_V1 UINT64_C(1048576)

struct spx_source_command_state_v1 {
    struct spx_language_command_state_v1 command;
    spx_str_v1 selected_path;
    bool granted;
    bool attempted;
    int selected_file;
    uint32_t acquisition_failure;
    spx_status_token failure;
    uint64_t operations;
    uint64_t reserved;
#if !defined(_WIN32)
    struct stat acquired;
#endif
};
_Static_assert(offsetof(struct spx_source_command_state_v1, command) == 0,
    "selected input command must retain command prefix");

static spx_status_token spx_selected_file_status_v1(struct spx_context *ctx, uint32_t code) {
    struct spx_source_command_state_v1 *state = ctx->target_state;
    if (state->failure != SPX_STATUS_SUCCESS) return state->failure;
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(ctx, "semaprax.selected-file.v1", code,
        SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token))
        spx_runtime_invariant_failure("selected file status carrier unavailable");
    state->failure = token;
    return token;
}
#if !defined(_WIN32)
static bool spx_selected_file_same_v1(const struct stat *left, const struct stat *right) {
    if (left->st_dev != right->st_dev || left->st_ino != right->st_ino ||
        left->st_size != right->st_size || left->st_mode != right->st_mode) return false;
#if defined(__APPLE__)
    return left->st_mtimespec.tv_sec == right->st_mtimespec.tv_sec &&
        left->st_mtimespec.tv_nsec == right->st_mtimespec.tv_nsec &&
        left->st_ctimespec.tv_sec == right->st_ctimespec.tv_sec &&
        left->st_ctimespec.tv_nsec == right->st_ctimespec.tv_nsec;
#else
    return left->st_mtim.tv_sec == right->st_mtim.tv_sec &&
        left->st_mtim.tv_nsec == right->st_mtim.tv_nsec &&
        left->st_ctim.tv_sec == right->st_ctim.tv_sec &&
        left->st_ctim.tv_nsec == right->st_ctim.tv_nsec;
#endif
}
static uint32_t spx_selected_file_acquire_v1(struct spx_source_command_state_v1 *state) {
    if (state->attempted) return state->acquisition_failure;
    state->attempted = true;
    /* argv was bounded and NUL-terminated by the launcher. The source key
       cannot replace it; only this sealed invocation spelling reaches open. */
    state->selected_file = open((const char *)state->selected_path.data,
        O_RDONLY | O_NONBLOCK | O_CLOEXEC);
    uint32_t code = UINT32_C(0);
    if (state->selected_file < 0)
        code = errno == ELOOP ? UINT32_C(4) : UINT32_C(5);
    else if (fstat(state->selected_file, &state->acquired) != 0) code = UINT32_C(5);
    else if (!S_ISREG(state->acquired.st_mode)) code = UINT32_C(4);
    else if (state->acquired.st_size < 0 ||
        (uint64_t)state->acquired.st_size > SPX_SELECTED_FILE_TEXT_MAX_V1) code = UINT32_C(3);
    state->acquisition_failure = code;
    return code;
}
#endif
static __attribute__((unused)) spx_status_token spx_host_file_read_text_v1(
    struct spx_context *ctx, spx_str_v1 key, char **result_out
) {
    if (ctx == NULL || ctx->target_state == NULL || result_out == NULL)
        spx_runtime_invariant_failure("selected file context unavailable");
    struct spx_source_command_state_v1 *state = (struct spx_source_command_state_v1 *)ctx->target_state;
    /* Reservation order is independent of grant/key validity. */
    if (state->operations >= SPX_SELECTED_FILE_OPERATIONS_MAX_V1 ||
        state->reserved > SPX_SELECTED_FILE_RESERVED_MAX_V1 - SPX_SELECTED_FILE_TEXT_MAX_V1) {
        state->operations += UINT64_C(1);
        return spx_selected_file_status_v1(ctx, UINT32_C(3));
    }
    state->operations += UINT64_C(1);
    state->reserved += SPX_SELECTED_FILE_TEXT_MAX_V1;
    if (state->failure != SPX_STATUS_SUCCESS) return state->failure;
    if (!state->granted) return spx_selected_file_status_v1(ctx, UINT32_C(1));
    if (key.len != state->selected_path.len ||
        (key.len != UINT64_C(0) && (key.data == NULL ||
            memcmp(key.data, state->selected_path.data, (size_t)key.len) != 0)))
        return spx_selected_file_status_v1(ctx, UINT32_C(2));
#if defined(_WIN32)
    return spx_selected_file_status_v1(ctx, UINT32_C(1));
#else
    uint32_t code = spx_selected_file_acquire_v1(state);
    if (code != UINT32_C(0)) return spx_selected_file_status_v1(ctx, code);
    struct stat before;
    if (fstat(state->selected_file, &before) != 0) return spx_selected_file_status_v1(ctx, UINT32_C(5));
    if (!spx_selected_file_same_v1(&state->acquired, &before))
        return spx_selected_file_status_v1(ctx, UINT32_C(7));
    uint8_t *buffer = (uint8_t *)malloc((size_t)SPX_SELECTED_FILE_TEXT_MAX_V1 + 1u);
    if (buffer == NULL) return spx_selected_file_status_v1(ctx, UINT32_C(5));
    uint64_t length = UINT64_C(0);
    while (code == UINT32_C(0)) {
        ssize_t count = pread(state->selected_file, buffer + (size_t)length,
            (size_t)(SPX_SELECTED_FILE_TEXT_MAX_V1 + UINT64_C(1) - length), (off_t)length);
        if (count < 0) {
            if (errno == EINTR) continue;
            code = UINT32_C(5);
        } else if (count == 0) break;
        else {
            length += (uint64_t)count;
            if (length > SPX_SELECTED_FILE_TEXT_MAX_V1) code = UINT32_C(3);
        }
    }
    struct stat after;
    if (code == UINT32_C(0) && fstat(state->selected_file, &after) != 0) code = UINT32_C(5);
    if (code == UINT32_C(0) && (!spx_selected_file_same_v1(&before, &after) ||
        (uint64_t)after.st_size != length)) code = UINT32_C(7);
    if (code == UINT32_C(0) && !spx_command_utf8_v1(length == UINT64_C(0) ? NULL : buffer, length)) code = UINT32_C(6);
    if (code != UINT32_C(0)) {
        free(buffer);
        return spx_selected_file_status_v1(ctx, code);
    }
    char *result = spx_string_from_literal((const char *)buffer, length);
    free(buffer);
    *result_out = result;
    return SPX_STATUS_SUCCESS;
#endif
}
