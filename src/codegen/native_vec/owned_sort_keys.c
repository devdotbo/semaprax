static __attribute__((unused)) spx_status_token spx_leaf_sort_keys(
    struct spx_context*c,spx_vec_v1*s,const spx_leaf_layout_v1*d,
    uint32_t count,const uint32_t*keys,const uint32_t*descending,spx_vec_v1*r
){
    struct spx_vec_authority_entry*e=spx_leaf_check(c,s,d);
    spx_sort_keys_check(d->count,count,keys,descending);
    unsigned char*rows=(unsigned char*)(void*)s->ptr;
    for(uint64_t i=1;i<s->len;++i){
        unsigned char row[80];memcpy(row,rows+i*d->stride,d->stride);uint64_t j=i;
        while(j){
            int order=0;
            for(uint32_t k=0;k<count&&!order;++k){uint32_t f=keys[k];
                spx_leaf_layout_v1 selected=*d;selected.count=1;selected.kinds[0]=d->kinds[f];selected.offsets[0]=d->offsets[f];
                const unsigned char*a=rows+(j-1)*d->stride;
                order=descending[k]?spx_leaf_compare(row,a,&selected):spx_leaf_compare(a,row,&selected);
            }
            if(order<=0){break;}
            memcpy(rows+j*d->stride,rows+(j-1)*d->stride,d->stride);
            --j;
        }
        memcpy(rows+j*d->stride,row,d->stride);
    }
    spx_vec_transfer_result(c,s,e,r);return SPX_STATUS_SUCCESS;
}
