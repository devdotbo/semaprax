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

The payload owner measures construction, actual lookup receiving and Drop,
with an independent literal request delta for a generic call. The full-driver
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
