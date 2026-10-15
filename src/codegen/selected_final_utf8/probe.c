/* Actual compiler-generated source factory/encoder/disposer. Input construction
 * is outside final constructor attempts; no hand-built writer substitutes it. */
struct observer {
    struct spx_context context;
    struct spx_status_entry entries[64];
    struct spx_final_utf8_lease *lease;
    void *owners[16];size_t attempts,fail_at,live,releases,retains,state_releases;
};
static void *retain_state(void *state){struct observer *o=state;++o->retains;return state;}
static struct spx_context *held_context(void *state){return &((struct observer *)state)->context;}
static void release_state(void *state){struct observer *o=state;++o->state_releases;assert(o->live==0);}
static void *allocate_selected(void *state,size_t bytes){
    struct observer *o=state;++o->attempts;
    assert(!semaprax_final_utf8_lease_close_v1(o->lease)); /* external callback reentry */
    if(o->attempts==o->fail_at)return NULL;
    void *p=malloc(bytes);assert(p!=NULL);
    for(unsigned i=0;i<16;++i)if(o->owners[i]==NULL){o->owners[i]=p;++o->live;return p;}abort();
}
static void release_selected(void *state,void *p){
    struct observer *o=state;assert(!semaprax_final_utf8_lease_close_v1(o->lease));
    for(unsigned i=0;i<16;++i)if(o->owners[i]==p){o->owners[i]=NULL;assert(o->live!=0);--o->live;++o->releases;free(p);return;}abort();
}
static void initialize(struct observer *o,struct spx_final_utf8_lease *lease){
    assert(spx_context_init(&o->context,724,o->entries,64,NULL,NULL,NULL));o->lease=lease;
    assert(semaprax_final_utf8_lease_init_v1(lease,o,retain_state,held_context,release_state,allocate_selected,release_selected));
    assert(!semaprax_final_utf8_lease_init_v1(lease,o,retain_state,held_context,release_state,allocate_selected,release_selected));
}
static void exact_status(struct observer *o,spx_status_token token,const char *domain,uint32_t code){
    const struct spx_normalized_status *s=spx_status_resolve(&o->context,token);
    assert(token!=0 && s!=NULL && strcmp(s->domain_id,domain)==0 && s->code==code);
    assert(s->status_class==SPX_STATUS_CLASS_ADAPTER && s->retryability==SPX_RETRYABILITY_FALSE);
}
static struct semaprax_final_utf8_report_v1 prepare(struct observer *o,struct spx_final_utf8_lease *lease,uint64_t mode){
    spx_status_token status=99;struct semaprax_final_utf8_report_v1 report={0};
    assert(semaprax_final_utf8_prepare_v1(lease,mode,&status,&report));assert(status==0 && report.identity==lease);
    assert(o->live==1 && physical_live==2);return report;
}
static void finish(struct observer *o,struct spx_final_utf8_lease *lease,struct semaprax_final_utf8_report_v1 report){
    spx_status_token status=99;assert(semaprax_final_utf8_release_report_v1(lease,report,&status));assert(status==0);
    assert(!semaprax_final_utf8_release_report_v1(lease,report,&status));assert(o->live==0 && physical_live==0);
    assert(semaprax_final_utf8_lease_close_v1(lease));assert(o->retains==1 && o->state_releases==1);
    assert(!semaprax_final_utf8_lease_close_v1(lease));
    assert(!semaprax_final_utf8_lease_init_v1(lease,o,retain_state,held_context,release_state,allocate_selected,release_selected));
}
static void exact_output(struct spx_final_utf8_lease *lease,struct spx_final_utf8_handle handle){
    static const uint8_t expected[]={123,34,120,34,58,34,195,169,92,117,48,48,48,48,34,125};
    struct spx_final_utf8_borrow borrow;const uint8_t *data=NULL;uint64_t length=0;
    assert(semaprax_final_utf8_borrow_v1(lease,handle,&borrow,&data,&length));
    assert(length==sizeof expected && memcmp(data,expected,sizeof expected)==0);
    assert(!semaprax_final_utf8_drop_v1(lease,handle));assert(!semaprax_final_utf8_lease_close_v1(lease));
    assert(!semaprax_final_utf8_borrow_v1(lease,handle,&borrow,&data,&length));
    assert(semaprax_final_utf8_end_borrow_v1(lease,borrow));assert(!semaprax_final_utf8_end_borrow_v1(lease,borrow));
    struct spx_final_utf8_borrow next;assert(semaprax_final_utf8_borrow_v1(lease,handle,&next,&data,&length));
    assert(!semaprax_final_utf8_end_borrow_v1(lease,borrow));assert(semaprax_final_utf8_end_borrow_v1(lease,next));
}
static void success_fault_and_handles(void){
    struct observer o={0};struct spx_final_utf8_lease lease={0};initialize(&o,&lease);
    struct semaprax_final_utf8_report_v1 report=prepare(&o,&lease,0);
    struct semaprax_final_utf8_result_v1 result;memset(&result,0xa5,sizeof result);struct semaprax_final_utf8_result_v1 poison=result;
    spx_status_token status=99;size_t before=o.attempts;
    struct semaprax_final_utf8_report_v1 forged=report;++forged.generation;
    assert(!semaprax_final_utf8_invoke_v1(&lease,forged,16,&status,&result));assert(status==99 && memcmp(&result,&poison,sizeof result)==0);
    assert(semaprax_final_utf8_invoke_v1(&lease,report,15,&status,&result));assert(status==0 && result.kind==2 && result.required==16 && o.attempts==before && physical_live==2);
    result=poison;o.fail_at=o.attempts+1;
    assert(semaprax_final_utf8_invoke_v1(&lease,report,16,&status,&result));exact_status(&o,status,"semaprax.allocation.v1",1);
    assert(memcmp(&result,&poison,sizeof result)==0 && physical_live==2 && o.live==1 && o.attempts==before+1);
    o.fail_at=0;assert(semaprax_final_utf8_invoke_v1(&lease,report,16,&status,&result));assert(status==0 && result.kind==1 && o.live==2 && physical_live==3);
    exact_output(&lease,result.owner);
    static unsigned char other_module;
    const void *module=lease.module_identity;lease.module_identity=&other_module;
    assert(!semaprax_final_utf8_drop_v1(&lease,result.owner));
    assert(!semaprax_final_utf8_invoke_v1(&lease,report,16,&status,&poison));
    assert(!semaprax_final_utf8_release_report_v1(&lease,report,&status));
    assert(!semaprax_final_utf8_lease_close_v1(&lease));lease.module_identity=module;
    struct spx_final_utf8_lease copy=lease;assert(!semaprax_final_utf8_drop_v1(&copy,result.owner));
    struct spx_final_utf8_handle bad=result.owner;++bad.generation;assert(!semaprax_final_utf8_drop_v1(&lease,bad));
    bad=result.owner;bad.identity=&copy;assert(!semaprax_final_utf8_drop_v1(&lease,bad));
    size_t saved=o.context.status_arena.length;o.context.status_arena.length=o.context.status_arena.capacity;
    struct semaprax_final_utf8_result_v1 untouched=poison;status=99;before=o.attempts;
    assert(!semaprax_final_utf8_invoke_v1(&lease,report,16,&status,&untouched));assert(status==99 && memcmp(&untouched,&poison,sizeof poison)==0 && o.attempts==before);
    o.context.status_arena.length=saved;
    assert(semaprax_final_utf8_drop_v1(&lease,result.owner));assert(!semaprax_final_utf8_drop_v1(&lease,result.owner));assert(physical_live==2);
    /* Original Report remains usable after allocator failure and handle drop. */
    assert(semaprax_final_utf8_invoke_v1(&lease,report,16,&status,&result));assert(status==0);exact_output(&lease,result.owner);assert(semaprax_final_utf8_drop_v1(&lease,result.owner));
    finish(&o,&lease,report);
}
static void utf8_and_contract_failure(void){
    for(uint64_t mode=1;mode<=2;++mode){
        struct observer o={0};struct spx_final_utf8_lease lease={0};initialize(&o,&lease);
        struct semaprax_final_utf8_report_v1 report=prepare(&o,&lease,mode);
        struct semaprax_final_utf8_result_v1 result;memset(&result,0x5a,sizeof result);struct semaprax_final_utf8_result_v1 before=result;
        spx_status_token status=99;size_t attempts=o.attempts;
        assert(semaprax_final_utf8_invoke_v1(&lease,report,16,&status,&result));assert(status!=0);
        if(mode==1){exact_status(&o,status,"semaprax.convert.v1",1);assert(o.attempts==attempts);}
        else {const struct spx_normalized_status *s=spx_status_resolve(&o.context,status);assert(s!=NULL && s->status_class==SPX_STATUS_CLASS_CONTRACT);assert(o.attempts==attempts+1 && o.releases==1);}
        assert(memcmp(&result,&before,sizeof result)==0 && o.live==1 && physical_live==2);
        finish(&o,&lease,report);
    }
}
static void private_overflow_and_transport_refusal(void){
    struct observer o={0};struct spx_final_utf8_lease lease={0};initialize(&o,&lease);
    o.fail_at=1;spx_status_token status=99;struct semaprax_final_utf8_report_v1 report={0},zero=report;
    assert(!semaprax_final_utf8_prepare_v1(&lease,0,&status,&report));assert(status==99 && memcmp(&report,&zero,sizeof report)==0 && physical_live==0);
    o.fail_at=0;
    /* Unreachable source extent: direct hostile private constructor verifies
     * overflow refusal before an invalid input address is read or allocated. */
    lease.busy=true;lease.active_slot=0;lease.slots[0].reserved=true;o.context.final_utf8_lease=&lease;
    assert(spx_status_record_adapter(&o.context,"semaprax.allocation.v1",1,SPX_STATUS_CLASS_ADAPTER,SPX_RETRYABILITY_FALSE,&lease.allocation_refusal));
    char *out=(char *)(uintptr_t)1;size_t attempts=o.attempts;
    status=spx_final_utf8_construct(&o.context,(const uint8_t *)(uintptr_t)1,UINT64_MAX,&out);
    exact_status(&o,status,"semaprax.allocation.v1",2);assert(out==(char *)(uintptr_t)1 && o.attempts==attempts && physical_live==0);
    assert(spx_final_utf8_construct(&o.context,NULL,UINT64_MAX,&out)==status);assert(o.attempts==attempts);
    o.context.final_utf8_lease=NULL;lease.slots[0].reserved=false;lease.busy=false;
    assert(semaprax_final_utf8_lease_close_v1(&lease));assert(o.state_releases==1);
}
int main(void){success_fault_and_handles();utf8_and_contract_failure();private_overflow_and_transport_refusal();assert(physical_live==0);puts("selected-final-utf8-v1: ok");return 0;}
