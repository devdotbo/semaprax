int main(void) {
    struct spx_status_entry entries[16];struct spx_context context={0};
    REQUIRE(spx_context_init(&context,737,entries,16,NULL,NULL,NULL));
    for(uint64_t length=0;length<=64;length+=64) {
        size_t allocations=fixture_allocations;
        spx_bytes_v1 result={0};REQUIRE(ALLOCATE(&context,length,&result)==SPX_STATUS_SUCCESS);
        REQUIRE(result.len==length);REQUIRE(fixture_allocations-allocations==(length?1:0));
        if(length) {bool found=false;for(size_t slot=0;slot<512;++slot)if(fixture_table[slot].pointer==result.ptr){REQUIRE(fixture_table[slot].size==length);found=true;}REQUIRE(found);}
        spx_bytes_drop(&result);REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    }
    for(unsigned phase=0;phase<2;++phase) {
        size_t allocations=fixture_allocations,frees=fixture_frees;
        spx_bytes_v1 result={.ptr=(uint8_t *)(uintptr_t)1,.len=999};
        spx_status_token status=ALLOCATE(&context,phase?UINT64_MAX:65,&result);
        const struct spx_normalized_status *normalized=spx_status_resolve(&context,status);
        REQUIRE(normalized!=NULL && normalized->code==2 && strcmp(normalized->domain_id,"semaprax.byte-buffer.v1")==0);
        REQUIRE(status!=SPX_STATUS_SUCCESS);REQUIRE(result.ptr==(uint8_t *)(uintptr_t)1 && result.len==999);
        REQUIRE(fixture_allocations==allocations && fixture_frees==frees && fixture_live==0);
        REQUIRE(context.call_depth==0);
        allocations=fixture_allocations;frees=fixture_frees;
        status=AFTER(&context,phase?UINT64_MAX:65,&result);
        normalized=spx_status_resolve(&context,status);
        REQUIRE(normalized!=NULL && normalized->code==2 && strcmp(normalized->domain_id,"semaprax.byte-buffer.v1")==0);
        REQUIRE(status!=SPX_STATUS_SUCCESS && result.ptr==(uint8_t *)(uintptr_t)1 && result.len==999);
        REQUIRE(fixture_allocations==allocations+1 && fixture_frees==frees+1 && fixture_live==0);
    }
    return 0;
}
