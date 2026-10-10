/* Private checked String allocation kernel v1. No production selector uses
 * this precursor. It is a borrowed kernel, not a consuming String callee or
 * a replacement for the status/context/out-slot ABI and canonical cleanup. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

struct spx_checked_string_status_v1 {
    const char *schema;
    const char *domain_id;
    uint32_t code;
    const char *class_name;
    bool retryable;
};
struct spx_checked_string_context_v1 {
    void *allocator_state;
    void *(*allocate)(void *, size_t);
    void (*release)(void *, void *);
    struct spx_checked_string_status_v1 selected;
};
struct spx_checked_string_header_v1 { uint64_t length; char data[]; };

static bool spx_checked_string_fail_v1(
    struct spx_checked_string_context_v1 *ctx, const char *domain, uint32_t code
) {
    if (ctx->selected.code == UINT32_C(0)) {
        ctx->selected = (struct spx_checked_string_status_v1){
            "semaprax.status.v1", domain, code, "adapter", false
        };
    }
    return false;
}
static struct spx_checked_string_header_v1 *spx_checked_string_header_v1(char *text) {
    return (struct spx_checked_string_header_v1 *)
        ((uint8_t *)text - offsetof(struct spx_checked_string_header_v1, data));
}
static uint64_t spx_checked_string_length_v1(const char *text) {
    const struct spx_checked_string_header_v1 *header =
        (const struct spx_checked_string_header_v1 *)
        ((const uint8_t *)text - offsetof(struct spx_checked_string_header_v1, data));
    return header->length;
}
static void spx_checked_string_drop_v1(struct spx_checked_string_context_v1 *ctx, char *text) {
    ctx->release(ctx->allocator_state, spx_checked_string_header_v1(text));
}
static bool spx_checked_string_reserve_v1(
    struct spx_checked_string_context_v1 *ctx, uint64_t length,
    struct spx_checked_string_header_v1 **reserved
) {
    if (ctx->selected.code != UINT32_C(0)) return false;
    if (length > (uint64_t)SIZE_MAX -
            (uint64_t)offsetof(struct spx_checked_string_header_v1, data) - UINT64_C(1))
        return spx_checked_string_fail_v1(ctx, "semaprax.allocation.v1", UINT32_C(2));
    struct spx_checked_string_header_v1 *value = ctx->allocate(
        ctx->allocator_state, offsetof(struct spx_checked_string_header_v1, data) +
            (size_t)length + 1u
    );
    if (value == NULL)
        return spx_checked_string_fail_v1(ctx, "semaprax.allocation.v1", UINT32_C(1));
    /* This is private allocation initialization, not publication into out. */
    value->length = length;
    value->data[length] = '\0';
    *reserved = value;
    return true;
}
static bool spx_checked_string_from_literal_v1(
    struct spx_checked_string_context_v1 *ctx, const char *data, uint64_t length, char **out
) {
    struct spx_checked_string_header_v1 *value;
    if (!spx_checked_string_reserve_v1(ctx, length, &value)) return false;
    if (length != UINT64_C(0)) memcpy(value->data, data, (size_t)length);
    *out = value->data;
    return true;
}
static bool spx_checked_string_clone_v1(
    struct spx_checked_string_context_v1 *ctx, const char *source, char **out
) {
    if (ctx->selected.code != UINT32_C(0)) return false;
    return spx_checked_string_from_literal_v1(ctx, source, spx_checked_string_length_v1(source), out);
}
static bool spx_checked_string_from_str_v1(
    struct spx_checked_string_context_v1 *ctx, const char *data, uint64_t length, char **out
) {
    /* The ordinary borrowed-str provenance gate supplies valid UTF-8. */
    return spx_checked_string_from_literal_v1(ctx, data, length, out);
}
static bool spx_checked_string_utf8_v1(const uint8_t *data, uint64_t length) {
    uint64_t at = UINT64_C(0);
    while (at < length) {
        uint8_t first = data[at++];
        if (first < UINT8_C(0x80)) continue;
        unsigned remaining;
        uint8_t lower = UINT8_C(0x80), upper = UINT8_C(0xbf);
        if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) remaining = 1u;
        else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
            remaining = 2u;
            if (first == UINT8_C(0xe0)) lower = UINT8_C(0xa0);
            if (first == UINT8_C(0xed)) upper = UINT8_C(0x9f);
        } else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
            remaining = 3u;
            if (first == UINT8_C(0xf0)) lower = UINT8_C(0x90);
            if (first == UINT8_C(0xf4)) upper = UINT8_C(0x8f);
        } else return false;
        if (length - at < remaining || data[at] < lower || data[at] > upper) return false;
        ++at;
        while (--remaining != 0u) {
            if (data[at] < UINT8_C(0x80) || data[at] > UINT8_C(0xbf)) return false;
            ++at;
        }
    }
    return true;
}
static bool spx_checked_string_from_utf8_v1(
    struct spx_checked_string_context_v1 *ctx, const uint8_t *data, uint64_t length, char **out
) {
    if (ctx->selected.code != UINT32_C(0)) return false;
    if (length > (uint64_t)SIZE_MAX -
            (uint64_t)offsetof(struct spx_checked_string_header_v1, data) - UINT64_C(1))
        return spx_checked_string_fail_v1(ctx, "semaprax.allocation.v1", UINT32_C(2));
    if (!spx_checked_string_utf8_v1(data, length))
        return spx_checked_string_fail_v1(ctx, "semaprax.convert.v1", UINT32_C(1));
    return spx_checked_string_from_literal_v1(ctx, (const char *)data, length, out);
}
static bool spx_checked_string_from_char_v1(
    struct spx_checked_string_context_v1 *ctx, uint32_t scalar, char **out
) {
    if (ctx->selected.code != UINT32_C(0)) return false;
    if (scalar > UINT32_C(0x10ffff) || (scalar >= UINT32_C(0xd800) && scalar <= UINT32_C(0xdfff)))
        return spx_checked_string_fail_v1(ctx, "semaprax.convert.v1", UINT32_C(1));
    char text[4]; uint64_t length;
    if (scalar < UINT32_C(0x80)) { text[0]=(char)scalar; length=UINT64_C(1); }
    else if (scalar < UINT32_C(0x800)) {
        text[0]=(char)(UINT32_C(0xc0)|(scalar>>6)); text[1]=(char)(UINT32_C(0x80)|(scalar&UINT32_C(0x3f))); length=UINT64_C(2);
    } else if (scalar < UINT32_C(0x10000)) {
        text[0]=(char)(UINT32_C(0xe0)|(scalar>>12)); text[1]=(char)(UINT32_C(0x80)|((scalar>>6)&UINT32_C(0x3f)));
        text[2]=(char)(UINT32_C(0x80)|(scalar&UINT32_C(0x3f))); length=UINT64_C(3);
    } else {
        text[0]=(char)(UINT32_C(0xf0)|(scalar>>18)); text[1]=(char)(UINT32_C(0x80)|((scalar>>12)&UINT32_C(0x3f)));
        text[2]=(char)(UINT32_C(0x80)|((scalar>>6)&UINT32_C(0x3f))); text[3]=(char)(UINT32_C(0x80)|(scalar&UINT32_C(0x3f))); length=UINT64_C(4);
    }
    return spx_checked_string_from_literal_v1(ctx, text, length, out);
}
static bool spx_checked_string_decimal_v1(
    struct spx_checked_string_context_v1 *ctx, uint64_t magnitude, bool negative, char **out
) {
    /* Stack-only rendering: no hidden snprintf/to_string allocation. */
    char text[21]; size_t start=sizeof text;
    do { text[--start]=(char)('0'+magnitude%UINT64_C(10)); magnitude/=UINT64_C(10); } while (magnitude != UINT64_C(0));
    if (negative) text[--start]='-';
    return spx_checked_string_from_literal_v1(ctx, text+start, (uint64_t)(sizeof text-start), out);
}
static bool spx_checked_string_from_i64_v1(
    struct spx_checked_string_context_v1 *ctx, int64_t value, char **out
) {
    uint64_t magnitude = value < INT64_C(0) ? UINT64_C(0)-(uint64_t)value : (uint64_t)value;
    return spx_checked_string_decimal_v1(ctx, magnitude, value < INT64_C(0), out);
}
static bool spx_checked_string_from_usize_v1(
    struct spx_checked_string_context_v1 *ctx, uint64_t value, char **out
) {
    return spx_checked_string_decimal_v1(ctx, value, false, out);
}
static bool spx_checked_string_concat_v1(
    struct spx_checked_string_context_v1 *ctx, const char *left, const char *right, char **out
) {
    if (ctx->selected.code != UINT32_C(0)) return false;
    uint64_t left_length=spx_checked_string_length_v1(left), right_length=spx_checked_string_length_v1(right);
    if (right_length > UINT64_MAX-left_length)
        return spx_checked_string_fail_v1(ctx, "semaprax.allocation.v1", UINT32_C(2));
    struct spx_checked_string_header_v1 *value;
    if (!spx_checked_string_reserve_v1(ctx, left_length+right_length, &value)) return false;
    if (left_length != UINT64_C(0)) memcpy(value->data, left, (size_t)left_length);
    if (right_length != UINT64_C(0)) memcpy(value->data+left_length, right, (size_t)right_length);
    *out=value->data;
    return true;
}
static bool spx_checked_string_collection_clone_v1(
    struct spx_checked_string_context_v1 *ctx, const char *element, char **out
) {
    /* One scalar String leaf only. Aggregate-prefix rollback is the caller's
     * canonical plan obligation and is deliberately not reconstructed here. */
    return spx_checked_string_clone_v1(ctx, element, out);
}
