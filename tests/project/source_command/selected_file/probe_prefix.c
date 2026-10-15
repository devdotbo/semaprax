/* Private owning witness: observe actual OS open/close, never grant source authority. */
#define _POSIX_C_SOURCE 200809L
#define SPX_NO_ENTRY_WRAPPER
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
static unsigned observed_opens, observed_closes;
static int held_fd = -1;
static int fail_close;
static int observed_open(const char *path, int flags, ...) {
    ++observed_opens;
    return open(path, flags);
}
static int observed_close(int descriptor) {
    ++observed_closes;
    assert(descriptor == held_fd);
    int result = close(descriptor);
    assert(result == 0);
    if (fail_close) { errno = EIO; return -1; }
    return result;
}
#define open observed_open
#define close observed_close
