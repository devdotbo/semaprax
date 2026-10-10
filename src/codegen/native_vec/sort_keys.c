static __attribute__((unused)) spx_status_token spx_copy_vec_sort_keys(
    struct spx_context*c,spx_vec_v1*s,uint64_t width,const uint32_t*tags,
    uint32_t count,const uint32_t*keys,const uint32_t*descending,spx_vec_v1*r
){
    struct spx_vec_authority_entry*e=spx_copy_vec_check(c,s,width);
    spx_sort_keys_check((uint32_t)width,count,keys,descending);
    uint64_t*v=(uint64_t*)(void*)s->ptr;
    for(uint64_t i=1;i<s->len/width;++i){
        uint64_t row[8]={0};for(uint64_t f=0;f<width;++f)row[f]=v[i*width+f];
        uint64_t j=i;
        while(j){
            int order=0;
            for(uint32_t k=0;k<count&&!order;++k){uint32_t f=keys[k];
                uint64_t a=spx_copy_vec_key(tags[f],v[(j-1)*width+f]),b=spx_copy_vec_key(tags[f],row[f]);
                order=descending[k]?(b>a)-(b<a):(a>b)-(a<b);
            }
            if(order<=0)break;
            for(uint64_t f=0;f<width;++f){v[j*width+f]=v[(j-1)*width+f];}
            --j;
        }
        for(uint64_t f=0;f<width;++f)v[j*width+f]=row[f];
    }
    spx_copy_vec_commit(c,s,e,r);return SPX_STATUS_SUCCESS;
}
