/* Private precursor only: no public ABI and no emitted source selector. */
#include <stddef.h>
#include <stdint.h>
#include <limits.h>
#include <string.h>
#define SPX_FUSED_ROWS 4096u
#define SPX_FUSED_VIEWS 64u
#define SPX_FUSED_EXTENT 131072u
#define SPX_FUSED_EVENTS 32u
#define SPX_FUSED_CUMULATIVE 2097152u
static unsigned char spx_fused_module_identity;
typedef struct spx_fused_issuer spx_fused_issuer;
typedef struct { const spx_fused_issuer *issuer; size_t slot; uint64_t token; } spx_fused_bytes;
typedef struct { const spx_fused_issuer *issuer; size_t slot; uint64_t token, owner; const unsigned char *ptr; size_t len; } spx_fused_view;
typedef struct { uint64_t token; size_t offset, len; unsigned live, borrows, published; } spx_fused_slot;
typedef struct { uint64_t token, owner; unsigned live; } spx_fused_borrow;
struct spx_fused_context;
struct spx_fused_issuer {
    const void *module; uint64_t next; struct spx_fused_context *active; unsigned sealed;
};
typedef void *(*spx_fused_alloc)(void *, size_t);
typedef void (*spx_fused_free)(void *, void *, size_t);
typedef struct spx_fused_context {
    const void *module; spx_fused_issuer *issuer; uint64_t invocation;
    unsigned sealed, state, failure, pin;
    size_t count, width, extent, issued, live, events, cumulative;
    unsigned char *backing; void *allocation_context;
    spx_fused_alloc allocate; spx_fused_free release;
    spx_fused_slot slots[SPX_FUSED_ROWS];
    spx_fused_borrow views[SPX_FUSED_VIEWS];
} spx_fused_context;
#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(spx_fused_bytes)==24,"native64 Bytes carrier");
_Static_assert(sizeof(spx_fused_view)==48,"native64 View carrier");
_Static_assert(sizeof(spx_fused_slot)==40,"native64 slot layout");
_Static_assert(sizeof(spx_fused_borrow)==24,"native64 borrow layout");
_Static_assert(sizeof(spx_fused_issuer)==32,"native64 issuer layout");
_Static_assert(sizeof(spx_fused_context)==165504,"native64 context layout");
#endif
/* Private states, not assigned language statuses. */
enum { SPX_FUSED_UNSTARTED, SPX_FUSED_BUILDING, SPX_FUSED_COMPLETE,
       SPX_FUSED_ABORTED, SPX_FUSED_SETTLED };
static int spx_fused_issuer_init(spx_fused_issuer *issuer) {
    if (!issuer || issuer->sealed) return 0;
    issuer->module=&spx_fused_module_identity; issuer->next=1;
    issuer->active=NULL; issuer->sealed=1; return 1;
}
static int spx_fused_mint(spx_fused_issuer *issuer,uint64_t *out) {
    if (!issuer || !issuer->sealed || issuer->module!=&spx_fused_module_identity ||
        !issuer->next || issuer->next==UINT64_MAX) return 0;
    *out=issuer->next++; return 1;
}
static int spx_fused_valid(const spx_fused_context *ctx) {
    return ctx && ctx->sealed && ctx->module==&spx_fused_module_identity &&
        ctx->issuer && ctx->issuer->sealed && ctx->issuer->module==ctx->module &&
        ctx->issuer->active==ctx && ctx->invocation &&
        ctx->state>=SPX_FUSED_BUILDING && ctx->state<=SPX_FUSED_ABORTED &&
        ctx->count && ctx->count<=SPX_FUSED_ROWS && ctx->width &&
        ctx->width<=SPX_FUSED_EXTENT/ctx->count && ctx->extent==ctx->count*ctx->width &&
        ctx->issued<=ctx->count && ctx->live<=ctx->issued &&
        ctx->events<=SPX_FUSED_EVENTS && ctx->cumulative<=SPX_FUSED_CUMULATIVE;
}
static int spx_fused_fail(spx_fused_context *ctx) {
    if (ctx && ctx->sealed && !ctx->failure) ctx->failure=1;
    return 0;
}
static int spx_fused_begin(spx_fused_context *ctx,spx_fused_issuer *issuer,
 size_t count,size_t width,size_t metadata_bytes,void *allocation_context,
 spx_fused_alloc allocate,spx_fused_free release) {
    uint64_t invocation;
    if (!ctx || ctx->sealed || !issuer || issuer->active || !allocate || !release ||
        metadata_bytes<sizeof(*ctx) || !count || count>SPX_FUSED_ROWS || !width ||
        width>SPX_FUSED_EXTENT/count || !spx_fused_mint(issuer,&invocation)) return 0;
    memset(ctx,0,sizeof(*ctx));
    ctx->module=&spx_fused_module_identity;ctx->issuer=issuer;ctx->invocation=invocation;
    ctx->sealed=1;ctx->state=SPX_FUSED_BUILDING;ctx->pin=1;
    ctx->count=count;ctx->width=width;ctx->extent=count*width;
    ctx->allocation_context=allocation_context;ctx->allocate=allocate;ctx->release=release;
    issuer->active=ctx; return 1;
}
static spx_fused_slot *spx_fused_find(spx_fused_context *ctx,spx_fused_bytes value) {
    if (!spx_fused_valid(ctx) || !value.token || value.issuer!=ctx->issuer) return NULL;
    if (value.slot>=ctx->issued) return NULL;
    size_t i=value.slot;spx_fused_slot *slot=&ctx->slots[i];
    if (slot->live && slot->token==value.token &&
        i<ctx->count && slot->offset==i*ctx->width && slot->len==ctx->width &&
        slot->offset<=ctx->extent && slot->len<=ctx->extent-slot->offset) return slot;
    return NULL;
}
static void spx_fused_settle(spx_fused_context *ctx) {
    if (!ctx->pin && !ctx->live) {
        if (ctx->backing) ctx->release(ctx->allocation_context,ctx->backing,ctx->extent);
        ctx->backing=NULL;ctx->state=SPX_FUSED_SETTLED;ctx->issuer->active=NULL;
    }
}
/* Called at the original Bytes operand, after the preceding row String. */
static int spx_fused_issue(spx_fused_context *ctx,size_t ordinal,spx_fused_bytes *out) {
    uint64_t token;
    if (!out || !spx_fused_valid(ctx) || ctx->failure || ctx->state!=SPX_FUSED_BUILDING ||
        ordinal!=ctx->issued || ordinal>=ctx->count ||
        (ctx->issued && !ctx->slots[ctx->issued-1].published) || !spx_fused_mint(ctx->issuer,&token))
        return spx_fused_fail(ctx);
    if (!ctx->backing) {
        if (ctx->events>=SPX_FUSED_EVENTS || ctx->cumulative>SPX_FUSED_CUMULATIVE ||
            ctx->extent>SPX_FUSED_CUMULATIVE-ctx->cumulative)
            return spx_fused_fail(ctx);
        /* Debit before physical allocation; cleanup never refunds the debit. */
        ctx->events++;ctx->cumulative+=ctx->extent;
        unsigned char *backing=ctx->allocate(ctx->allocation_context,ctx->extent);
        if (!backing) return spx_fused_fail(ctx);
        memset(backing,0,ctx->extent);ctx->backing=backing;
    }
    spx_fused_slot *slot=&ctx->slots[ordinal];
    slot->token=token;slot->offset=ordinal*ctx->width;slot->len=ctx->width;
    slot->live=1;slot->borrows=0;ctx->issued++;ctx->live++;
    *out=(spx_fused_bytes){ctx->issuer,ordinal,token};return 1;
}
static int spx_fused_len(spx_fused_context *ctx,spx_fused_bytes value,size_t *out) {
    spx_fused_slot *slot=spx_fused_find(ctx,value);
    if (!slot || !out || ctx->failure) return spx_fused_fail(ctx);
    *out=slot->len;return 1;
}
static int spx_fused_move(spx_fused_context *ctx,spx_fused_bytes *source,spx_fused_bytes *out) {
    if (!source || !out || source==out) return spx_fused_fail(ctx);
    spx_fused_slot *slot=spx_fused_find(ctx,*source);uint64_t token;
    if (!slot || slot->borrows || ctx->failure || ctx->state==SPX_FUSED_ABORTED || !spx_fused_mint(ctx->issuer,&token))
        return spx_fused_fail(ctx);
    slot->token=token;*out=(spx_fused_bytes){ctx->issuer,source->slot,token};
    *source=(spx_fused_bytes){NULL,0,0};return 1;
}
static int spx_fused_drop(spx_fused_context *ctx,spx_fused_bytes *value) {
    if (!value) return 0;
    spx_fused_slot *slot=spx_fused_find(ctx,*value);
    if (!slot || slot->borrows ||
        (ctx->state!=SPX_FUSED_COMPLETE && ctx->state!=SPX_FUSED_ABORTED))
        return spx_fused_fail(ctx);
    slot->live=0;slot->token=0;ctx->live--;*value=(spx_fused_bytes){NULL,0,0};
    spx_fused_settle(ctx);return 1;
}
static int spx_fused_borrow_begin(spx_fused_context *ctx,spx_fused_bytes owner,spx_fused_view *out) {
    spx_fused_slot *slot=spx_fused_find(ctx,owner);uint64_t token;
    if (!out || !slot || ctx->failure || ctx->state!=SPX_FUSED_COMPLETE) return spx_fused_fail(ctx);
    for (size_t i=0;i<SPX_FUSED_VIEWS;i++) if (!ctx->views[i].live) {
        if (!spx_fused_mint(ctx->issuer,&token)) return spx_fused_fail(ctx);
        ctx->views[i]=(spx_fused_borrow){token,owner.token,1};slot->borrows++;
        *out=(spx_fused_view){ctx->issuer,owner.slot,token,owner.token,ctx->backing+slot->offset,slot->len};return 1;
    }
    return spx_fused_fail(ctx);
}
static spx_fused_borrow *spx_fused_view_find(spx_fused_context *ctx,spx_fused_view view) {
    spx_fused_slot *slot=spx_fused_find(ctx,(spx_fused_bytes){view.issuer,view.slot,view.owner});
    if (!slot || !slot->borrows || view.len!=slot->len || view.ptr!=ctx->backing+slot->offset) return NULL;
    for (size_t i=0;i<SPX_FUSED_VIEWS;i++) if (ctx->views[i].live &&
        ctx->views[i].token==view.token && ctx->views[i].owner==view.owner) return &ctx->views[i];
    return NULL;
}
static int spx_fused_read(spx_fused_context *ctx,spx_fused_view view,size_t index,unsigned char *out) {
    if (!out || !spx_fused_valid(ctx) || ctx->failure ||
        !spx_fused_view_find(ctx,view) || index>=view.len) return spx_fused_fail(ctx);
    *out=view.ptr[index];return 1;
}
static int spx_fused_borrow_end(spx_fused_context *ctx,spx_fused_view *view) {
    if (!view) return 0;
    spx_fused_borrow *borrow=spx_fused_view_find(ctx,*view);
    if (!borrow) return spx_fused_fail(ctx);
    spx_fused_slot *slot=spx_fused_find(ctx,(spx_fused_bytes){view->issuer,view->slot,view->owner});
    borrow->live=0;borrow->token=0;slot->borrows--;*view=(spx_fused_view){NULL,0,0,0,NULL,0};return 1;
}
static int spx_fused_complete(spx_fused_context *ctx) {
    if (!spx_fused_valid(ctx) || ctx->failure || ctx->state!=SPX_FUSED_BUILDING ||
        ctx->issued!=ctx->count || !ctx->slots[ctx->issued-1].published) return spx_fused_fail(ctx);
    ctx->state=SPX_FUSED_COMPLETE;ctx->pin=0;spx_fused_settle(ctx);return 1;
}
/* Caller first executes canonical cleanup of every staged/published owner. */
static int spx_fused_abort(spx_fused_context *ctx) {
    if (!spx_fused_valid(ctx) || ctx->state!=SPX_FUSED_BUILDING) return 0;
    ctx->state=SPX_FUSED_ABORTED;return 1;
}
static int spx_fused_settle_abort(spx_fused_context *ctx) {
    if (!spx_fused_valid(ctx) || ctx->state!=SPX_FUSED_ABORTED || ctx->live) return 0;
    ctx->pin=0;spx_fused_settle(ctx);return 1;
}
/* The real compiler binds publication to its exact atomic record/Vec commit. */
static int spx_fused_publish(spx_fused_context *ctx,spx_fused_bytes value) {
    spx_fused_slot *slot=spx_fused_find(ctx,value);
    if (!slot || ctx->failure || ctx->state!=SPX_FUSED_BUILDING || slot->published ||
        value.slot+1!=ctx->issued || slot->borrows) return spx_fused_fail(ctx);
    slot->published=1;return 1;
}
/* Source clones/partition/public pointer routes are outside this precursor. */
static int spx_fused_clone_closed(spx_fused_context *ctx,spx_fused_bytes value,spx_fused_bytes *out) {
    (void)value;(void)out;return spx_fused_fail(ctx);
}
/* Owning Vec storage is supplied by its ordinary allocation, never by a hidden arena. */
typedef struct { int64_t scalar; void *string; spx_fused_bytes bytes; } spx_fused_row;
typedef struct { spx_fused_row *rows; size_t len,capacity; } spx_fused_vec;
#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(spx_fused_row)==40,"native64 row layout");
#endif
static int spx_fused_row_push(spx_fused_context *ctx,spx_fused_vec *vec,spx_fused_row *row) {
    if (!spx_fused_valid(ctx) || ctx->failure || ctx->state!=SPX_FUSED_BUILDING ||
        !vec || !row || !vec->rows || vec->capacity!=ctx->count || vec->len>=vec->capacity)
        return spx_fused_fail(ctx);
    uintptr_t address=(uintptr_t)row,base=(uintptr_t)vec->rows;
    size_t bytes=vec->capacity*sizeof(*vec->rows);
    if (address>=base && address-base<bytes) return spx_fused_fail(ctx);
    spx_fused_slot *slot=spx_fused_find(ctx,row->bytes);
    if (!slot || slot->published || row->bytes.slot!=vec->len || slot->borrows ||
        row->bytes.slot+1!=ctx->issued) return spx_fused_fail(ctx);
    spx_fused_bytes moved;
    if (!spx_fused_move(ctx,&row->bytes,&moved)) return 0;
    slot->published=1;
    vec->rows[vec->len++]=(spx_fused_row){row->scalar,row->string,moved};
    row->string=NULL;return 1;
}
static int spx_fused_row_swap(spx_fused_context *ctx,spx_fused_vec *vec,size_t a,size_t b) {
    if (!spx_fused_valid(ctx) || ctx->failure || ctx->state!=SPX_FUSED_COMPLETE ||
        !vec || !vec->rows || vec->capacity!=ctx->count || vec->len!=ctx->count ||
        a>=vec->len || b>=vec->len) return spx_fused_fail(ctx);
    spx_fused_slot *first=spx_fused_find(ctx,vec->rows[a].bytes);
    spx_fused_slot *second=spx_fused_find(ctx,vec->rows[b].bytes);
    if (!first || !second || first->borrows || second->borrows) return spx_fused_fail(ctx);
    spx_fused_row temporary=vec->rows[a];vec->rows[a]=vec->rows[b];vec->rows[b]=temporary;
    return 1;
}
