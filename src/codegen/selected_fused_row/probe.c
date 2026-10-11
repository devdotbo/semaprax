#include <assert.h>
#include <stdlib.h>
typedef struct { size_t calls,frees,live,phase,fail_at; } observer;
static void *allocate(void *raw,size_t len) {
    observer *o=raw;assert(o->phase==2);o->calls++;
    if (o->calls==o->fail_at) return NULL;
    void *p=malloc(len);if(p)o->live++;return p;
}
static void release(void *raw,void *p,size_t len) {
    observer *o=raw;assert(len>0 && p && o->live);o->frees++;o->live--;free(p);
}
static spx_fused_context *fresh(void) {
    return calloc(1,sizeof(spx_fused_context));
}
static void maximum(void) {
    spx_fused_issuer issuer={0};assert(spx_fused_issuer_init(&issuer));
    spx_fused_bytes previous={0};
    for (unsigned round=0;round<3;round++) {
        observer o={0};
        /* Ordinary Vec storage precedes row operands. This is private bridge storage. */
        spx_fused_row *rows=calloc(SPX_FUSED_ROWS,sizeof(*rows));assert(rows);
        /* First String preparation precedes selected metadata/backing acquisition. */
        o.phase=1;void *string=malloc(4);assert(string);memcpy(string,"\xc3\xa9\0",4);
        spx_fused_context *ctx=fresh();assert(ctx);o.phase=2;
        assert(spx_fused_begin(ctx,&issuer,4096,3,sizeof(*ctx),&o,allocate,release));
        spx_fused_vec vec={rows,0,4096};
        for(size_t i=0;i<4096;i++) {
            if(i) {string=malloc(4);assert(string);memcpy(string,"\xc3\xa9\0",4);}
            spx_fused_row row={(int64_t)i,string,{NULL,0,0}};
            assert(spx_fused_issue(ctx,i,&row.bytes));
            if (!i && round) assert(!spx_fused_find(ctx,previous));
            size_t length=999;assert(spx_fused_len(ctx,row.bytes,&length));assert(length==3);
            assert(spx_fused_row_push(ctx,&vec,&row));
            assert(!row.string && !row.bytes.token);
        }
        assert(o.calls==1 && o.live==1 && ctx->events==1 && ctx->cumulative==12288);
        previous=rows[0].bytes;
        assert(spx_fused_complete(ctx));
        assert(spx_fused_row_swap(ctx,&vec,0,4095));
        assert(spx_fused_row_swap(ctx,&vec,0,4095));
        for(size_t i=0;i<4096;i++) {
            assert(rows[i].scalar==(int64_t)i && !memcmp(rows[i].string,"\xc3\xa9\0",4));
            spx_fused_view view={0};assert(spx_fused_borrow_begin(ctx,rows[i].bytes,&view));
            for(size_t j=0;j<3;j++){unsigned char byte=255;assert(spx_fused_read(ctx,view,j,&byte));assert(byte==0);}
            assert(spx_fused_borrow_end(ctx,&view));
            /* The owning compiler must supply canonical leaf order. Probe owns both fields. */
            free(rows[i].string);rows[i].string=NULL;
            assert(spx_fused_drop(ctx,&rows[i].bytes));
        }
        assert(o.calls==1 && o.frees==1 && !o.live && ctx->state==SPX_FUSED_SETTLED);
        assert(ctx->events==1 && ctx->cumulative==12288 && !issuer.active);
        assert(!spx_fused_begin(ctx,&issuer,1,3,sizeof(*ctx),&o,allocate,release));
        free(rows);free(ctx);
    }
}
static void controls(void) {
    spx_fused_issuer issuer={0},foreign={0};assert(spx_fused_issuer_init(&issuer));
    assert(spx_fused_issuer_init(&foreign));assert(!spx_fused_issuer_init(&issuer));
    observer o={0};o.phase=2;spx_fused_context *ctx=fresh();assert(ctx);
    assert(!spx_fused_begin(ctx,&issuer,4096,3,sizeof(*ctx)-1,&o,allocate,release));
    assert(!ctx->sealed && !issuer.active && !o.calls);
    assert(!spx_fused_begin(ctx,&issuer,4097,3,sizeof(*ctx),&o,allocate,release));
    assert(!spx_fused_begin(ctx,&issuer,4096,33,sizeof(*ctx),&o,allocate,release));
    assert(spx_fused_begin(ctx,&issuer,2,3,sizeof(*ctx),&o,allocate,release));
    spx_fused_bytes a={0},b={0},moved={0};
    assert(spx_fused_issue(ctx,0,&a));assert(spx_fused_publish(ctx,a));
    assert(spx_fused_issue(ctx,1,&b));assert(spx_fused_publish(ctx,b));
    unsigned char foreign_module=0;ctx->module=&foreign_module;
    assert(!spx_fused_find(ctx,a));ctx->module=&spx_fused_module_identity;
    ctx->slots[1].offset=0;assert(!spx_fused_find(ctx,b));ctx->slots[1].offset=3;
    spx_fused_context *overlap=fresh();assert(overlap);
    assert(!spx_fused_begin(overlap,&issuer,1,3,sizeof(*overlap),&o,allocate,release));
    assert(!overlap->sealed);free(overlap);
    spx_fused_bytes stale=a;assert(spx_fused_move(ctx,&a,&moved));assert(!a.token);
    assert(!spx_fused_find(ctx,stale));
    spx_fused_bytes wrong=moved;wrong.issuer=&foreign;assert(!spx_fused_find(ctx,wrong));
    wrong=moved;wrong.slot=SPX_FUSED_ROWS;assert(!spx_fused_find(ctx,wrong));
    wrong=moved;wrong.token=UINT64_MAX;assert(!spx_fused_find(ctx,wrong));
    assert(spx_fused_complete(ctx));
    spx_fused_view first={0},second={0};
    assert(spx_fused_borrow_begin(ctx,moved,&first));
    assert(spx_fused_borrow_begin(ctx,moved,&second));
    spx_fused_view old=first;assert(spx_fused_borrow_end(ctx,&first));
    assert(spx_fused_borrow_begin(ctx,moved,&first));assert(first.token!=old.token);
    assert(!spx_fused_view_find(ctx,old));
    spx_fused_view fake=first;fake.len++;assert(!spx_fused_view_find(ctx,fake));
    fake=first;fake.ptr++;assert(!spx_fused_view_find(ctx,fake));
    assert(!spx_fused_move(ctx,&moved,&a));assert(ctx->failure==1 && moved.token);
    assert(!spx_fused_drop(ctx,&moved));assert(moved.token);
    unsigned char unchanged=199;assert(!spx_fused_read(ctx,first,0,&unchanged));assert(unchanged==199);
    assert(spx_fused_borrow_end(ctx,&first));assert(spx_fused_borrow_end(ctx,&second));
    spx_fused_bytes untouched={&foreign,999,999};
    assert(!spx_fused_clone_closed(ctx,moved,&untouched));assert(untouched.token==999);
    assert(spx_fused_drop(ctx,&moved));assert(spx_fused_drop(ctx,&b));
    assert(!o.live && o.frees==1);
    assert(ctx->failure==1 && !o.live && o.frees==1);free(ctx);
    /* Backing failure leaves the result untouched and no published lease. */
    o=(observer){0};o.phase=2;o.fail_at=1;ctx=fresh();assert(ctx);
    assert(spx_fused_begin(ctx,&issuer,8,3,sizeof(*ctx),&o,allocate,release));
    assert(!spx_fused_issue(ctx,0,&untouched));assert(untouched.token==999);
    assert(ctx->failure==1 && !ctx->issued && !ctx->live && o.calls==1 && !o.live);
    assert(ctx->events==1 && ctx->cumulative==24);assert(spx_fused_abort(ctx));assert(spx_fused_settle_abort(ctx));free(ctx);
    /* Partial source/push failure: earlier rows and staged row settle before pin release. */
    for(size_t cut=1;cut<=8;cut++) {
        o=(observer){0};o.phase=2;ctx=fresh();assert(ctx);
        assert(spx_fused_begin(ctx,&issuer,8,3,sizeof(*ctx),&o,allocate,release));
        spx_fused_bytes values[8]={{0}};
        for(size_t i=0;i<cut;i++){assert(spx_fused_issue(ctx,i,&values[i]));if(i+1<cut)assert(spx_fused_publish(ctx,values[i]));}
        assert(!spx_fused_fail(ctx)); /* same false result; selected first failure remains sticky */
        assert(spx_fused_abort(ctx));assert(!spx_fused_settle_abort(ctx));
        for(size_t i=cut;i>0;i--)assert(spx_fused_drop(ctx,&values[i-1]));
        assert(o.live==1);assert(spx_fused_settle_abort(ctx));
        assert(!o.live && o.frees==1 && ctx->failure==1);free(ctx);
    }
    /* Consuming sort cannot move a live borrowed row or mutate after refusal. */
    o=(observer){0};o.phase=2;ctx=fresh();assert(ctx);
    assert(spx_fused_begin(ctx,&issuer,2,3,sizeof(*ctx),&o,allocate,release));
    spx_fused_row rows[2]={{0}};spx_fused_vec vec={rows,0,2};
    for(size_t i=0;i<2;i++) {
        spx_fused_row row={(int64_t)i,NULL,{0}};
        assert(spx_fused_issue(ctx,i,&row.bytes));assert(spx_fused_row_push(ctx,&vec,&row));
    }
    assert(spx_fused_complete(ctx));spx_fused_view held={0};
    assert(spx_fused_borrow_begin(ctx,rows[0].bytes,&held));
    uint64_t first_token=rows[0].bytes.token,second_token=rows[1].bytes.token;
    assert(!spx_fused_row_swap(ctx,&vec,0,1));
    assert(rows[0].scalar==0 && rows[1].scalar==1 && rows[0].bytes.token==first_token && rows[1].bytes.token==second_token);
    assert(spx_fused_borrow_end(ctx,&held));assert(!spx_fused_row_swap(ctx,&vec,0,1));
    assert(rows[0].scalar==0 && rows[1].scalar==1 && ctx->failure==1);
    assert(spx_fused_drop(ctx,&rows[1].bytes));assert(spx_fused_drop(ctx,&rows[0].bytes));
    assert(!o.live && o.frees==1);free(ctx);
    /* Publication and phase refusals preserve every caller output. */
    for(unsigned mode=0;mode<3;mode++) {
        o=(observer){0};o.phase=2;ctx=fresh();assert(ctx);
        assert(spx_fused_begin(ctx,&issuer,2,3,sizeof(*ctx),&o,allocate,release));
        spx_fused_bytes child={0};assert(spx_fused_issue(ctx,0,&child));
        if(!mode) {
            spx_fused_bytes output={&foreign,999,999};
            assert(!spx_fused_issue(ctx,1,&output));assert(output.token==999);
        } else if(mode==1) {
            spx_fused_view output={&foreign,999,999,999,NULL,999};
            assert(!spx_fused_borrow_begin(ctx,child,&output));assert(output.token==999);
        } else {
            uint64_t token=child.token;assert(!spx_fused_drop(ctx,&child));assert(child.token==token);
        }
        assert(ctx->failure && ctx->issued==1);assert(spx_fused_abort(ctx));
        assert(!spx_fused_settle_abort(ctx));assert(spx_fused_drop(ctx,&child));
        assert(o.live==1);assert(spx_fused_settle_abort(ctx));assert(!o.live && o.frees==1);free(ctx);
    }
    /* Issuer exhaustion refuses without wrapping; no fabricated allocation status. */
    issuer.next=UINT64_MAX;ctx=fresh();assert(ctx);
    assert(!spx_fused_begin(ctx,&issuer,1,3,sizeof(*ctx),&o,allocate,release));
    assert(!ctx->sealed && issuer.next==UINT64_MAX);free(ctx);
}
int main(void){maximum();controls();return 0;}
