/* Probe-only classification layered over the unchanged TOTAL memcpy observer.
 * This does not replace a backend operation, filter the total, or change owners.
 * Only the exact checked String-slot extraction call is separately identified;
 * every forward reaches fixture_bulk_memcpy once, including descriptor bytes. */
#undef memcpy
static size_t fixture_slot_copy_calls, fixture_slot_copy_bytes;
static size_t fixture_other_copy_calls, fixture_other_copy_bytes;
static size_t fixture_payload_copy_calls, fixture_payload_copy_bytes;
static const uint8_t *fixture_payload_expected;
static size_t fixture_payload_length;
static size_t fixture_copy_owner(const void *pointer, size_t length) {
    uintptr_t at=(uintptr_t)pointer;
    for(size_t slot=0;slot<512;++slot){
        uintptr_t start=(uintptr_t)fixture_table[slot].pointer;
        size_t size=fixture_table[slot].size;
        if(start && at>=start && at-start<=size && length<=size-(at-start))return slot;
    }
    return SIZE_MAX;
}
static void fixture_payload_watch(const uint8_t *expected,size_t length) {
    fixture_payload_expected=expected;fixture_payload_length=length;
}
static void fixture_copy_reset(void) {
    fixture_watch(NULL,0);
    fixture_slot_copy_calls=fixture_slot_copy_bytes=0;
    fixture_other_copy_calls=fixture_other_copy_bytes=0;
    fixture_payload_copy_calls=fixture_payload_copy_bytes=0;
}
static void *fixture_other_memcpy(void *output,const void *input,size_t length) {
    ++fixture_other_copy_calls;fixture_other_copy_bytes+=length;
    size_t from=fixture_copy_owner(input,length),to=fixture_copy_owner(output,length);
    /* Complete independent wire bytes, different actual live owning allocations.
     * Length alone never establishes a payload (pointer-width payloads included). */
    if(fixture_payload_expected && length==fixture_payload_length && from!=SIZE_MAX
       && to!=SIZE_MAX && from!=to && !memcmp(input,fixture_payload_expected,length)){
        ++fixture_payload_copy_calls;fixture_payload_copy_bytes+=length;
    }
    return fixture_bulk_memcpy(output,input,length);
}
static void *fixture_slot_memcpy(void *output,const void *input,size_t length) {
    /* The exact compiler operation established primitive String kind9/offset0
     * and acquired this field through checked spx_leaf_read_field. Validate actual
     * living descriptor storage and a separate living owned String allocation.
     * Introspection copies use ordinary libc, like all observer-only setup. */
    REQUIRE(length==sizeof(const char *));
    size_t descriptor_owner=fixture_copy_owner(input,length);
    REQUIRE(descriptor_owner!=SIZE_MAX);
    const char *payload=NULL;memcpy(&payload,input,sizeof payload);
    size_t payload_owner=fixture_copy_owner(payload,1);
    REQUIRE(payload_owner!=SIZE_MAX && payload_owner!=descriptor_owner);
    REQUIRE(fixture_copy_owner(output,length)==SIZE_MAX);
    ++fixture_slot_copy_calls;fixture_slot_copy_bytes+=length;
    return fixture_bulk_memcpy(output,input,length);
}
static void fixture_copy_classification_control(void) {
    REQUIRE(fixture_live==0 && sizeof(const char *)==8);
    uint8_t *payload=fixture_malloc(8),*received=fixture_malloc(8);
    void *descriptor=fixture_malloc(sizeof(const char *));
    const uint8_t expected[8]={'1','2','3','4','5','6','7','8'};
    memcpy(payload,expected,8);const char *stored=(const char *)payload;
    memcpy(descriptor,&stored,sizeof stored);
    fixture_payload_watch(expected,8);fixture_copy_reset();
    fixture_other_memcpy(received,payload,8);
    const char *borrowed=NULL;fixture_slot_memcpy(&borrowed,descriptor,sizeof borrowed);
    REQUIRE(borrowed==(const char *)payload && !memcmp(received,expected,8));
    REQUIRE(fixture_copy_calls==2 && fixture_copy_bytes==16);
    REQUIRE(fixture_slot_copy_calls==1 && fixture_slot_copy_bytes==8);
    REQUIRE(fixture_other_copy_calls==1 && fixture_other_copy_bytes==8);
    REQUIRE(fixture_payload_copy_calls==1 && fixture_payload_copy_bytes==8);
    fixture_free(descriptor);fixture_free(received);fixture_free(payload);
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    fixture_payload_watch(NULL,0);fixture_copy_reset();
}
#define memcpy fixture_other_memcpy
