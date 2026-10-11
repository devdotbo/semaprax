/* Test-owned driver appended after an emitted selected unit. It is the only
   caller of the private receiving entry; no public symbol is involved.
   SPX_EXPECTED_RESULT is defined by the owning Rust test. */
typedef struct {
    struct spx_status_entry entries[1];
    struct spx_context context;
} invocation;
typedef struct {
    spx_fused_ledger ledger;
    spx_fused_payload payload;
} accounts;
static void open_accounts(accounts *held, size_t limit) {
    memset(held, 0, sizeof(*held));
    assert(spx_fused_ledger_init(&held->ledger, limit) && spx_fused_payload_init(&held->payload));
}
static int invoke(invocation *call, spx_fused_issuer *issuer, accounts *held,
                  int64_t *result, spx_status_token *status) {
    memset(call, 0, sizeof(*call));
    assert(spx_context_init(&call->context, UINT64_C(1), call->entries, UINT32_C(1), NULL, NULL, NULL));
    return spx_fused_selected_entry_v1(&call->context, issuer, &held->ledger, &held->payload, result, status);
}
static void expect_status(const invocation *call, const char *domain, uint32_t code) {
    assert(call->context.status_arena.length == 1);
    assert(call->entries[0].status.code == code);
    assert(!strcmp(call->entries[0].status.domain_id, domain));
}
/* Admission frame, the ordinary Vec, the first row's String, then the exact
   tables and the single backing at the first Bytes operand. */
static void expect_allocation_order(void) {
    size_t count = SPX_FUSED_SELECTED_COUNT;
    assert(census.calls == 5 && census.mallocs == count);
    assert(census.bytes[0] == sizeof(spx_fused_frame));
    assert(census.bytes[1] == count * sizeof(spx_fused_row));
    assert(census.bytes[2] == count * sizeof(spx_fused_slot));
    assert(census.bytes[3] == SPX_FUSED_VIEWS * sizeof(spx_fused_borrow));
    assert(census.bytes[4] == SPX_FUSED_SELECTED_EXTENT);
    assert(!strncmp(census.order, "ccmccc", 6));
    if (count > 1) assert(census.order[6] == 'm');
}
static int oracle(spx_fused_issuer *issuer) {
    for (unsigned round = 0; round < 3; round++) {
        census_reset();
        invocation call;
        accounts held;
        open_accounts(&held, SPX_FUSED_SELECTED_METADATA);
        uint64_t before = issuer->next;
        int64_t result = -1;
        spx_status_token status = SPX_STATUS_SUCCESS;
        assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_PUBLISHED);
        assert(result == SPX_EXPECTED_RESULT && status == SPX_STATUS_SUCCESS);
        assert(held.ledger.debited == SPX_FUSED_SELECTED_METADATA && !held.ledger.failed);
        assert(held.payload.events == 1 && held.payload.cumulative == SPX_FUSED_SELECTED_EXTENT);
        assert(!held.payload.held_events && !held.payload.held_bytes && !held.payload.failed);
        expect_allocation_order();
        /* Every String, child, table, backing and the frame settled first. */
        assert(!census.live && !issuer->active && issuer->next > before);
    }
    return 0;
}
static int one_short(spx_fused_issuer *issuer) {
    census_reset();
    invocation call;
    accounts held;
    open_accounts(&held, SPX_FUSED_SELECTED_METADATA - 1);
    uint64_t before = issuer->next;
    int64_t result = -1;
    spx_status_token status = SPX_STATUS_SUCCESS;
    assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_REFUSED);
    /* Refused before any effect: no allocation, String, claim or hold. */
    assert(result == -1 && status == SPX_STATUS_SUCCESS && call.context.status_arena.length == 0);
    assert(held.ledger.failed && !held.ledger.debited && issuer->next == before && !issuer->active);
    assert(!held.payload.held_events && !held.payload.events);
    assert(!census.calls && !census.mallocs && !census.live);
    /* The refusal is sticky for the held account; a fresh exact account succeeds. */
    assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_REFUSED && !census.calls);
    census_reset();
    open_accounts(&held, SPX_FUSED_SELECTED_METADATA);
    assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_PUBLISHED);
    assert(result == SPX_EXPECTED_RESULT && !census.live);
    return 0;
}
static int claim_refusals(spx_fused_issuer *issuer) {
    invocation call;
    int64_t result = -1;
    spx_status_token status = SPX_STATUS_SUCCESS;
    accounts held;
    open_accounts(&held, SPX_FUSED_SELECTED_METADATA * 4);
    /* Overlap: the held issuer already grants another active domain. */
    static const spx_fused_domain other;
    census_reset();
    issuer->active = &other;
    assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_REFUSED);
    assert(issuer->active == &other && !held.ledger.debited && !census.calls);
    issuer->active = NULL;
    /* Exhaustion: refuses before any debit and never wraps. */
    issuer->next = UINT64_MAX - 1;
    assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_REFUSED);
    assert(issuer->next == UINT64_MAX - 1 && !held.ledger.debited && !census.calls && result == -1);
    /* The one ordinary payload account has no room for one more backing event. */
    spx_fused_issuer fresh = {0};
    assert(spx_fused_issuer_init(&fresh));
    accounts full;
    open_accounts(&full, SPX_FUSED_SELECTED_METADATA);
    assert(spx_fused_payload_debit(&full.payload, SPX_FUSED_EVENTS, 0));
    assert(invoke(&call, &fresh, &full, &result, &status) == SPX_FUSED_REFUSED);
    assert(full.payload.failed && full.payload.events == SPX_FUSED_EVENTS && !full.ledger.debited);
    assert(!census.calls && !fresh.active);
    return 0;
}
/* Ordinary Vec allocation failure keeps its ordinary status; the admitted
   funding is not refunded and the unconsumed hold is released. */
static int vec_failure(spx_fused_issuer *issuer) {
    census_reset();
    census.fail_at = 2;
    invocation call;
    accounts held;
    open_accounts(&held, SPX_FUSED_SELECTED_METADATA);
    int64_t result = -1;
    spx_status_token status = SPX_STATUS_SUCCESS;
    assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_LANGUAGE_FAILURE);
    assert(status != SPX_STATUS_SUCCESS && result == -1);
    expect_status(&call, "semaprax.vec.v1", 3);
    assert(held.ledger.debited == SPX_FUSED_SELECTED_METADATA);
    assert(!held.payload.events && !held.payload.held_events);
    assert(!census.mallocs && !census.live && !issuer->active);
    return 0;
}
/* Suffix arithmetic failure with a live loan: the loan ends, the plan exit
   finalizes owners in its canonical order, the backing is released once and
   no result is published. */
static int suffix_failure(spx_fused_issuer *issuer) {
    census_reset();
    invocation call;
    accounts held;
    open_accounts(&held, SPX_FUSED_SELECTED_METADATA);
    int64_t result = -1;
    spx_status_token status = SPX_STATUS_SUCCESS;
    assert(invoke(&call, issuer, &held, &result, &status) == SPX_FUSED_LANGUAGE_FAILURE);
    assert(result == -1 && status != SPX_STATUS_SUCCESS);
    expect_status(&call, "semaprax.arithmetic.v1", 1);
    assert(held.ledger.debited == SPX_FUSED_SELECTED_METADATA && held.payload.events == 1);
    assert(!census.live && !issuer->active);
    return 0;
}
/* The ordinary call-depth guard fails before any program effect. */
static int call_depth(spx_fused_issuer *issuer) {
    census_reset();
    invocation call;
    accounts held;
    open_accounts(&held, SPX_FUSED_SELECTED_METADATA);
    memset(&call, 0, sizeof(call));
    assert(spx_context_init(&call.context, UINT64_C(1), call.entries, UINT32_C(1), NULL, NULL, NULL));
    call.context.call_depth = SPX_MAX_CALL_DEPTH;
    int64_t result = -1;
    spx_status_token status = SPX_STATUS_SUCCESS;
    assert(spx_fused_selected_entry_v1(&call.context, issuer, &held.ledger, &held.payload, &result, &status)
        == SPX_FUSED_LANGUAGE_FAILURE);
    expect_status(&call, "semaprax.runtime.v1", 1);
    assert(result == -1 && !held.payload.events && !held.payload.held_events);
    assert(census.calls == 1 && !census.live && !issuer->active);
    return 0;
}
/* Physical allocation failure is fail-stop after its funding (process aborts). */
static int fail_stop(spx_fused_issuer *issuer, size_t cut) {
    census_reset();
    census.fail_at = cut;
    invocation call;
    accounts held;
    open_accounts(&held, SPX_FUSED_SELECTED_METADATA);
    int64_t result = -1;
    spx_status_token status = SPX_STATUS_SUCCESS;
    (void)invoke(&call, issuer, &held, &result, &status);
    return 0; /* unreachable when the boundary holds */
}
int main(int argc, char **argv) {
    const char *mode = argc > 1 ? argv[1] : "oracle";
    spx_fused_issuer issuer = {0};
    assert(spx_fused_issuer_init(&issuer));
    if (!strcmp(mode, "oracle")) return oracle(&issuer);
    if (!strcmp(mode, "one-short")) return one_short(&issuer);
    if (!strcmp(mode, "claim")) return claim_refusals(&issuer);
    if (!strcmp(mode, "vec-failure")) return vec_failure(&issuer);
    if (!strcmp(mode, "suffix-failure")) return suffix_failure(&issuer);
    if (!strcmp(mode, "call-depth")) return call_depth(&issuer);
    if (!strcmp(mode, "oom-frame")) return fail_stop(&issuer, 1);
    if (!strcmp(mode, "oom-table")) return fail_stop(&issuer, 3);
    if (!strcmp(mode, "oom-views")) return fail_stop(&issuer, 4);
    if (!strcmp(mode, "oom-backing")) return fail_stop(&issuer, 5);
    if (!strcmp(mode, "unsealed")) {
        invocation call;
        accounts unsealed;
        memset(&unsealed, 0, sizeof(unsealed));
        int64_t result = -1;
        spx_status_token status = SPX_STATUS_SUCCESS;
        (void)invoke(&call, &issuer, &unsealed, &result, &status);
        return 0;
    }
    return 64;
}
