/* Private adapter evidence: caller owns cells and releases. No canonical
 * CleanupPlan authority is supplied or inferred by this probe. */
#include <assert.h>
#include <stdio.h>
struct tracked {void *pointer; unsigned id;};
struct observer {size_t attempts,fail_at,live;unsigned next,drops[128],drop_count;struct tracked slots[32];};
static void *allocate(void *state,size_t bytes) {
    struct observer *o=state;++o->attempts;
    if(o->attempts==o->fail_at)return NULL;
    void *value=malloc(bytes);assert(value!=NULL);
    for(unsigned i=0;i<32;++i)if(o->slots[i].pointer==NULL){o->slots[i]=(struct tracked){value,++o->next};++o->live;return value;}
    abort();
}
static void release(void *state,void *pointer) {
    struct observer *o=state;
    for(unsigned i=0;i<32;++i)if(o->slots[i].pointer==pointer){
        assert(o->live!=0 && o->drop_count<128);o->drops[o->drop_count++]=o->slots[i].id;
        o->slots[i]=(struct tracked){0};--o->live;free(pointer);return;
    }
    abort();
}
static void initialize(struct spx_context *semantic,struct spx_status_entry *entries,
    struct spx_checked_string_adapter_v1 *adapter,struct observer *observer) {
    assert(spx_context_init(semantic,724,entries,8,NULL,NULL,NULL));
    spx_status_token prior=0;
    assert(spx_status_record_adapter(semantic,"fixture.prior.v1",7,SPX_STATUS_CLASS_ADAPTER,SPX_RETRYABILITY_FALSE,&prior));
    assert(prior==1);
    assert(spx_checked_string_adapter_init_v1(adapter,semantic,observer,allocate,release));
}
static spx_status_token make(unsigned operation,struct spx_checked_string_adapter_v1 *adapter,
    char *left,char *right,char **out) {
    static const char text[]={'a',0,(char)0xce,(char)0xbb};
    switch(operation) {
    case 0:return spx_checked_string_adapter_from_literal_v1(adapter,text,4,out);
    case 1:return spx_checked_string_adapter_clone_v1(adapter,left,out);
    case 2:return spx_checked_string_adapter_from_char_v1(adapter,0,out);
    case 3:return spx_checked_string_adapter_from_i64_v1(adapter,INT64_MIN,out);
    case 4:return spx_checked_string_adapter_from_usize_v1(adapter,UINT64_MAX,out);
    case 5:return spx_checked_string_adapter_from_str_v1(adapter,text,4,out);
    case 6:return spx_checked_string_adapter_from_utf8_v1(adapter,(const uint8_t *)text,4,out);
    case 7:return spx_checked_string_adapter_concat_v1(adapter,left,right,out);
    case 8:return spx_checked_string_adapter_collection_clone_v1(adapter,left,out);
    default:abort();
    }
}
static void exact(unsigned operation,const char *out) {
    static const char text[]={'a',0,(char)0xce,(char)0xbb};
    static const char joined[]={'L',0,(char)0xce,(char)0xbb};
    const char *expected=text;size_t length=4;
    switch(operation) {
    case 1:case 8:expected="L";length=2;break; /* L followed by semantic NUL */
    case 2:expected="";length=1;break; /* U+0000 */
    case 3:expected="-9223372036854775808";length=20;break;
    case 4:expected="18446744073709551615";length=20;break;
    case 7:expected=joined;break;
    default:break;
    }
    assert(spx_checked_string_length_v1(out)==length && memcmp(out,expected,length)==0);
}
static void status(const struct spx_context *semantic,spx_status_token token,const char *domain,uint32_t code) {
    const struct spx_normalized_status *value=spx_status_resolve(semantic,token);
    assert(token==2 && value!=NULL && value->code==code);
    assert(strcmp(value->schema,"semaprax.status.v1")==0 && strcmp(value->domain_id,domain)==0);
    assert(value->status_class==SPX_STATUS_CLASS_ADAPTER && value->retryability==SPX_RETRYABILITY_FALSE);
}
static void constructors(void) {
    struct observer o={0};
    static const char left_bytes[]={'L',0};static const char right_bytes[]={(char)0xce,(char)0xbb};
    for(unsigned operation=0;operation<9;++operation) {
        struct spx_context semantic={0};struct spx_status_entry entries[8];
        struct spx_checked_string_adapter_v1 adapter={0};initialize(&semantic,entries,&adapter,&o);
        char *left=NULL,*right=NULL;
        assert(spx_checked_string_adapter_from_literal_v1(&adapter,left_bytes,2,&left)==0);
        assert(spx_checked_string_adapter_from_literal_v1(&adapter,right_bytes,2,&right)==0);
        assert(!spx_checked_string_adapter_init_v1(&adapter,&semantic,&o,allocate,release));
        size_t before=o.attempts;o.fail_at=before+1;
        unsigned newest=o.next;
        char *out=(char *)(uintptr_t)1;
        spx_status_token failed=make(operation,&adapter,left,right,&out);
        status(&semantic,failed,"semaprax.allocation.v1",1);
        assert(out==(char *)(uintptr_t)1 && o.attempts==before+1 && o.live==2);
        assert(memcmp(left,left_bytes,2)==0 && memcmp(right,right_bytes,2)==0);
        assert(spx_checked_string_adapter_from_literal_v1(&adapter,NULL,UINT64_MAX,&out)==failed);
        assert(spx_checked_string_adapter_from_utf8_v1(&adapter,NULL,UINT64_MAX,&out)==failed);
        assert(o.attempts==before+1 && semantic.status_arena.length==2 && out==(char *)(uintptr_t)1);
        spx_checked_string_drop_v1(&adapter.kernel,right);spx_checked_string_drop_v1(&adapter.kernel,left);
        assert(o.live==0 && o.drops[o.drop_count-2]==newest && o.drops[o.drop_count-1]==newest-1);
        status(&semantic,failed,"semaprax.allocation.v1",1);
        /* New explicit invocation, same allocator and cumulative observer. */
        o.fail_at=0;
        struct spx_context fresh={0};struct spx_status_entry fresh_entries[8];
        struct spx_checked_string_adapter_v1 reuse={0};initialize(&fresh,fresh_entries,&reuse,&o);
        assert(spx_checked_string_adapter_from_literal_v1(&reuse,left_bytes,2,&left)==0);
        assert(spx_checked_string_adapter_from_literal_v1(&reuse,right_bytes,2,&right)==0);
        before=o.attempts;assert(make(operation,&reuse,left,right,&out)==0);
        assert(o.attempts==before+1 && o.live==3);exact(operation,out);
        spx_checked_string_drop_v1(&reuse.kernel,out);spx_checked_string_drop_v1(&reuse.kernel,right);spx_checked_string_drop_v1(&reuse.kernel,left);
        assert(o.live==0 && fresh.status_arena.length==1);
    }
}
static void preflight(void) {
    for(unsigned mode=0;mode<2;++mode) {
        struct observer o={0};struct spx_context semantic={0};struct spx_status_entry entries[8];
        struct spx_checked_string_adapter_v1 adapter={0};initialize(&semantic,entries,&adapter,&o);
        char *out=(char *)(uintptr_t)1;const uint8_t malformed[]={0xc0,0x80};
        spx_status_token failed=mode?spx_checked_string_adapter_from_utf8_v1(&adapter,malformed,2,&out):spx_checked_string_adapter_from_literal_v1(&adapter,NULL,UINT64_MAX,&out);
        status(&semantic,failed,mode?"semaprax.convert.v1":"semaprax.allocation.v1",mode?1:2);
        assert(o.attempts==0 && o.live==0 && out==(char *)(uintptr_t)1);
        assert(spx_checked_string_adapter_from_i64_v1(&adapter,1,&out)==failed);
        assert(o.attempts==0 && semantic.status_arena.length==2);
    }
    struct observer o={0};struct spx_context semantic={0};struct spx_status_entry entry;
    assert(spx_context_init(&semantic,724,&entry,1,NULL,NULL,NULL));spx_status_token token=0;
    assert(spx_status_record_adapter(&semantic,"fixture.full.v1",1,SPX_STATUS_CLASS_ADAPTER,SPX_RETRYABILITY_FALSE,&token));
    struct spx_checked_string_adapter_v1 adapter={0};
    assert(!spx_checked_string_adapter_init_v1(&adapter,&semantic,&o,allocate,release));
    assert(adapter.semantic==NULL && o.attempts==0);
}
int main(void) {assert(fixture_binary_stdout());constructors();preflight();puts("checked-string-status-adapter-v1: ok");return 0;}
