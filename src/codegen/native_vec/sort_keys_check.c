/* Key policy is compiler-owned static metadata, independently replayed in HIR.
 * No allocation, payload clone, integer negation, comparator effect or drop. */
static __attribute__((unused)) void spx_sort_keys_check(uint32_t width,uint32_t count,const uint32_t *keys,const uint32_t *descending){
    if(!keys||!descending||!count||count>width||width>8)
        spx_runtime_invariant_failure("invalid explicit Vec sort keys");
    uint32_t used=0;
    for(uint32_t k=0;k<count;++k){
        if(keys[k]>=width||descending[k]>1||(used&(UINT32_C(1)<<keys[k])))
            spx_runtime_invariant_failure("invalid explicit Vec sort key identity");
        used|=UINT32_C(1)<<keys[k];
    }
}
