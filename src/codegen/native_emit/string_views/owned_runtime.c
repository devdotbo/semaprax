#include <limits.h>
#include <stddef.h>
#include <string.h>

typedef struct {
    const uint8_t *data;
    uint64_t len;
} spx_str_v1;

#define SPX_BORROWED_STR_MAX_BYTES UINT64_C(65536)

static __attribute__((unused)) void spx_str_require_valid(spx_str_v1 value) {
    if (value.len != UINT64_C(0) && value.data == NULL) {
        spx_runtime_invariant_failure("borrowed str has null data with nonzero length");
    }
    if (value.len > (uint64_t)SIZE_MAX
        || value.len > (uint64_t)INT64_MAX) {
        spx_runtime_invariant_failure("borrowed str length exceeds native profile");
    }
    uint64_t offset = UINT64_C(0);
    while (offset < value.len) {
        const uint8_t first = value.data[offset];
        uint64_t width = UINT64_C(0);
        if (first <= UINT8_C(0x7f)) {
            width = UINT64_C(1);
        } else if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) {
            width = UINT64_C(2);
        } else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
            width = UINT64_C(3);
        } else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
            width = UINT64_C(4);
        } else {
            spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
        }
        if (width > value.len - offset) {
            spx_runtime_invariant_failure("borrowed str has truncated UTF-8");
        }
        if (width >= UINT64_C(2)) {
            const uint8_t second = value.data[offset + UINT64_C(1)];
            if ((second & UINT8_C(0xc0)) != UINT8_C(0x80)
                || (first == UINT8_C(0xe0) && second < UINT8_C(0xa0))
                || (first == UINT8_C(0xed) && second > UINT8_C(0x9f))
                || (first == UINT8_C(0xf0) && second < UINT8_C(0x90))
                || (first == UINT8_C(0xf4) && second > UINT8_C(0x8f))) {
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
            }
        }
        for (uint64_t tail = UINT64_C(2); tail < width; ++tail) {
            if ((value.data[offset + tail] & UINT8_C(0xc0)) != UINT8_C(0x80)) {
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
            }
        }
        offset += width;
    }
}

static __attribute__((unused)) int64_t spx_str_len_bytes(spx_str_v1 value) {
    spx_str_require_valid(value);
    return (int64_t)value.len;
}

static __attribute__((unused)) bool spx_str_is_empty(spx_str_v1 value) {
    spx_str_require_valid(value);
    return value.len == UINT64_C(0);
}

static __attribute__((unused)) bool spx_str_starts_with(
    spx_str_v1 value, spx_str_v1 prefix
) {
    spx_str_require_valid(value);
    spx_str_require_valid(prefix);
    if (prefix.len == UINT64_C(0)) return true;
    if (prefix.len > value.len) return false;
    return memcmp(value.data, prefix.data, (size_t)prefix.len) == 0;
}

/* Critical factorization and Two-Way search use constant scratch space.
 * split is the maximal-suffix position plus one: no negative index sentinel. */
typedef struct { uint64_t split; uint64_t period; } spx_str_factor_v1;
static __attribute__((unused)) spx_str_factor_v1 spx_str_factor(
    spx_str_v1 needle, bool reverse
) {
    uint64_t split = 0, candidate = 0, offset = 1, period = 1;
    while (offset < needle.len - candidate) {
        const uint8_t left = needle.data[split + offset - 1];
        const uint8_t right = needle.data[candidate + offset];
        if (left == right) {
            if (offset == period) { candidate += period; offset = 1; }
            else ++offset;
        } else if (reverse ? left < right : left > right) {
            candidate += offset;
            offset = 1;
            period = candidate + 1 - split;
        } else {
            split = candidate + 1;
            ++candidate;
            offset = period = 1;
        }
    }
    return (spx_str_factor_v1){split, period};
}

static __attribute__((unused)) bool spx_str_contains(
    spx_str_v1 value, spx_str_v1 needle
) {
    spx_str_require_valid(value);
    spx_str_require_valid(needle);
    if (needle.len == 0) return true;
    if (needle.len > value.len) return false;
    const spx_str_factor_v1 forward = spx_str_factor(needle, false);
    const spx_str_factor_v1 backward = spx_str_factor(needle, true);
    const spx_str_factor_v1 factor = forward.split > backward.split ? forward : backward;
    const uint64_t split = factor.split;
    uint64_t period = factor.period;
    uint64_t retained = 0;
    if (period <= needle.len && split <= needle.len - period
        && memcmp(needle.data, needle.data + period, (size_t)split) == 0) {
        retained = needle.len - period;
    } else {
        const uint64_t left = split == 0 ? 0 : split - 1;
        const uint64_t right = needle.len - split;
        period = (left > right ? left : right) + 1;
    }
    const uint64_t last = value.len - needle.len;
    uint64_t position = 0, memory = 0;
    for (;;) {
        uint64_t matched = split > memory ? split : memory;
        while (matched < needle.len
            && needle.data[matched] == value.data[position + matched]) ++matched;
        if (matched < needle.len) {
            const uint64_t advance = matched - split + 1;
            if (advance > last - position) return false;
            position += advance;
            memory = 0;
        } else {
            matched = split;
            while (matched > memory
                && needle.data[matched - 1] == value.data[position + matched - 1]) --matched;
            if (matched <= memory) return true;
            if (period > last - position) return false;
            position += period;
            memory = retained;
        }
    }
}

/* Only the checked call-depth-zero boundary may originate foreign Str input.
 * Reject extent before dereferencing; forwarding never mints root authority. */
static __attribute__((unused)) void spx_str_require_root_valid(spx_str_v1 value) {
    if (value.len > SPX_BORROWED_STR_MAX_BYTES) {
        spx_runtime_invariant_failure("borrowed str length exceeds native profile");
    }
    spx_str_require_valid(value);
}
