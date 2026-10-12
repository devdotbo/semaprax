# Borrowed cleanup replay call payloads v1

The production expression-fact collector retains references to the exact held
HIR argument slices, type-argument slices and concrete instance IDs. Invoke
retains one callable type reference; ByteRange retains three expression
references in source/start/end order. Callee IDs borrow held HIR or the existing
compiler operation identity. HostCommand alone retains the current owned
legacy constructor until the separately reviewed declaration carrier joins.

The map remains independently constructed. Its keys, preorder, duplicate and
late-depth failures, status checks and call-commit comparisons are unchanged.
Full argument identity (including its fingerprint), type equality and concrete
instance presence remain part of replay. RowLoan retains the original collector
classification (no call fact); this change grants no new operation authority.
References cannot outlive their held HIR owner. Selected and ordinary drivers
continue independently constructing their facts from the actual function.

All existing callee/instance/argument/type clone reservations and work charges
remain conservative. The actual map node payer uses the new CallFact layout.
No quota, forecast, cache schema or admission reduction accompanies borrowing.
The saved owned collector keeps its original complete payload construction and
capacity policy for independent output, diagnostic and allocation comparisons.

The payload owner measures payload clone construction, actual lookup receiving
and Drop, with an independent literal array request delta for a generic call
and a separately observed identity-clone baseline. Complete maps are built
outside that observer; no equal map-node layout is assumed. The full-driver
owner separately retains complete HIR equality and actual replay through Drop;
it does not assert that every allocation in that scope has a payer. Existing
selected exact/one-short, duplicate and late-depth owners remain executable
requirements. Whole-engine funding, diagnostic publication and application fit
remain open; this is a payload-copy reduction only.

Owning library selectors live under
`cleanup_plan::replay::expression_facts::tests::payloads::`:

- `all_operation_payloads_preserve_saved_owned_facts_and_exact_current_references`
- `borrowed_payloads_remove_literal_clone_requests_through_receiving_and_drop`
- `current_fact_payloads_survive_complete_replay_and_drop_before_hir_owner`

## Consumed local observation extensions

Sequencing still constructs one paid prefix copy for each suffix. Its local
`Observations` receiver now transfers that copy into an ordinary or selected
sequence node instead of cloning its tail a second time before dropping it.
Borrowed suffix tails still copy and retain their original funding. The moved
tail keeps its original reservation; only the physically absent head clone
buffer loses its clone reservation. There are no refunds or work reductions.
Node kind/order, ordinals, shared prefix identity, selected destruction edge
registration and suffix-copy reservation order remain unchanged. Flat extension
retains its original growth/append route. Consumed locals are unpublished on
error; this route does not mutate a caller-owned borrowed observation value.

The complete prior borrowed extension/freezer implementations remain cfg(test)
oracles, including original work and physical refusal order. New controls under
`cleanup_plan::replay::skeleton_work::dag_storage::tests::tail_moves` compare
complete node shapes and errors, all empty/nonempty/shared-prefix combinations,
actual moved tail receiving and borrowed suffix retention, literal clone request
and cumulative debit differences through Drop, exact/one-short sticky funding,
and complete ordinary plus factored cleanup and hostile commit replay. Original
parameter-view, merge/union equations and application corpora are retained.
This source followup awaits consolidated qualification and establishes no
application fit, runtime efficiency or live-agent token ratio.
