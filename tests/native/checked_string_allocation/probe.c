/* Physical NULL allocation evidence for the private kernel only. This harness
 * owns its explicit input/result cells; it is not generated cleanup evidence. */
#include <assert.h>
#include <stdlib.h>
#include <stdio.h>

struct observed_allocation { void *pointer; unsigned id; };
struct observed_allocator {
    size_t attempts, fail_at, live;
    unsigned next_id, dropped[64], drop_count;
    struct observed_allocation slots[64];
};
static void *observed_allocate(void *state, size_t bytes) {
    struct observed_allocator *observer=state;
    ++observer->attempts;
    if (observer->attempts==observer->fail_at) return NULL;
    void *value=malloc(bytes);
    assert(value != NULL);
    for (size_t slot=0;slot<64u;++slot) if (observer->slots[slot].pointer==NULL) {
        observer->slots[slot]=(struct observed_allocation){value,++observer->next_id};
        ++observer->live;
        return value;
    }
    abort();
}
static void observed_release(void *state, void *pointer) {
    struct observed_allocator *observer=state;
    for (size_t slot=0;slot<64u;++slot) if (observer->slots[slot].pointer==pointer) {
        assert(pointer != NULL && observer->live != 0u && observer->drop_count < 64u);
        observer->dropped[observer->drop_count++]=observer->slots[slot].id;
        observer->slots[slot].pointer=NULL;
        --observer->live;
        free(pointer);
        return;
    }
    abort(); /* double/foreign drop, never a successful settlement */
}
static struct spx_checked_string_context_v1 context(struct observed_allocator *observer) {
    struct spx_checked_string_context_v1 ctx={observer,observed_allocate,observed_release,{0}};
    return ctx;
}
static void reset(struct spx_checked_string_context_v1 *ctx, size_t fail_at) {
    struct observed_allocator *observer=ctx->allocator_state;
    assert(observer->live==0u);
    *observer=(struct observed_allocator){0};
    observer->fail_at=fail_at;
    ctx->selected=(struct spx_checked_string_status_v1){0};
}
static void allocation_failure(const struct spx_checked_string_context_v1 *ctx, uint32_t code) {
    assert(strcmp(ctx->selected.schema,"semaprax.status.v1")==0);
    assert(strcmp(ctx->selected.domain_id,"semaprax.allocation.v1")==0);
    assert(ctx->selected.code==code);
    assert(strcmp(ctx->selected.class_name,"adapter")==0);
    assert(!ctx->selected.retryable);
}
static bool make(unsigned operation, struct spx_checked_string_context_v1 *ctx,
                 char *left, char *right, char **out) {
    static const char text[]={'a',0,(char)0xce,(char)0xbb};
    switch (operation) {
    case 0: return spx_checked_string_from_literal_v1(ctx,text,4u,out);
    case 1: return spx_checked_string_clone_v1(ctx,left,out);
    case 2: return spx_checked_string_from_char_v1(ctx,0u,out);
    case 3: return spx_checked_string_from_i64_v1(ctx,INT64_MIN,out);
    case 4: return spx_checked_string_from_usize_v1(ctx,UINT64_MAX,out);
    case 5: return spx_checked_string_from_str_v1(ctx,text,4u,out);
    case 6: return spx_checked_string_from_utf8_v1(ctx,(const uint8_t *)text,4u,out);
    case 7: return spx_checked_string_concat_v1(ctx,left,right,out);
    case 8: return spx_checked_string_collection_clone_v1(ctx,left,out);
    default: abort();
    }
}
static void expected(unsigned operation, char *text) {
    static const char combined[]={'a',0,(char)0xce,(char)0xbb};
    const char *bytes=combined; uint64_t length=4u;
    if (operation==1u || operation==8u) length=2u;
    if (operation==2u) { bytes="";length=1u; }
    if (operation==3u) {bytes="-9223372036854775808";length=20u;}
    if (operation==4u) {bytes="18446744073709551615";length=20u;}
    assert(spx_checked_string_length_v1(text)==length);
    assert(memcmp(text,bytes,(size_t)length)==0);
    assert(text[length]=='\0');
}
static void inputs(struct spx_checked_string_context_v1 *ctx, char **left, char **right) {
    static const char a[]={'a',0};
    static const char b[]={(char)0xce,(char)0xbb};
    assert(spx_checked_string_from_literal_v1(ctx,a,2u,left));
    assert(spx_checked_string_from_literal_v1(ctx,b,2u,right));
}
static void direct_constructors(void) {
    struct observed_allocator observer={0};
    struct spx_checked_string_context_v1 ctx=context(&observer);
    for (unsigned operation=0;operation<9u;++operation) {
        reset(&ctx,0u);
        char *left=NULL,*right=NULL; inputs(&ctx,&left,&right);
        observer.fail_at=observer.attempts+1u;
        char *out=(char *)(uintptr_t)1u;
        assert(!make(operation,&ctx,left,right,&out));
        assert(out==(char *)(uintptr_t)1u && observer.live==2u && observer.attempts==3u);
        allocation_failure(&ctx,1u);
        /* Later overflow/conversion failure cannot replace physical refusal. */
        assert(!spx_checked_string_from_literal_v1(&ctx,NULL,UINT64_MAX,&out));
        assert(!spx_checked_string_from_char_v1(&ctx,0xd800u,&out));
        allocation_failure(&ctx,1u);
        assert(observer.attempts==3u && out==(char *)(uintptr_t)1u);
        spx_checked_string_drop_v1(&ctx,right);
        spx_checked_string_drop_v1(&ctx,left);
        assert(observer.live==0u && observer.drop_count==2u);
        assert(observer.dropped[0]==2u && observer.dropped[1]==1u);
        allocation_failure(&ctx,1u); /* cleanup never selects a new failure */
        reset(&ctx,0u); /* same allocator/context, new explicit invocation */
        inputs(&ctx,&left,&right);
        out=(char *)(uintptr_t)1u;
        assert(make(operation,&ctx,left,right,&out));
        assert(observer.attempts==3u && observer.live==3u && ctx.selected.code==0u);
        expected(operation,out);
        spx_checked_string_drop_v1(&ctx,out);
        spx_checked_string_drop_v1(&ctx,right);
        spx_checked_string_drop_v1(&ctx,left);
        assert(observer.live==0u && observer.drop_count==3u);
        assert(observer.dropped[0]==3u && observer.dropped[1]==2u && observer.dropped[2]==1u);
    }
}
static void partial_chain(void) {
    struct observed_allocator observer={0};
    struct spx_checked_string_context_v1 ctx=context(&observer);
    /* Every physical allocation in one mixed constructor chain, including the
     * otherwise hidden literal, clone and scalar collection-leaf clone. */
    for (size_t failure=1u;failure<=12u;++failure) {
        reset(&ctx,failure);
        char *owners[11]={0},*published=(char *)(uintptr_t)1u;
        size_t live=0u;
        if (spx_checked_string_from_literal_v1(&ctx,"a\0",2u,&owners[live])) ++live; else goto settle;
        if (spx_checked_string_from_char_v1(&ctx,0x3bbu,&owners[live])) ++live; else goto settle;
        for (unsigned operation=0;operation<9u;++operation) {
            if (make(operation,&ctx,owners[0],owners[1],&owners[live])) ++live; else goto settle;
        }
        published=owners[10];
settle:
        if (failure<=11u) {
            allocation_failure(&ctx,1u);
            assert(observer.attempts==failure && live==failure-1u);
            assert(published==(char *)(uintptr_t)1u);
        } else {
            assert(ctx.selected.code==0u && observer.attempts==11u && live==11u);
            assert(published==owners[10]);
        }
        size_t count=live;
        while (live != 0u) spx_checked_string_drop_v1(&ctx,owners[--live]);
        assert(observer.live==0u && observer.drop_count==count);
        for (size_t at=0u;at<count;++at) assert(observer.dropped[at]==count-at);
    }
}
static void invalid_and_overflow(void) {
    struct observed_allocator observer={0};
    struct spx_checked_string_context_v1 ctx=context(&observer);
    for (unsigned kind=0u;kind<3u;++kind) {
        reset(&ctx,0u); char *out=(char *)(uintptr_t)1u;
        bool success=kind==0u ? spx_checked_string_from_literal_v1(&ctx,NULL,UINT64_MAX,&out) :
            kind==1u ? spx_checked_string_from_str_v1(&ctx,NULL,UINT64_MAX,&out) :
            spx_checked_string_from_utf8_v1(&ctx,NULL,UINT64_MAX,&out);
        assert(!success && out==(char *)(uintptr_t)1u && observer.attempts==0u);
        allocation_failure(&ctx,2u);
    }
    static const uint8_t invalid[][4]={{0xc0,0x80,0,0},{0xed,0xa0,0x80,0},{0xf4,0x90,0x80,0x80},{0xf0,0x80,0x80,0x80},{0x80,0,0,0},{0xe2,0x82,0,0}};
    static const uint64_t lengths[]={2u,3u,4u,4u,1u,2u};
    for (size_t index=0u;index<6u;++index) {
        reset(&ctx,0u);char *out=(char *)(uintptr_t)1u;
        assert(!spx_checked_string_from_utf8_v1(&ctx,invalid[index],lengths[index],&out));
        assert(strcmp(ctx.selected.domain_id,"semaprax.convert.v1")==0 && ctx.selected.code==1u);
        assert(observer.attempts==0u && out==(char *)(uintptr_t)1u);
    }
    static const uint32_t scalars[]={0u,0x7fu,0x80u,0x7ffu,0x800u,0xffffu,0x10000u,0x10ffffu};
    static const uint64_t encoded_lengths[]={1u,1u,2u,2u,3u,3u,4u,4u};
    for (size_t index=0u;index<8u;++index) {
        reset(&ctx,0u);char *out=NULL;
        assert(spx_checked_string_from_char_v1(&ctx,scalars[index],&out));
        assert(spx_checked_string_length_v1(out)==encoded_lengths[index]);
        assert(spx_checked_string_utf8_v1((const uint8_t *)out,encoded_lengths[index]));
        spx_checked_string_drop_v1(&ctx,out);assert(observer.live==0u);
    }
    reset(&ctx,0u);char *out=NULL;
    assert(spx_checked_string_from_literal_v1(&ctx,NULL,0u,&out));
    assert(spx_checked_string_length_v1(out)==0u && out[0]=='\0' && observer.attempts==1u);
    spx_checked_string_drop_v1(&ctx,out);assert(observer.live==0u);
}
int main(void) {
    assert(fixture_binary_stdout());
    direct_constructors();partial_chain();invalid_and_overflow();
    puts("checked-string-kernel-v1: ok");
    return 0;
}
