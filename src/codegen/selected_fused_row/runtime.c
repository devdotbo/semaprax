/* Private selected FusedConstructionV1 native runtime. No public ABI.
   Owner carriers are issuer/slot/generation/nonce records; none converts to the
   frozen 16-byte public Bytes ABI and no interior range reaches the ordinary
   tag-10 free(ptr) drop.

   Funding: invocation admission, before any program effect, debits the held
   metadata ledger for the complete heap frame (domain header plus private
   carriers) and the exact lease/view tables, holds future row allocation bytes
   on that same ledger, and holds one ordinary backing
   event and its extent in the held payload ledger. Only then is the frame
   allocated. The row hold is debited before calloc at the original Vec boundary.
   Its embedded receipt binds the actual pointer and exact funding account.
   The tables are allocated, and the payload hold is consumed, at
   the first original Bytes operand. Nothing is refunded; an unconsumed hold
   (no Bytes operand ever ran) is released because it was never debited. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#ifndef SPX_FUSED_FAIL_STOP
#define SPX_FUSED_FAIL_STOP(message) abort()
#endif
#define SPX_FUSED_FN static __attribute__((unused))
#define SPX_FUSED_ROWS 4096u
#define SPX_FUSED_VIEWS 64u
#define SPX_FUSED_EXTENT 131072u
#define SPX_FUSED_EVENTS 32u
#define SPX_FUSED_CUMULATIVE 2097152u
#define SPX_FUSED_FRAME_LIMIT 65536u
/* Field roles, in the order the selected record declares them. */
enum { SPX_FUSED_SCALAR, SPX_FUSED_STRING, SPX_FUSED_PAYLOAD };
/* Private states, not assigned language statuses. */
enum { SPX_FUSED_ZERO, SPX_FUSED_PREPARED, SPX_FUSED_BUILDING, SPX_FUSED_COMPLETE,
       SPX_FUSED_ABORTED, SPX_FUSED_SETTLED, SPX_FUSED_FINISHED };
static unsigned char spx_fused_module_identity;
struct spx_fused_domain;
/* Caller-held and never reset: nonces are strictly increasing and never wrap. */
typedef struct spx_fused_issuer {
    const void *module; uint64_t next; const struct spx_fused_domain *active; uint32_t sealed;
} spx_fused_issuer;
/* Separately held real metadata account. Debits are monotonic; there is no
   refund operation and the first refusal is sticky for the account. */
typedef struct { size_t limit, debited, held_rows; uint32_t sealed, failed; } spx_fused_ledger;
/* The one ordinary backing-event/payload account (32 events, 2 MiB). Holds
   become debits only by consumption; debits are never refunded or reset. */
typedef struct { size_t events, cumulative, held_events, held_bytes; uint32_t sealed, failed; } spx_fused_payload;
typedef struct { const spx_fused_issuer *issuer; uint32_t slot, generation; uint64_t nonce; } spx_fused_bytes;
/* Compact stored owner: slot, generation and the exact 64-bit nonce. Its
   domain authenticator is the row Vec header's held domain, issuer and
   invocation nonce, all checked before any stored owner is resolved. */
typedef struct { uint32_t slot, generation; uint64_t nonce; } spx_fused_stored;
typedef struct {
    const spx_fused_issuer *issuer; uint32_t slot, generation; uint64_t nonce, owner;
    const unsigned char *ptr; size_t len;
} spx_fused_view;
typedef struct { uint64_t nonce; uint32_t generation, live, borrows, published; } spx_fused_slot;
typedef struct { uint64_t nonce, owner; uint32_t slot, live; } spx_fused_borrow;
typedef struct { int64_t scalar; char *string; spx_fused_stored bytes; } spx_fused_row;
typedef struct { int64_t scalar; char *string; spx_fused_bytes bytes; } spx_fused_staged;
typedef struct {
    spx_fused_row *rows; uint64_t len, capacity, generation;
    struct spx_fused_domain *domain; const spx_fused_issuer *issuer; uint64_t invocation;
} spx_fused_rows;
/* Fresh ordinary owner produced by a charged deep clone; never an interior range. */
typedef struct { unsigned char *ptr; uint64_t len; } spx_fused_copy;
typedef void (*spx_fused_string_drop)(char *);
typedef uint64_t (*spx_fused_string_len)(const char *);
/* Address-bound private destination funding receipt, embedded in its domain.
   A copied carrier is never a receipt. Geometry and ledger/issuer/nonce are
   received again at allocation and activation; debit has no refund path. */
typedef struct {
    struct spx_fused_domain *domain; const spx_fused_issuer *issuer; spx_fused_ledger *ledger;
    uint64_t invocation; spx_fused_row *pointer; size_t capacity, stride, bytes;
    uint32_t funded, allocated, settled;
} spx_fused_row_receipt;
typedef struct spx_fused_domain {
    const void *module; spx_fused_issuer *issuer; spx_fused_ledger *ledger; spx_fused_payload *payload;
    uint64_t invocation, vec_generation;
    uint32_t sealed, state, failure, pin, vec_live, activated, held, consumed;
    size_t count, width, extent, metadata, issued, live, loans;
    spx_fused_slot *slots; spx_fused_borrow *views; unsigned char *backing;
    spx_fused_row_receipt rows_receipt;
} spx_fused_domain;
#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(spx_fused_bytes)==24,"native64 private owner carrier");
_Static_assert(sizeof(spx_fused_stored)==16,"native64 compact stored owner");
_Static_assert(sizeof(spx_fused_view)==48,"native64 private view carrier");
_Static_assert(sizeof(spx_fused_slot)==24,"native64 lease slot");
_Static_assert(sizeof(spx_fused_borrow)==24,"native64 view slot");
_Static_assert(sizeof(spx_fused_row)==32,"native64 row equals the ordinary leaf stride");
_Static_assert(sizeof(spx_fused_staged)==40,"native64 staged row");
_Static_assert(sizeof(spx_fused_rows)==56,"native64 row Vec header");
_Static_assert(sizeof(spx_fused_issuer)==32,"native64 issuer");
_Static_assert(sizeof(spx_fused_ledger)==32,"native64 metadata ledger");
_Static_assert(sizeof(spx_fused_payload)==40,"native64 payload ledger");
_Static_assert(sizeof(spx_fused_domain)==240,"native64 domain header");
_Static_assert(sizeof(spx_fused_row_receipt)==80,"native64 held destination receipt");
#endif
SPX_FUSED_FN int spx_fused_issuer_init(spx_fused_issuer *issuer) {
    if (!issuer || issuer->sealed) return 0;
    issuer->module=&spx_fused_module_identity;issuer->next=1;issuer->active=NULL;issuer->sealed=1;
    return 1;
}
SPX_FUSED_FN int spx_fused_ledger_init(spx_fused_ledger *ledger,size_t limit) {
    if (!ledger || ledger->sealed) return 0;
    ledger->limit=limit;ledger->debited=0;ledger->held_rows=0;ledger->failed=0;ledger->sealed=1;return 1;
}
/* All-or-nothing debit before any physical allocation it covers. */
SPX_FUSED_FN int spx_fused_ledger_debit(spx_fused_ledger *ledger,size_t bytes) {
    if (!ledger || !ledger->sealed || ledger->failed) return 0;
    if (ledger->debited>ledger->limit || ledger->held_rows>ledger->limit-ledger->debited ||
        bytes>ledger->limit-ledger->debited-ledger->held_rows) {
        ledger->failed=1;return 0;
    }
    ledger->debited+=bytes;return 1;
}
SPX_FUSED_FN int spx_fused_rows_hold(spx_fused_ledger *ledger,size_t bytes) {
    if (!ledger || !ledger->sealed || ledger->failed || ledger->debited>ledger->limit ||
        ledger->held_rows>ledger->limit-ledger->debited || bytes>ledger->limit-ledger->debited-ledger->held_rows) {
        if (ledger && ledger->sealed) { ledger->failed=1; }
        return 0;
    }
    ledger->held_rows+=bytes;return 1;
}
SPX_FUSED_FN int spx_fused_rows_consume(spx_fused_ledger *ledger,size_t bytes) {
    if (!ledger || !ledger->sealed || ledger->failed || ledger->held_rows<bytes ||
        ledger->debited>ledger->limit || bytes>ledger->limit-ledger->debited) return 0;
    ledger->held_rows-=bytes;ledger->debited+=bytes;return 1;
}
SPX_FUSED_FN int spx_fused_payload_init(spx_fused_payload *payload) {
    if (!payload || payload->sealed) return 0;
    *payload=(spx_fused_payload){0,0,0,0,1,0};return 1;
}
SPX_FUSED_FN int spx_fused_payload_room(const spx_fused_payload *p,size_t events,size_t bytes) {
    if (!p || !p->sealed || p->failed || p->events>SPX_FUSED_EVENTS ||
        p->held_events>SPX_FUSED_EVENTS-p->events || p->cumulative>SPX_FUSED_CUMULATIVE ||
        p->held_bytes>SPX_FUSED_CUMULATIVE-p->cumulative) return 0;
    size_t used_events=p->events+p->held_events,used_bytes=p->cumulative+p->held_bytes;
    return events<=SPX_FUSED_EVENTS-used_events && bytes<=SPX_FUSED_CUMULATIVE-used_bytes;
}
/* Checked direct debit (clone). Refusal is sticky for the account. */
SPX_FUSED_FN int spx_fused_payload_debit(spx_fused_payload *p,size_t events,size_t bytes) {
    if (!spx_fused_payload_room(p,events,bytes)) { if (p && p->sealed) p->failed=1; return 0; }
    p->events+=events;p->cumulative+=bytes;return 1;
}
SPX_FUSED_FN int spx_fused_payload_hold(spx_fused_payload *p,size_t events,size_t bytes) {
    if (!spx_fused_payload_room(p,events,bytes)) { if (p && p->sealed) p->failed=1; return 0; }
    p->held_events+=events;p->held_bytes+=bytes;return 1;
}
/* Consumption converts an exact admitted hold into a debit; it cannot succeed
   without that hold. */
SPX_FUSED_FN int spx_fused_payload_consume(spx_fused_payload *p,size_t events,size_t bytes) {
    if (!p || !p->sealed || p->held_events<events || p->held_bytes<bytes) return 0;
    p->held_events-=events;p->held_bytes-=bytes;p->events+=events;p->cumulative+=bytes;return 1;
}
SPX_FUSED_FN int spx_fused_payload_release(spx_fused_payload *p,size_t events,size_t bytes) {
    if (!p || !p->sealed || p->held_events<events || p->held_bytes<bytes) return 0;
    p->held_events-=events;p->held_bytes-=bytes;return 1;
}
SPX_FUSED_FN int spx_fused_mint(spx_fused_issuer *issuer,uint64_t *out) {
    if (!issuer || !out || !issuer->sealed || issuer->module!=&spx_fused_module_identity ||
        !issuer->next || issuer->next==UINT64_MAX) return 0;
    *out=issuer->next++;return 1;
}
SPX_FUSED_FN int spx_fused_fail(spx_fused_domain *d) {
    if (d && d->sealed && !d->failure) d->failure=1;
    return 0;
}
/* Exact metadata from the target's actual sizeof: the frame (header plus
   private carriers) and the lease and view tables. */
SPX_FUSED_FN size_t spx_fused_metadata_bytes(size_t count,size_t frame) {
    if (!count || count>SPX_FUSED_ROWS || frame<sizeof(spx_fused_domain) || frame>SPX_FUSED_FRAME_LIMIT)
        return 0;
    return frame+count*sizeof(spx_fused_slot)+SPX_FUSED_VIEWS*sizeof(spx_fused_borrow);
}
/* Invocation admission, before any program effect. Every refusal leaves the
   issuer, both ledgers' debits and the allocator untouched except the sticky
   refusal mark of the account that refused. Returns the zeroed frame. */
SPX_FUSED_FN size_t spx_fused_invocation_bytes(size_t count,size_t frame) {
    size_t metadata=spx_fused_metadata_bytes(count,frame);
    if (!metadata || count>SIZE_MAX/sizeof(spx_fused_row) || count*sizeof(spx_fused_row)>SIZE_MAX-metadata) return 0;
    return metadata+count*sizeof(spx_fused_row);
}
SPX_FUSED_FN void *spx_fused_admit(spx_fused_issuer *issuer,spx_fused_ledger *ledger,
 spx_fused_payload *payload,size_t count,size_t width,size_t frame) {
    size_t metadata=spx_fused_metadata_bytes(count,frame);
    if (!issuer || !ledger || !payload || !metadata || !width || width>SPX_FUSED_EXTENT/count ||
        !issuer->sealed || issuer->module!=&spx_fused_module_identity || issuer->active ||
        !issuer->next || issuer->next>=UINT64_MAX-(uint64_t)count-2u) return NULL;
    if (!spx_fused_payload_room(payload,1,count*width)) { if (payload->sealed) payload->failed=1; return NULL; }
    size_t row_bytes=count*sizeof(spx_fused_row),total=spx_fused_invocation_bytes(count,frame);
    if (!total || !ledger->sealed || ledger->failed || ledger->debited>ledger->limit ||
        ledger->held_rows>ledger->limit-ledger->debited || total>ledger->limit-ledger->debited-ledger->held_rows) {
        if (ledger->sealed) { ledger->failed=1; }
        return NULL;
    }
    if (!spx_fused_ledger_debit(ledger,metadata)) SPX_FUSED_FAIL_STOP("fused metadata debit drift");
    if (!spx_fused_rows_hold(ledger,row_bytes)) SPX_FUSED_FAIL_STOP("fused row hold drift");
    if (!spx_fused_payload_hold(payload,1,count*width)) SPX_FUSED_FAIL_STOP("fused payload hold drift");
    void *storage=calloc(1,frame);
    if (!storage) SPX_FUSED_FAIL_STOP("fused frame allocation failed");
    return storage;
}
/* Binds the domain header at offset zero of an admitted frame. */
SPX_FUSED_FN void spx_fused_bind(spx_fused_domain *d,spx_fused_issuer *issuer,spx_fused_ledger *ledger,
 spx_fused_payload *payload,size_t count,size_t width,size_t frame) {
    static const spx_fused_domain zero;
    if (!d || memcmp(d,&zero,sizeof(*d)) || issuer->active || !spx_fused_mint(issuer,&d->invocation))
        SPX_FUSED_FAIL_STOP("fused domain bind without admission");
    d->module=&spx_fused_module_identity;d->issuer=issuer;d->ledger=ledger;d->payload=payload;
    d->sealed=1;d->state=SPX_FUSED_PREPARED;d->pin=1;d->held=1;
    d->count=count;d->width=width;d->extent=count*width;d->metadata=spx_fused_metadata_bytes(count,frame);
    issuer->active=d;
}
SPX_FUSED_FN int spx_fused_valid(const spx_fused_domain *d) {
    return d && d->sealed && d->module==&spx_fused_module_identity && d->activated &&
        d->issuer && d->issuer->sealed && d->issuer->module==d->module &&
        d->issuer->active==d && d->invocation && d->slots && d->views && d->consumed &&
        d->state>=SPX_FUSED_BUILDING && d->state<=SPX_FUSED_ABORTED &&
        d->count && d->count<=SPX_FUSED_ROWS && d->width &&
        d->width<=SPX_FUSED_EXTENT/d->count && d->extent==d->count*d->width &&
        d->issued<=d->count && d->live<=d->issued && d->loans<=SPX_FUSED_VIEWS;
}
SPX_FUSED_FN int spx_fused_rows_receipt_current(const spx_fused_domain *d,const spx_fused_row_receipt *receipt) {
    return d && receipt==&d->rows_receipt && receipt->funded && receipt->domain==d &&
        receipt->issuer==d->issuer && receipt->ledger==d->ledger && receipt->invocation==d->invocation &&
        receipt->capacity==d->count && receipt->stride==sizeof(spx_fused_row) &&
        receipt->bytes==d->count*sizeof(spx_fused_row);
}
/* First original Bytes operand: exact tables, consumption of the admitted
   ordinary hold, then the single zeroed backing. No accounting can refuse
   here; physical allocation failure is fail-stop. */
SPX_FUSED_FN void spx_fused_activate_with_rows(spx_fused_domain *d,const spx_fused_row_receipt *receipt) {
    if (d->state!=SPX_FUSED_PREPARED || d->activated || !d->held || d->issuer->active!=d)
        SPX_FUSED_FAIL_STOP("fused activation without admission");
    if (!spx_fused_rows_receipt_current(d,receipt) || !receipt->allocated || receipt->settled ||
        !receipt->pointer || !d->vec_live) SPX_FUSED_FAIL_STOP("fused activation without funded destination");
    d->slots=calloc(d->count,sizeof(spx_fused_slot));
    if (!d->slots) SPX_FUSED_FAIL_STOP("fused lease table allocation failed");
    d->views=calloc(SPX_FUSED_VIEWS,sizeof(spx_fused_borrow));
    if (!d->views) SPX_FUSED_FAIL_STOP("fused view table allocation failed");
    if (!spx_fused_payload_consume(d->payload,1,d->extent)) SPX_FUSED_FAIL_STOP("fused payload hold drift");
    d->held=0;d->consumed=1;d->activated=1;d->state=SPX_FUSED_BUILDING;
    d->backing=calloc(d->extent,1);
    if (!d->backing) SPX_FUSED_FAIL_STOP("owned byte buffer allocation failed");
}
SPX_FUSED_FN void spx_fused_activate(spx_fused_domain *d) {
    spx_fused_activate_with_rows(d,&d->rows_receipt);
}
SPX_FUSED_FN spx_fused_slot *spx_fused_find(spx_fused_domain *d,const spx_fused_issuer *issuer,
 uint32_t index,uint32_t generation,uint64_t nonce) {
    if (!spx_fused_valid(d) || !nonce || !generation || issuer!=d->issuer || index>=d->issued)
        return NULL;
    spx_fused_slot *slot=&d->slots[index];
    if (!slot->live || slot->nonce!=nonce || slot->generation!=generation) return NULL;
    return slot;
}
SPX_FUSED_FN spx_fused_slot *spx_fused_owner(spx_fused_domain *d,spx_fused_bytes value) {
    return spx_fused_find(d,value.issuer,value.slot,value.generation,value.nonce);
}
/* Row Vec authority: exact domain, issuer, invocation and generation. */
SPX_FUSED_FN int spx_fused_rows_valid(const spx_fused_domain *d,const spx_fused_rows *rows) {
    return d && rows && d->vec_live && rows->domain==d && rows->rows && rows->issuer==d->issuer &&
        rows->invocation==d->invocation && rows->generation==d->vec_generation &&
        rows->capacity==d->count && rows->len<=rows->capacity &&
        spx_fused_rows_receipt_current(d,&d->rows_receipt) && d->rows_receipt.allocated &&
        !d->rows_receipt.settled && rows->rows==d->rows_receipt.pointer;
}
SPX_FUSED_FN spx_fused_slot *spx_fused_stored_owner(spx_fused_domain *d,const spx_fused_rows *rows,
 spx_fused_stored value) {
    if (!spx_fused_rows_valid(d,rows)) return NULL;
    return spx_fused_find(d,rows->issuer,value.slot,value.generation,value.nonce);
}
/* Every transfer renews both nonce and generation; neither ever wraps. */
SPX_FUSED_FN int spx_fused_renew(spx_fused_domain *d,spx_fused_slot *slot) {
    uint64_t nonce;
    if (slot->generation==UINT32_MAX || !spx_fused_mint(d->issuer,&nonce)) return spx_fused_fail(d);
    slot->nonce=nonce;slot->generation++;return 1;
}
SPX_FUSED_FN void spx_fused_settle(spx_fused_domain *d) {
    if (d->pin || d->live || d->loans) return;
    if (d->backing) free(d->backing);
    d->backing=NULL;d->state=SPX_FUSED_SETTLED;
    if (d->issuer && d->issuer->active==d) d->issuer->active=NULL;
}
SPX_FUSED_FN int spx_fused_issue(spx_fused_domain *d,size_t ordinal,spx_fused_bytes *out) {
    uint64_t nonce;
    if (!d || !d->sealed || !out) SPX_FUSED_FAIL_STOP("fused issue without held domain");
    if (d->failure) return 0;
    if (d->state==SPX_FUSED_PREPARED) {
        if (ordinal) return spx_fused_fail(d);
        spx_fused_activate(d);
    }
    if (!spx_fused_valid(d) || d->state!=SPX_FUSED_BUILDING || ordinal!=d->issued ||
        ordinal>=d->count || (d->issued && !d->slots[d->issued-1].published) ||
        !spx_fused_mint(d->issuer,&nonce)) return spx_fused_fail(d);
    spx_fused_slot *slot=&d->slots[ordinal];
    *slot=(spx_fused_slot){nonce,1,1,0,0};d->issued++;d->live++;
    *out=(spx_fused_bytes){d->issuer,(uint32_t)ordinal,1,nonce};return 1;
}
SPX_FUSED_FN int spx_fused_move(spx_fused_domain *d,spx_fused_bytes *source,spx_fused_bytes *out) {
    if (!source || !out || source==out) return spx_fused_fail(d);
    spx_fused_slot *slot=spx_fused_owner(d,*source);
    if (!slot || slot->borrows || d->failure || d->state==SPX_FUSED_ABORTED ||
        !spx_fused_renew(d,slot)) return spx_fused_fail(d);
    *out=(spx_fused_bytes){d->issuer,source->slot,slot->generation,slot->nonce};
    *source=(spx_fused_bytes){NULL,0,0,0};return 1;
}
/* Release one child. Building requires abort first so staged rows settle
   before the construction pin; the last child releases backing exactly once. */
SPX_FUSED_FN int spx_fused_drop_slot(spx_fused_domain *d,spx_fused_slot *slot) {
    if (!slot || slot->borrows ||
        (d->state!=SPX_FUSED_COMPLETE && d->state!=SPX_FUSED_ABORTED)) return spx_fused_fail(d);
    slot->live=0;slot->nonce=0;d->live--;spx_fused_settle(d);return 1;
}
SPX_FUSED_FN int spx_fused_drop(spx_fused_domain *d,spx_fused_bytes *value) {
    if (!value) return spx_fused_fail(d);
    if (!spx_fused_drop_slot(d,spx_fused_owner(d,*value))) return 0;
    *value=(spx_fused_bytes){NULL,0,0,0};return 1;
}
SPX_FUSED_FN int spx_fused_complete(spx_fused_domain *d) {
    if (!spx_fused_valid(d) || d->failure || d->state!=SPX_FUSED_BUILDING ||
        d->issued!=d->count || !d->slots[d->issued-1].published) return spx_fused_fail(d);
    d->state=SPX_FUSED_COMPLETE;d->pin=0;spx_fused_settle(d);return 1;
}
/* Administrative abort before canonical failure finalizers. A domain whose
   first Bytes operand never ran has nothing to retain. Otherwise inert. */
SPX_FUSED_FN void spx_fused_abort(spx_fused_domain *d) {
    if (d && d->sealed && d->state==SPX_FUSED_PREPARED && !d->activated) {
        d->pin=0;d->state=SPX_FUSED_SETTLED;
        if (d->issuer && d->issuer->active==d) d->issuer->active=NULL;
        return;
    }
    if (spx_fused_valid(d) && d->state==SPX_FUSED_BUILDING) d->state=SPX_FUSED_ABORTED;
}
SPX_FUSED_FN int spx_fused_settle_abort(spx_fused_domain *d) {
    if (d && d->sealed && d->state==SPX_FUSED_SETTLED) return 1;
    if (!spx_fused_valid(d) || d->state!=SPX_FUSED_ABORTED || d->live || d->loans) return 0;
    d->pin=0;spx_fused_settle(d);return 1;
}
/* Loans: complete-only, readonly, exact range, independent nonce per view. */
SPX_FUSED_FN int spx_fused_lend(spx_fused_domain *d,spx_fused_slot *slot,uint32_t index,spx_fused_view *out) {
    uint64_t nonce;
    if (!out || !slot || d->failure || d->state!=SPX_FUSED_COMPLETE) return spx_fused_fail(d);
    for (size_t i=0;i<SPX_FUSED_VIEWS;i++) if (!d->views[i].live) {
        if (!spx_fused_mint(d->issuer,&nonce)) return spx_fused_fail(d);
        d->views[i]=(spx_fused_borrow){nonce,slot->nonce,index,1};slot->borrows++;d->loans++;
        *out=(spx_fused_view){d->issuer,index,slot->generation,nonce,slot->nonce,
            d->backing+(size_t)index*d->width,d->width};
        return 1;
    }
    return spx_fused_fail(d);
}
SPX_FUSED_FN int spx_fused_borrow_begin(spx_fused_domain *d,spx_fused_bytes owner,spx_fused_view *out) {
    return spx_fused_lend(d,spx_fused_owner(d,owner),owner.slot,out);
}
SPX_FUSED_FN spx_fused_borrow *spx_fused_view_find(spx_fused_domain *d,spx_fused_view view) {
    spx_fused_slot *slot=spx_fused_find(d,view.issuer,view.slot,view.generation,view.owner);
    if (!slot || !slot->borrows || view.len!=d->width ||
        view.ptr!=d->backing+(size_t)view.slot*d->width) return NULL;
    for (size_t i=0;i<SPX_FUSED_VIEWS;i++) if (d->views[i].live && d->views[i].nonce==view.nonce &&
        d->views[i].owner==view.owner && d->views[i].slot==view.slot) return &d->views[i];
    return NULL;
}
SPX_FUSED_FN int spx_fused_view_len(spx_fused_domain *d,spx_fused_view view,uint64_t *out) {
    if (!out || d->failure || !spx_fused_view_find(d,view)) return spx_fused_fail(d);
    *out=view.len;return 1;
}
/* byte_get: authenticated before dereference; out of range is None, not failure. */
SPX_FUSED_FN int spx_fused_view_get(spx_fused_domain *d,spx_fused_view view,uint64_t index,
 bool *some,uint8_t *out) {
    if (!some || !out || d->failure || !spx_fused_view_find(d,view)) return spx_fused_fail(d);
    *some=index<view.len;*out=*some?view.ptr[index]:0;return 1;
}
SPX_FUSED_FN int spx_fused_borrow_end(spx_fused_domain *d,spx_fused_view *view) {
    if (!view) return spx_fused_fail(d);
    spx_fused_borrow *borrow=spx_fused_view_find(d,*view);
    if (!borrow) return spx_fused_fail(d);
    d->slots[view->slot].borrows--;d->loans--;
    *borrow=(spx_fused_borrow){0,0,0,0};*view=(spx_fused_view){NULL,0,0,0,0,NULL,0};
    spx_fused_settle(d);return 1;
}
/* Charged deep clone: ordinary event and width debit in the one payload
   account before a fresh allocation of only the authenticated range. */
SPX_FUSED_FN int spx_fused_clone(spx_fused_domain *d,spx_fused_bytes value,spx_fused_copy *out) {
    spx_fused_slot *slot=spx_fused_owner(d,value);
    if (!out || !slot || d->failure || d->state!=SPX_FUSED_COMPLETE ||
        !spx_fused_payload_debit(d->payload,1,d->width)) return spx_fused_fail(d);
    unsigned char *copy=calloc(d->width,1);
    if (!copy) SPX_FUSED_FAIL_STOP("owned byte buffer allocation failed");
    memcpy(copy,d->backing+(size_t)value.slot*d->width,d->width);
    *out=(spx_fused_copy){copy,d->width};return 1;
}
SPX_FUSED_FN void spx_fused_copy_drop(spx_fused_copy *copy) {
    if (copy && copy->ptr) free(copy->ptr);
    if (copy) *copy=(spx_fused_copy){NULL,0};
}
/* Same-ledger physical row debit occurs at the original Vec boundary. */
SPX_FUSED_FN void spx_fused_rows_reserve(spx_fused_domain *d,uint64_t capacity,spx_fused_row_receipt **out) {
    if (!d || !d->sealed || !out || d->state!=SPX_FUSED_PREPARED || d->vec_live ||
        d->rows_receipt.funded || !d->issuer || d->issuer->active!=d || d->module!=&spx_fused_module_identity ||
        capacity!=d->count || d->vec_generation==UINT64_MAX || !d->invocation)
        SPX_FUSED_FAIL_STOP("fused row reservation without held destination");
    size_t bytes=d->count*sizeof(spx_fused_row);
    if (!spx_fused_rows_consume(d->ledger,bytes)) SPX_FUSED_FAIL_STOP("fused row funding hold drift");
    d->rows_receipt=(spx_fused_row_receipt){d,d->issuer,d->ledger,d->invocation,NULL,d->count,sizeof(spx_fused_row),bytes,1,0,0};
    *out=&d->rows_receipt;
}
/* Physical calloc failure keeps the ordinary Vec failure status. No refund. */
SPX_FUSED_FN int spx_fused_rows_allocate_funded(spx_fused_domain *d,spx_fused_row_receipt *receipt,uint64_t capacity,spx_fused_rows *out) {
    if (!out || !spx_fused_rows_receipt_current(d,receipt) || receipt->allocated || receipt->settled ||
        d->state!=SPX_FUSED_PREPARED || d->vec_live || capacity!=d->count)
        SPX_FUSED_FAIL_STOP("fused row allocation without current funding receipt");
    *out=(spx_fused_rows){NULL,0,0,0,NULL,NULL,0};
    /* This receipt funds one attempt, including an ordinary calloc failure. */
    receipt->allocated=1;
    spx_fused_row *rows=calloc((size_t)capacity,sizeof(spx_fused_row));
    if (!rows) return 0;
    receipt->pointer=rows;
    *out=(spx_fused_rows){rows,0,capacity,++d->vec_generation,d,d->issuer,d->invocation};
    d->vec_live=1;return 1;
}
SPX_FUSED_FN int spx_fused_rows_allocate(spx_fused_domain *d,uint64_t capacity,spx_fused_rows *out) {
    spx_fused_row_receipt *receipt=NULL;spx_fused_rows_reserve(d,capacity,&receipt);
    return spx_fused_rows_allocate_funded(d,receipt,capacity,out);
}
SPX_FUSED_FN int spx_fused_rows_receipt_settle(spx_fused_domain *d,spx_fused_row_receipt *receipt) {
    if (!spx_fused_rows_receipt_current(d,receipt) || receipt->settled || d->vec_live) return 0;
    receipt->settled=1;receipt->pointer=NULL;return 1;
}
/* Immutable Vec transfer: the prior header is retired by generation. */
SPX_FUSED_FN int spx_fused_rows_move(spx_fused_domain *d,spx_fused_rows *source,spx_fused_rows *out) {
    if (!source || !out || source==out || !spx_fused_rows_valid(d,source) ||
        d->vec_generation==UINT64_MAX) return spx_fused_fail(d);
    *out=*source;out->generation=++d->vec_generation;
    *source=(spx_fused_rows){NULL,0,0,0,NULL,NULL,0};return 1;
}
SPX_FUSED_FN int spx_fused_rows_len(spx_fused_domain *d,const spx_fused_rows *rows,uint64_t *out) {
    if (!out || !spx_fused_rows_valid(d,rows)) return spx_fused_fail(d);
    *out=rows->len;return 1;
}
/* Push preflight runs while staged owners remain live; full is ordinary failure. */
SPX_FUSED_FN int spx_fused_rows_has_room(spx_fused_domain *d,const spx_fused_rows *rows) {
    if (!spx_fused_rows_valid(d,rows)) SPX_FUSED_FAIL_STOP("fused row Vec preflight without authority");
    return rows->len<rows->capacity;
}
/* Atomic row commit: the staged child must be the newest unpublished lease. */
SPX_FUSED_FN int spx_fused_rows_push(spx_fused_domain *d,spx_fused_rows *rows,spx_fused_staged *row) {
    if (!spx_fused_valid(d) || d->failure || d->state!=SPX_FUSED_BUILDING || !row ||
        !spx_fused_rows_valid(d,rows) || rows->len>=rows->capacity || d->vec_generation==UINT64_MAX)
        return spx_fused_fail(d);
    spx_fused_slot *slot=spx_fused_owner(d,row->bytes);
    if (!slot || slot->published || slot->borrows || row->bytes.slot!=rows->len ||
        (size_t)row->bytes.slot+1!=d->issued || !spx_fused_renew(d,slot)) return spx_fused_fail(d);
    slot->published=1;
    rows->rows[rows->len++]=(spx_fused_row){row->scalar,row->string,
        {row->bytes.slot,slot->generation,slot->nonce}};
    rows->generation=++d->vec_generation;
    *row=(spx_fused_staged){0,NULL,{NULL,0,0,0}};return 1;
}
/* Row index access; an index outside the current length is ordinary failure. */
SPX_FUSED_FN int spx_fused_rows_at(spx_fused_domain *d,const spx_fused_rows *rows,uint64_t index,
 const spx_fused_row **out) {
    if (!out || !spx_fused_rows_valid(d,rows)) SPX_FUSED_FAIL_STOP("fused row read without authority");
    *out=NULL;if (index>=rows->len) return 0;
    *out=&rows->rows[index];return 1;
}
SPX_FUSED_FN int spx_fused_rows_borrow(spx_fused_domain *d,const spx_fused_rows *rows,uint64_t index,
 spx_fused_view *out) {
    if (!spx_fused_rows_valid(d,rows) || index>=rows->len) return spx_fused_fail(d);
    spx_fused_stored stored=rows->rows[index].bytes;
    return spx_fused_lend(d,spx_fused_stored_owner(d,rows,stored),stored.slot,out);
}
/* Store: replace the child of one row. The displaced child settles first. */
SPX_FUSED_FN int spx_fused_rows_store(spx_fused_domain *d,spx_fused_rows *rows,uint64_t index,
 spx_fused_bytes *value) {
    if (!value || d->failure || d->state!=SPX_FUSED_COMPLETE || !spx_fused_rows_valid(d,rows) ||
        index>=rows->len) return spx_fused_fail(d);
    spx_fused_slot *incoming=spx_fused_owner(d,*value);
    spx_fused_slot *displaced=spx_fused_stored_owner(d,rows,rows->rows[index].bytes);
    if (!incoming || !displaced || incoming==displaced || incoming->borrows || displaced->borrows ||
        d->vec_generation==UINT64_MAX || !spx_fused_renew(d,incoming)) return spx_fused_fail(d);
    uint32_t slot=value->slot;
    displaced->live=0;displaced->nonce=0;d->live--;
    rows->rows[index].bytes=(spx_fused_stored){slot,incoming->generation,incoming->nonce};
    rows->generation=++d->vec_generation;
    *value=(spx_fused_bytes){NULL,0,0,0};return 1;
}
/* Pop: the child leaves the Vec but stays in the held domain until dropped. */
SPX_FUSED_FN int spx_fused_rows_pop(spx_fused_domain *d,spx_fused_rows *rows,spx_fused_staged *out) {
    if (!out || d->failure || d->state!=SPX_FUSED_COMPLETE || !spx_fused_rows_valid(d,rows) ||
        !rows->len || d->vec_generation==UINT64_MAX) return spx_fused_fail(d);
    spx_fused_row *row=&rows->rows[rows->len-1];
    spx_fused_slot *slot=spx_fused_stored_owner(d,rows,row->bytes);
    if (!slot || slot->borrows || !spx_fused_renew(d,slot)) return spx_fused_fail(d);
    *out=(spx_fused_staged){row->scalar,row->string,{d->issuer,row->bytes.slot,slot->generation,slot->nonce}};
    *row=(spx_fused_row){0,NULL,{0,0,0}};rows->len--;rows->generation=++d->vec_generation;
    return 1;
}
SPX_FUSED_FN int spx_fused_rows_compare(spx_fused_domain *d,const spx_fused_row *a,
 const spx_fused_row *b,const uint32_t order[3],spx_fused_string_len length) {
    for (unsigned k=0;k<3;k++) {
        if (order[k]==SPX_FUSED_SCALAR) {
            if (a->scalar!=b->scalar) return a->scalar>b->scalar?1:-1;
            continue;
        }
        const unsigned char *x,*y;uint64_t xn,yn;
        if (order[k]==SPX_FUSED_STRING) {
            x=(const unsigned char *)a->string;y=(const unsigned char *)b->string;
            xn=length(a->string);yn=length(b->string);
        } else {
            x=d->backing+(size_t)a->bytes.slot*d->width;y=d->backing+(size_t)b->bytes.slot*d->width;
            xn=d->width;yn=d->width;
        }
        uint64_t common=xn<yn?xn:yn;int sign=common?memcmp(x,y,(size_t)common):0;
        if (sign) return sign>0?1:-1;
        if (xn!=yn) return xn>yn?1:-1;
    }
    return 0;
}
/* Consuming stable sort in declaration-order comparison. Every child is
   authenticated and unborrowed before any row moves; refusal mutates nothing. */
SPX_FUSED_FN int spx_fused_rows_sort(spx_fused_domain *d,spx_fused_rows *source,spx_fused_rows *out,
 const uint32_t order[3],spx_fused_string_len length) {
    if (!source || !out || source==out || !order || !length || d->failure ||
        d->state!=SPX_FUSED_COMPLETE || !spx_fused_rows_valid(d,source) ||
        d->vec_generation==UINT64_MAX) return spx_fused_fail(d);
    for (uint64_t i=0;i<source->len;i++) {
        spx_fused_slot *slot=spx_fused_stored_owner(d,source,source->rows[i].bytes);
        if (!slot || slot->borrows) return spx_fused_fail(d);
    }
    spx_fused_row *rows=source->rows;
    for (uint64_t i=1;i<source->len;i++) {
        spx_fused_row row=rows[i];uint64_t j=i;
        while (j && spx_fused_rows_compare(d,&rows[j-1],&row,order,length)>0) {rows[j]=rows[j-1];--j;}
        rows[j]=row;
    }
    *out=*source;out->generation=++d->vec_generation;
    *source=(spx_fused_rows){NULL,0,0,0,NULL,NULL,0};return 1;
}
/* The Vec finalizer: rows in index order, fields in declaration order, the
   ordinary String drop and the held domain's child release. */
SPX_FUSED_FN int spx_fused_rows_drop(spx_fused_domain *d,spx_fused_rows *rows,const uint32_t order[3],
 spx_fused_string_drop drop) {
    if (!order || !drop || !spx_fused_rows_valid(d,rows)) return spx_fused_fail(d);
    int settled=1;
    for (uint64_t i=0;i<rows->len;i++) for (unsigned k=0;k<3;k++) {
        spx_fused_row *row=&rows->rows[i];
        if (order[k]==SPX_FUSED_STRING && row->string) {drop(row->string);row->string=NULL;}
        if (order[k]==SPX_FUSED_PAYLOAD) {
            if (!spx_fused_drop_slot(d,spx_fused_stored_owner(d,rows,row->bytes))) settled=0;
            row->bytes=(spx_fused_stored){0,0,0};
        }
    }
    free(rows->rows);d->vec_live=0;*rows=(spx_fused_rows){NULL,0,0,0,NULL,NULL,0};
    if (!spx_fused_rows_receipt_settle(d,&d->rows_receipt)) settled=0;
    return settled;
}
/* A standalone row follows the caller's authenticated epoch cleanup order,
   which is reverse declaration order. Vec internals use rows_drop instead. */
SPX_FUSED_FN int spx_fused_staged_drop(spx_fused_domain *d,spx_fused_staged *row,const uint32_t order[3],
 spx_fused_string_drop drop) {
    if (!row || !order || !drop) return spx_fused_fail(d);
    int settled=1;
    for (unsigned k=0;k<3;k++) {
        if (order[k]==SPX_FUSED_STRING && row->string) {drop(row->string);row->string=NULL;}
        if (order[k]==SPX_FUSED_PAYLOAD && row->bytes.nonce && !spx_fused_drop(d,&row->bytes)) settled=0;
    }
    return settled;
}
/* Invocation exit. A settled domain frees its exact tables; an unconsumed
   payload hold is released (it was never debited). Anything still live is
   abandonment: backing and tables are released, no result is published and
   no language finalizer order is synthesized. The frame owner frees the frame. */
SPX_FUSED_FN int spx_fused_finish(spx_fused_domain *d) {
    if (!d || !d->sealed || d->state==SPX_FUSED_FINISHED) return 0;
    int settled=d->state==SPX_FUSED_SETTLED && !d->live && !d->loans && !d->vec_live && !d->backing;
    if (d->held) {
        if (!spx_fused_payload_release(d->payload,1,d->extent)) SPX_FUSED_FAIL_STOP("fused payload hold drift");
        d->held=0;
    }
    if (!d->rows_receipt.funded) {
        size_t bytes=d->count*sizeof(spx_fused_row);
        if (!d->ledger || d->ledger->held_rows<bytes) SPX_FUSED_FAIL_STOP("fused unused row hold drift");
        d->ledger->held_rows-=bytes;
    } else if (!d->vec_live && !d->rows_receipt.settled) {
        if (!spx_fused_rows_receipt_settle(d,&d->rows_receipt)) SPX_FUSED_FAIL_STOP("fused row receipt settlement drift");
    }
    if (!settled && d->vec_live) {
        /* Administrative abandonment releases actual row backing; it does
           not synthesize a language field finalizer or publish a result. */
        free(d->rows_receipt.pointer);d->vec_live=0;
        if (!spx_fused_rows_receipt_settle(d,&d->rows_receipt)) SPX_FUSED_FAIL_STOP("abandoned row receipt drift");
    }
    if (!settled && d->backing) {free(d->backing);d->backing=NULL;}
    if (d->slots) free(d->slots);
    if (d->views) free(d->views);
    d->slots=NULL;d->views=NULL;d->state=SPX_FUSED_FINISHED;
    if (d->issuer && d->issuer->active==d) d->issuer->active=NULL;
    return settled;
}
