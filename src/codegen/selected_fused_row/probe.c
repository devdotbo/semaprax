/* Owning runtime probe for the selected native domain. Calls the same runtime
   text the selected emitter embeds; the emitted-program gates live in tests.rs.
   No probe resets a sticky failure or rewrites a ledger counter: every refusal
   is observed on a fresh domain and then settled through the real API. */
typedef struct { uint64_t len; char data[]; } probe_text;
static char *text(const char *bytes, uint64_t len) {
    probe_text *t = malloc(sizeof(probe_text) + (size_t)len + 1);
    assert(t);
    t->len = len; memcpy(t->data, bytes, (size_t)len); t->data[len] = 0;
    return t->data;
}
static uint64_t text_len(const char *value) {
    return ((const probe_text *)(const void *)(value - offsetof(probe_text, data)))->len;
}
static void text_drop(char *value) { free(value - offsetof(probe_text, data)); }
static const uint32_t declared[3] = {SPX_FUSED_SCALAR, SPX_FUSED_STRING, SPX_FUSED_PAYLOAD};
static const uint32_t standalone[3] = {SPX_FUSED_PAYLOAD, SPX_FUSED_STRING, SPX_FUSED_SCALAR};
/* Standalone record cleanup must release payload before its String; this
   callback observes actual epoch order independently from the order array. */
static const spx_fused_staged *standalone_observed;
static void standalone_text_drop(char *value) {
    assert(standalone_observed && !standalone_observed->bytes.nonce);
    text_drop(value);
}
static int standalone_drop(spx_fused_domain *d, spx_fused_staged *row) {
    standalone_observed = row;
    int settled = spx_fused_staged_drop(d, row, standalone, standalone_text_drop);
    standalone_observed = NULL;
    return settled;
}
static const size_t header = sizeof(spx_fused_domain);
static const size_t original_metadata = 240u + 4096u * 24u + 64u * 24u;
static const size_t original_funding = 240u + 4096u * 24u + 64u * 24u + 4096u * 32u;

typedef struct {
    spx_fused_domain *d; spx_fused_ledger ledger; spx_fused_payload payload; spx_fused_rows rows;
} fixture;
/* Admission funds the frame and tables and holds the payload before any effect. */
static int open_domain(fixture *f, spx_fused_issuer *issuer, size_t count, size_t width,
                       size_t frame, size_t limit) {
    memset(f, 0, sizeof(*f));
    assert(spx_fused_ledger_init(&f->ledger, limit) && spx_fused_payload_init(&f->payload));
    void *storage = spx_fused_admit(issuer, &f->ledger, &f->payload, count, width, frame);
    if (!storage) return 0;
    f->d = storage;
    spx_fused_bind(f->d, issuer, &f->ledger, &f->payload, count, width, frame);
    return 1;
}
static int close_domain(fixture *f) {
    int settled = spx_fused_finish(f->d);
    free(f->d); f->d = NULL;
    return settled;
}
/* Builds `count` complete rows; scalar = base + sign * i. */
static void build(fixture *f, spx_fused_issuer *issuer, size_t count, size_t width,
                  int64_t base, int64_t sign) {
    assert(open_domain(f, issuer, count, width, header, spx_fused_invocation_bytes(count, header)));
    assert(spx_fused_rows_allocate(f->d, count, &f->rows));
    for (size_t i = 0; i < count; i++) {
        spx_fused_staged row = {base + sign * (int64_t)i, text("\xc3\xa9\0", 3), {0}};
        assert(spx_fused_issue(f->d, i, &row.bytes));
        assert(spx_fused_rows_has_room(f->d, &f->rows));
        assert(spx_fused_rows_push(f->d, &f->rows, &row));
        assert(!row.string && !row.bytes.nonce);
    }
    assert(spx_fused_complete(f->d));
}
static void settle(fixture *f) {
    assert(spx_fused_rows_drop(f->d, &f->rows, declared, text_drop));
    assert(close_domain(f));
}

/* Original 4096 rows: one backing, 12288 payload bytes, sum 8386560, exact
   admission funding before the frame, tables after the first String,
   reentry with fresh nonces. */
static void maximum(void) {
    spx_fused_issuer issuer = {0};
    assert(spx_fused_issuer_init(&issuer) && !spx_fused_issuer_init(&issuer));
    spx_fused_bytes previous = {0};
    for (unsigned round = 0; round < 3; round++) {
        census_reset();
        assert(spx_fused_metadata_bytes(4096, header) == original_metadata);
        fixture f;
        assert(open_domain(&f, &issuer, 4096, 3, header, original_funding));
        assert(f.ledger.debited == original_metadata && f.payload.held_events == 1 &&
               f.payload.held_bytes == 12288 && !f.payload.events && issuer.active == f.d);
        assert(census.calls == 1 && census.bytes[0] == header);
        assert(spx_fused_rows_allocate(f.d, 4096, &f.rows));
        assert(census.calls == 2 && census.bytes[1] == 4096u * 32u);
        assert(f.ledger.debited == original_funding && !f.ledger.held_rows);
        int64_t sum = 0;
        for (size_t i = 0; i < 4096; i++) {
            spx_fused_staged row = {(int64_t)i, text("\xc3\xa9\0", 3), {0}};
            if (!i) assert(census.calls == 2 && census.mallocs == 1);
            assert(spx_fused_issue(f.d, i, &row.bytes));
            if (!i) {
                assert(census.calls == 5 && census.bytes[2] == 4096u * 24u &&
                       census.bytes[3] == 64u * 24u && census.bytes[4] == 12288u);
                assert(f.payload.events == 1 && f.payload.cumulative == 12288 && !f.payload.held_events);
                if (round) assert(!spx_fused_owner(f.d, previous) && !f.d->failure);
            }
            assert(spx_fused_rows_push(f.d, &f.rows, &row));
            sum += (int64_t)i;
        }
        assert(sum == 8386560 && census.calls == 5);
        previous = (spx_fused_bytes){&issuer, 0, f.rows.rows[0].bytes.generation, f.rows.rows[0].bytes.nonce};
        assert(spx_fused_complete(f.d));
        spx_fused_rows sorted = {0};
        assert(spx_fused_rows_sort(f.d, &f.rows, &sorted, declared, text_len));
        assert(!f.rows.rows && !spx_fused_rows_valid(f.d, &f.rows));
        int64_t observed = 0;
        for (uint64_t i = 0; i < 4096; i++) {
            const spx_fused_row *row = NULL;
            assert(spx_fused_rows_at(f.d, &sorted, i, &row) && row->scalar == (int64_t)i);
            assert(text_len(row->string) == 3 && !memcmp(row->string, "\xc3\xa9\0", 3));
            spx_fused_view view = {0};
            assert(spx_fused_rows_borrow(f.d, &sorted, i, &view));
            for (uint64_t j = 0; j < 4; j++) {
                bool some = false; uint8_t byte = 255;
                assert(spx_fused_view_get(f.d, view, j, &some, &byte));
                assert(some == (j < 3) && byte == 0);
            }
            assert(spx_fused_borrow_end(f.d, &view));
            observed += row->scalar;
        }
        assert(observed == 8386560);
        size_t before = census.frees;
        f.rows = sorted;
        assert(spx_fused_rows_drop(f.d, &f.rows, declared, text_drop));
        assert(census.frees == before + 4096 + 2 && f.d->state == SPX_FUSED_SETTLED && !issuer.active);
        assert(close_domain(&f) && !census.live && census.calls == 5);
        assert(f.payload.events == 1 && f.payload.cumulative == 12288 && f.ledger.debited == original_funding);
    }
}

/* Admission refusals precede every effect and every allocation. */
static void admission(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    census_reset();
    fixture f;
    uint64_t next = issuer.next;
    assert(!open_domain(&f, &issuer, 4096, 3, header, original_funding - 1));
    assert(!census.calls && f.ledger.failed && !f.ledger.debited && !f.payload.held_events);
    assert(issuer.next == next && !issuer.active);
    assert(!spx_fused_ledger_debit(&f.ledger, 0)); /* sticky; no refund path exists */
    /* The one payload account: no room for another backing event refuses. */
    spx_fused_payload full = {0}; spx_fused_ledger ledger = {0};
    assert(spx_fused_payload_init(&full) && spx_fused_ledger_init(&ledger, original_funding));
    assert(spx_fused_payload_debit(&full, 32, 0));
    assert(!spx_fused_admit(&issuer, &ledger, &full, 1, 3, header));
    assert(full.failed && !ledger.debited && full.events == 32 && !census.calls);
    assert(!spx_fused_payload_debit(&full, 0, 0)); /* sticky */
    /* Overlap: the held issuer already grants an active domain. */
    fixture a, b;
    build(&a, &issuer, 2, 3, 0, 1);
    assert(!open_domain(&b, &issuer, 1, 3, header, spx_fused_invocation_bytes(1, header)));
    assert(!b.ledger.debited && !b.payload.held_events && issuer.active == a.d);
    settle(&a);
    /* Frame placement is part of the same debit; an abandoned live child is
       released at exit without publication. */
    size_t frame = header + 96;
    assert(open_domain(&b, &issuer, 1, 3, frame, spx_fused_invocation_bytes(1, frame)));
    assert(b.ledger.debited == 240u + 96u + 24u + 1536u && census.bytes[census.calls - 1] == frame);
    assert(spx_fused_rows_allocate(b.d, 1, &b.rows));
    spx_fused_bytes child = {0};
    assert(spx_fused_issue(b.d, 0, &child));
    assert(!close_domain(&b) && !issuer.active && !census.live);
    /* A hold that no Bytes operand consumed is released, never debited. */
    assert(open_domain(&b, &issuer, 2, 3, header, spx_fused_invocation_bytes(2, header)));
    assert(spx_fused_rows_allocate(b.d, 2, &b.rows));
    spx_fused_abort(b.d);
    assert(spx_fused_settle_abort(b.d) && spx_fused_rows_drop(b.d, &b.rows, declared, text_drop));
    assert(close_domain(&b) && !b.payload.held_events && !b.payload.events && !b.payload.cumulative);
    assert(b.ledger.debited == spx_fused_invocation_bytes(2, header) && !census.live);
    /* Issuer exhaustion refuses before any debit and never wraps. */
    issuer.next = UINT64_MAX - 3;
    assert(!open_domain(&b, &issuer, 2, 3, header, spx_fused_invocation_bytes(2, header)));
    assert(!b.ledger.debited && issuer.next == UINT64_MAX - 3 && !issuer.active);
}

/* Physical allocation failure is fail-stop, observed after its funding. */
static void physical_fail_stop_order(void) {
    static const size_t cuts[] = {1, 3, 4, 5};
    for (unsigned k = 0; k < 4; k++) {
        size_t cut = cuts[k];
        spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
        census_reset(); census.fail_at = cut;
        static fixture f;
        static spx_fused_bytes out;
        memset(&f, 0, sizeof(f));
        out = (spx_fused_bytes){0, 7, 7, 7};
        assert(spx_fused_ledger_init(&f.ledger, spx_fused_invocation_bytes(8, header)));
        assert(spx_fused_payload_init(&f.payload));
        jmp_buf target; stop_target = &target;
        if (!setjmp(target)) {
            void *storage = spx_fused_admit(&issuer, &f.ledger, &f.payload, 8, 3, header);
            f.d = storage;
            spx_fused_bind(f.d, &issuer, &f.ledger, &f.payload, 8, 3, header);
            assert(spx_fused_rows_allocate(f.d, 8, &f.rows));
            (void)spx_fused_issue(f.d, 0, &out);
            assert(0);
        }
        stop_target = NULL;
        assert(out.nonce == 7 && f.ledger.debited == (cut == 1 ? spx_fused_metadata_bytes(8, header) : spx_fused_invocation_bytes(8, header)));
        assert(!strcmp(stop_message, cut == 1 ? "fused frame allocation failed"
                       : cut == 5 ? "owned byte buffer allocation failed"
                       : cut == 4 ? "fused view table allocation failed" : "fused lease table allocation failed"));
        assert(cut == 5 ? f.payload.events == 1 && f.payload.cumulative == 24 && !f.payload.held_events
                        : !f.payload.events && f.payload.held_events == 1);
        if (cut != 1) {
            (void)close_domain(&f);
        }
        assert(!census.live && !issuer.active);
    }
}

/* The ordinary Vec allocation boundary returns failure without fail-stop. */
static void vec_allocation_failure(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    census_reset(); census.fail_at = 2;
    fixture f;
    assert(open_domain(&f, &issuer, 8, 3, header, spx_fused_invocation_bytes(8, header)));
    assert(!spx_fused_rows_allocate(f.d, 8, &f.rows) && !f.rows.rows && !f.d->vec_live);
    assert(f.ledger.debited == spx_fused_invocation_bytes(8, header) && !f.ledger.held_rows);
    spx_fused_abort(f.d);
    assert(spx_fused_settle_abort(f.d) && close_domain(&f) && !f.payload.held_events && !census.live);
}

/* Funding is address-bound and consumed once before the real destination
   allocation. Copies, altered geometry and foreign accounts are never leases. */
static void row_funding_receipt(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    census_reset();
    fixture f;
    assert(open_domain(&f, &issuer, 8, 3, header, spx_fused_invocation_bytes(8, header)));
    assert(f.ledger.debited == spx_fused_metadata_bytes(8, header) && f.ledger.held_rows == 8 * sizeof(spx_fused_row));
    spx_fused_row_receipt *receipt = NULL;
    spx_fused_rows_reserve(f.d, 8, &receipt);
    assert(receipt == &f.d->rows_receipt && !receipt->pointer && !receipt->allocated);
    assert(f.ledger.debited == spx_fused_invocation_bytes(8, header) && !f.ledger.held_rows && census.calls == 1);
    spx_fused_row_receipt copy = *receipt;
    assert(!spx_fused_rows_receipt_current(f.d, &copy));
    copy.capacity++; assert(!spx_fused_rows_receipt_current(f.d, &copy));
    copy = *receipt; copy.invocation++; assert(!spx_fused_rows_receipt_current(f.d, &copy));
    copy = *receipt; copy.ledger = NULL; assert(!spx_fused_rows_receipt_current(f.d, &copy));
    jmp_buf target; stop_target = &target;
    if (!setjmp(target)) {
        (void)spx_fused_rows_allocate_funded(f.d, &copy, 8, &f.rows);
        assert(0);
    }
    stop_target = NULL;
    assert(!strcmp(stop_message, "fused row allocation without current funding receipt"));
    assert(census.calls == 1 && !receipt->allocated && !f.rows.rows);
    assert(spx_fused_rows_allocate_funded(f.d, receipt, 8, &f.rows));
    assert(receipt->pointer == f.rows.rows && receipt->allocated && !receipt->settled);
    spx_fused_rows wrong = f.rows; wrong.rows++;
    assert(!spx_fused_rows_valid(f.d, &wrong));
    spx_fused_abort(f.d);
    assert(spx_fused_settle_abort(f.d) && spx_fused_rows_drop(f.d, &f.rows, declared, text_drop));
    assert(receipt->settled && !receipt->pointer && !spx_fused_rows_receipt_settle(f.d, receipt));
    assert(close_domain(&f) && !census.live && f.ledger.debited == spx_fused_invocation_bytes(8, header));
}

/* Partial source/push failure: earlier rows and the staged row settle in
   canonical order while the construction pin holds the single backing. */
static void partial_rows(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    for (size_t cut = 1; cut <= 8; cut++) {
        census_reset();
        fixture f;
        assert(open_domain(&f, &issuer, 8, 3, header, spx_fused_invocation_bytes(8, header)));
        assert(spx_fused_rows_allocate(f.d, 8, &f.rows));
        spx_fused_staged staged = {0};
        for (size_t i = 0; i < cut; i++) {
            staged = (spx_fused_staged){(int64_t)i, text("t", 1), {0}};
            assert(spx_fused_issue(f.d, i, &staged.bytes));
            if (i + 1 < cut) assert(spx_fused_rows_push(f.d, &f.rows, &staged));
        }
        spx_fused_bytes early = {0};
        assert(!spx_fused_issue(f.d, cut, &early) && f.d->failure); /* unpublished predecessor */
        assert(!spx_fused_complete(f.d) && !spx_fused_drop(f.d, &staged.bytes));
        spx_fused_abort(f.d);
        assert(!spx_fused_settle_abort(f.d));
        assert(standalone_drop(f.d, &staged));
        assert(f.d->backing && f.d->pin);
        assert(spx_fused_rows_drop(f.d, &f.rows, declared, text_drop));
        assert(f.d->backing && !f.d->live);
        assert(spx_fused_settle_abort(f.d) && !f.d->backing && f.d->failure == 1);
        assert(close_domain(&f) && !census.live && census.calls == 5);
    }
}

static void identity(void) {
    spx_fused_issuer issuer = {0}, foreign = {0};
    assert(spx_fused_issuer_init(&issuer) && spx_fused_issuer_init(&foreign));
    census_reset();
    fixture f; build(&f, &issuer, 4, 3, 30, -10);
    spx_fused_stored s = f.rows.rows[1].bytes;
    spx_fused_bytes owner = {&issuer, s.slot, s.generation, s.nonce}, wrong;
    assert(spx_fused_owner(f.d, owner));
    wrong = owner; wrong.issuer = &foreign; assert(!spx_fused_owner(f.d, wrong));
    wrong = owner; wrong.slot = 4; assert(!spx_fused_owner(f.d, wrong));
    wrong = owner; wrong.generation++; assert(!spx_fused_owner(f.d, wrong));
    wrong = owner; wrong.nonce = UINT64_MAX; assert(!spx_fused_owner(f.d, wrong));
    /* The compact stored owner authenticates only through its exact header. */
    spx_fused_rows forged = f.rows;
    forged.invocation++; assert(!spx_fused_stored_owner(f.d, &forged, s));
    forged = f.rows; forged.issuer = &foreign; assert(!spx_fused_stored_owner(f.d, &forged, s));
    forged = f.rows; forged.generation--; assert(!spx_fused_stored_owner(f.d, &forged, s));
    assert(spx_fused_stored_owner(f.d, &f.rows, s));
    /* Independent views; a retired view nonce never authenticates again. */
    spx_fused_view first = {0}, second = {0};
    assert(spx_fused_rows_borrow(f.d, &f.rows, 1, &first));
    assert(spx_fused_rows_borrow(f.d, &f.rows, 1, &second));
    spx_fused_view old = first; assert(spx_fused_borrow_end(f.d, &first));
    assert(spx_fused_rows_borrow(f.d, &f.rows, 1, &first) && first.nonce != old.nonce);
    assert(!spx_fused_view_find(f.d, old));
    spx_fused_view fake = first; fake.len++; assert(!spx_fused_view_find(f.d, fake));
    fake = first; fake.ptr++; assert(!spx_fused_view_find(f.d, fake));
    fake = first; fake.generation++; assert(!spx_fused_view_find(f.d, fake));
    assert(!f.d->failure);
    assert(spx_fused_borrow_end(f.d, &first) && spx_fused_borrow_end(f.d, &second));
    settle(&f);
    assert(!census.live && !issuer.active);
}

/* Consuming sort cannot move a borrowed row; the refusal is sticky and
   mutates nothing; canonical settlement still completes. */
static void sort_blocked(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    census_reset();
    fixture f; build(&f, &issuer, 4, 3, 30, -10);
    spx_fused_view held = {0};
    assert(spx_fused_rows_borrow(f.d, &f.rows, 1, &held));
    spx_fused_rows sorted = {0};
    assert(!spx_fused_rows_sort(f.d, &f.rows, &sorted, declared, text_len));
    assert(f.rows.rows && f.rows.rows[0].scalar == 30 && f.rows.rows[3].scalar == 0 && f.d->failure);
    assert(!sorted.rows);
    assert(spx_fused_borrow_end(f.d, &held));
    settle(&f);
    assert(!census.live && !issuer.active);
}

static void pop_store_clone(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    census_reset();
    fixture f; build(&f, &issuer, 4, 3, 30, -10); /* scalars 30,20,10,0 */
    spx_fused_rows sorted = {0};
    assert(spx_fused_rows_sort(f.d, &f.rows, &sorted, declared, text_len));
    for (uint32_t i = 0; i < 4; i++)
        assert(sorted.rows[i].scalar == (int64_t)i * 10 && sorted.rows[i].bytes.slot == 3 - i);
    /* Popped child survives Vec settlement; store replaces and retires. */
    spx_fused_staged popped = {0};
    assert(spx_fused_rows_pop(f.d, &sorted, &popped) && sorted.len == 3 && popped.bytes.slot == 0);
    spx_fused_bytes moved = {0}, stale = popped.bytes;
    assert(spx_fused_move(f.d, &popped.bytes, &moved) && !spx_fused_owner(f.d, stale));
    assert(spx_fused_rows_store(f.d, &sorted, 2, &moved) && !moved.nonce);
    assert(sorted.rows[2].bytes.slot == 0 && f.d->live == 3);
    spx_fused_staged second = {0};
    assert(spx_fused_rows_pop(f.d, &sorted, &second) && second.bytes.slot == 0);
    /* Charged deep clone in the one payload account; isolated copy. */
    spx_fused_copy copy = {0};
    f.d->backing[0] = 7;
    assert(spx_fused_clone(f.d, second.bytes, &copy) && copy.len == 3 && copy.ptr[0] == 7);
    assert(f.payload.events == 2 && f.payload.cumulative == 15);
    f.rows = sorted;
    assert(spx_fused_rows_drop(f.d, &f.rows, declared, text_drop));
    assert(f.d->backing && f.d->live == 1);
    spx_fused_view held = {0};
    assert(spx_fused_borrow_begin(f.d, second.bytes, &held));
    assert(!spx_fused_drop(f.d, &second.bytes) && second.bytes.nonce); /* sticky from here */
    assert(spx_fused_borrow_end(f.d, &held));
    assert(standalone_drop(f.d, &second) && !f.d->backing);
    assert(copy.ptr[0] == 7); spx_fused_copy_drop(&copy);
    text_drop(popped.string);
    assert(close_domain(&f) && !census.live);
}

/* Clone refuses at the unchanged 32-event cap with no allocation. */
static void clone_cap(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    census_reset();
    fixture f; build(&f, &issuer, 1, 3, 0, 1);
    assert(spx_fused_payload_debit(&f.payload, 31, 0) && f.payload.events == 32);
    spx_fused_staged last = {0};
    assert(spx_fused_rows_pop(f.d, &f.rows, &last));
    spx_fused_copy refused = {(unsigned char *)1, 9};
    size_t calls = census.calls;
    assert(!spx_fused_clone(f.d, last.bytes, &refused) && census.calls == calls && refused.len == 9);
    assert(f.payload.failed && f.payload.events == 32 && f.d->failure);
    assert(standalone_drop(f.d, &last));
    settle(&f);
    assert(!census.live);
}

/* Generation exhaustion refuses without wrapping (state injected by the probe). */
static void generation_exhaustion(void) {
    spx_fused_issuer issuer = {0}; assert(spx_fused_issuer_init(&issuer));
    census_reset();
    fixture f; build(&f, &issuer, 1, 3, 0, 1);
    spx_fused_staged last = {0};
    assert(spx_fused_rows_pop(f.d, &f.rows, &last));
    f.d->slots[0].generation = UINT32_MAX; last.bytes.generation = UINT32_MAX;
    spx_fused_bytes moved = {0};
    assert(!spx_fused_move(f.d, &last.bytes, &moved) && last.bytes.nonce && f.d->slots[0].generation == UINT32_MAX);
    assert(standalone_drop(f.d, &last));
    settle(&f);
    assert(!census.live && !issuer.active);
}

int main(void) {
    maximum();
    admission();
    physical_fail_stop_order();
    vec_allocation_failure();
    row_funding_receipt();
    partial_rows();
    identity();
    sort_blocked();
    pop_store_clone();
    clone_cap();
    generation_exhaustion();
    return 0;
}
