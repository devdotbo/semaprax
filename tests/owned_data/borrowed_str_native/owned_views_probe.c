/* Private algorithm oracle, independently compared with bounded naive search. */
static bool reference_contains(const uint8_t *value, uint64_t n,
    const uint8_t *needle, uint64_t m) {
    if (m == 0) return true;
    if (m > n) return false;
    for (uint64_t i = 0; i <= n - m; ++i) {
        if (memcmp(value + i, needle, (size_t)m) == 0) return true;
    }
    return false;
}
int main(void) {
    uint8_t value[8], needle[6];
    for (uint64_t n = 0; n <= 8; ++n) {
        for (uint64_t m = 0; m <= 6; ++m) {
            for (uint64_t a = 0; a < (UINT64_C(1) << n); ++a) {
                for (uint64_t b = 0; b < (UINT64_C(1) << m); ++b) {
                    for (uint64_t i = 0; i < n; ++i) value[i] = (uint8_t)((a >> i) & 1u);
                    for (uint64_t i = 0; i < m; ++i) needle[i] = (uint8_t)((b >> i) & 1u);
                    spx_str_v1 v = {value, n}, p = {needle, m};
                    if (spx_str_contains(v, p) != reference_contains(value, n, needle, m)) return 11;
                }
            }
        }
    }
    static uint8_t long_value[262144], long_needle[131073];
    memset(long_value, 'a', sizeof(long_value));
    memset(long_needle, 'a', sizeof(long_needle));
    long_needle[sizeof(long_needle) - 1u] = 'b';
    spx_str_v1 v = {long_value, sizeof(long_value)}, p = {long_needle, sizeof(long_needle)};
    if (spx_str_contains(v, p)) return 12;
    long_value[sizeof(long_value) - 1u] = 'b';
    if (!spx_str_contains(v, p)) return 13;
    long_needle[0] = 0;
    if (spx_str_contains(v, p)) return 14;
    long_value[sizeof(long_value) - sizeof(long_needle)] = 0;
    if (!spx_str_contains(v, p)) return 15;
    const uint8_t unicode[] = {0xe2, 0x82, 0xac, 0, 0xf0, 0x9f, 0x98, 0x80};
    const uint8_t suffix[] = {0, 0xf0, 0x9f, 0x98, 0x80};
    spx_str_v1 u = {unicode, sizeof(unicode)}, s = {suffix, sizeof(suffix)};
    if (!spx_str_contains(u, s) || !spx_str_contains(u, u)) return 16;
    return 0;
}
