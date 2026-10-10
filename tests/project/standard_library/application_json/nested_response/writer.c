/* Same ordinary context for all cases; no Fixed/4096 counter resets.
 * This phase invokes the actual checked derived quote helper, not a full
 * request/response app. calloc reserves Bytes; malloc materializes String.
 * memcpy is explicit runtime copy traffic, not all machine memory writes. */
#define CHECK_CASE(input, input_length, expected, expected_length) do { \
    spx_str_v1 view = {.data=(input), .len=(input_length)}; \
    size_t allocated=fixture_allocations,freed=fixture_frees; \
    char *result=NULL; fixture_watch(NULL,0);writer_reservations=writer_reserved_size=0; \
    REQUIRE(WRITER(&context,view,(expected_length),&result)==SPX_STATUS_SUCCESS); \
    REQUIRE(spx_string_length_v10(result)==(expected_length)); \
    REQUIRE(!memcmp(result,(expected),(expected_length))); \
    REQUIRE(fixture_allocations-allocated==2); \
    REQUIRE(writer_reservations==1 && writer_reserved_size==(expected_length)); \
    REQUIRE(fixture_frees-freed==1 && fixture_live==1); \
    REQUIRE(fixture_copy_calls==1 && fixture_copy_bytes==(expected_length)); \
    size_t buffer_allocations=0,string_allocations=0; \
    for(size_t slot=0;slot<512;++slot) { \
        if(fixture_table[slot].pointer==spx_string_header_v10(result)) { \
            REQUIRE(fixture_table[slot].size==offsetof(struct spx_string_v10,data)+(expected_length)+1); \
            ++string_allocations; \
        } \
        if(fixture_table[slot].pointer && fixture_table[slot].pointer!=spx_string_header_v10(result)) ++buffer_allocations; \
    } \
    REQUIRE(string_allocations==1 && buffer_allocations==0); \
    spx_string_drop(result); \
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees); \
    REQUIRE(context.call_depth==0 && context.borrowed_str_depth==0); \
    char *poison=(char *)(uintptr_t)1;allocated=fixture_allocations;freed=fixture_frees; \
    REQUIRE(WRITER(&context,view,(expected_length)-1,&poison)!=SPX_STATUS_SUCCESS); \
    REQUIRE(poison==(char *)(uintptr_t)1); \
    REQUIRE(fixture_allocations==allocated && fixture_frees==freed && fixture_live==0); \
} while(0)
