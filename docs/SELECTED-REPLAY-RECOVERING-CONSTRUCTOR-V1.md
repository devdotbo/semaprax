# Selected replay recovering constructor v1

This internal ownership adapter changes no source admission, graph family,
physical/work ceiling, or authority proof. Source qualification is pending.

`Session::try_from_preparation` borrows the exact existing preparation budget
while reserving the Session header and constructing its one borrowed callee
index, in the existing order. Success moves that budget into Session. Failure
returns the same failed Preparation and the first owned Diagnostic; temporary
index storage has dropped before return. The caller can observe and drop both
without rebuilding the index or restarting either allowance.

The existing `from_preparation` owned-Diagnostic wrapper preserves its prior
already-failed error path. Otherwise it delegates and drops the recovered owner
on refusal. Callee-index structural failures use the selected budget's already
paid refusal String with the same SPX-H006 code and first-report text.

The new test-only zero probe observes sticky state on the same owner. It grants
no authority or credit and does not claim that the unchanged owned-Diagnostic
retry APIs allocate nothing. First-report and recovered-owner Drop remain in
allocation observations; fixed two-member work and exact carrier requests are
independent expectations, rather than receipts reused as formulas.

This does not establish funding for an outward `Vec<Diagnostic>` whose first
carrier reservation fails. The legacy initial Preparation construction error
channel remains owned and unfunded by an outer report carrier. No ordinary scalar lexical authority proposal is applied here.

Owning lib selectors under `cleanup_plan::replay::selected_entry::tests::recovering`:

- `recovering_constructor_exact_work_backing_and_first_report_drop_keep_one_owner`
- `recovering_constructor_structural_refusal_keeps_paid_first_diagnostic_and_temporary_drop`

## Inline initial setup

`Preparation::try_new` now returns either a preparation or an affine failure
containing the exact failed `PhysicalReservation` and a Copy setup reason. It
allocates no Diagnostic, report Vec, or failure text on a refused reservation.
Successful setup preserves the existing reservation order: ledger Box, clone
cursor, then one bounded refusal String. Construction occurs only after those
requests succeed. A later first refusal moves that actual paid String into its
Diagnostic; it does not fund an outward report Vec.

The compatibility `Preparation::new` (now test-only) and
`ReplayBudget::from_reservation` wrappers still convert the inline reason into
an allocating owned Diagnostic.
They are not zero-allocation or caller-funded report boundaries. This neutral
constructor adds no ordinary lexical admission, G410 bypass, new ceiling or
allowance. The caller-owned report that it does not fund is described below.

Additional owning lib selectors under
`cleanup_plan::replay::selected_entry::tests::setup`:

- `inline_setup_zero_and_each_one_short_return_the_exact_sticky_ledger_without_allocating`
- `inline_setup_exact_preparation_preserves_first_prepaid_text_pointer_receipt_and_drop`

These controls observe setup and refusal through Drop, with independent literal
carrier requests, zero/one-short setup, first receipt and same text/ledger
pointers. They do not themselves fund an outer report owner.

## Caller-owned paid report (#781, authored, not yet executed)

Status: source-only. Neither the build, Clippy nor the selectors below have
run against this protocol; no allocation fit, performance or benchmark claim
follows from it.

`selected_entry/paid.rs` owns the crate-private protocol types. `ReportForm`
is one fixed outward code and literal message chosen by the actual caller.
`Acquired` is the affine carrier-and-ledger stage: `try_acquire` debits exactly one
`Diagnostic` slot and the form's literal bytes on the caller's selected ledger
before allocating a capacity-one Vec and an exact-capacity String. No function
id or formatted detail is published, so no further text is needed. A refused
debit returns the same rejected ledger and a Copy `SetupReason`
(`ReportCarrier` or `ReportText`) without allocating. `ReportCapacity` is a
post-allocation invariant refusal: the requested layouts were paid, both
buffers are dropped, and their debit stays spent. It is not an allocation-free
initial refusal and does not authorize an unexpected allocator capacity.

`PaidRefusal` is sealed; only `paid.rs` constructs it, so no caller can
present an unpaid or foreign Vec as paid. Its unpaid form retains the failed
ledger and reason. Its paid form retains the published carrier, a Copy
`Refused`, and exactly one ledger owner: the `SetupFailure` from
`Acquired::prepare` (`Preparation::try_new` on the ledger that paid the
carrier), the rejected bare ledger, or the failed `Preparation` recovered from
authentication, a constructor, `Session::try_from_preparation` or a Session
refusal (`Session::into_failed_preparation`, `Session::finish_paid`).
Publication writes into the acquired backing without reallocation. Transfer
marks the preparation failed and rejects its ledger without replacing a first
receipt. Neither form restores work, physical allowance or admission.

### Affine stage pairing

Acquisition returns one `Acquired`, never a `(report, ledger)` pair. Its
`prepare(function)` consumes both and returns the existing `Preparation`
with the carrier held internally. Authentication borrows that combined
preparation; it cannot take the report. `Preparation::into_paid_session`
consumes the entire stage and transfers its carrier with the cumulative
budget into the existing `Session`. Constructor refusal retains that same
combined preparation. `refuse`, `refuse_session`, `refuse_unprepared`, and
`finish_paid` accept no external report, preparation or physical ledger.
Moving a whole stage preserves pairing; no identity token, extra allocation,
new allowance, or report-to-ledger pointer lookup is introduced.

The neutral raw setup constructor is private to `selected_entry`. Only legacy
test constructors produce a preparation/session without a report. Production
cannot construct a detached carrier or split a paid stage; the report's fields
and publication method remain private. The actual local and imported callers
use these consuming transitions, rather than reconstructing a pair at finish.

### Inline refusal on the qualified route

`Refused` is a Copy, heap-free value: a `Cause` (`Mismatch`, `Budget`,
`Constructor`, `Failed`, `Setup`, `Session`), a static detail literal, and the
first static boundary stage. It has no `From` conversion into a `Diagnostic`
or `Vec`. Only the test-only `into_legacy_diagnostic`, and the test-only
`static_text::primitive::legacy` (G410) helper, render it.

The held `Preparation` refuses inline through `try_charge`,
`try_reserve_array`, `try_reserve_vec` and `construct`. A budget refusal from
`ReplayBudget` is the first paid Diagnostic: `budget_refusal` moves the
prepaid refusal text exactly once. The preparation retains it by move as its
`receipt`. A later refusal returns `Refused::FAILED`, or an empty, unallocated
String from a spent budget that is dropped, and never replaces that receipt.
`construct` lends a Copy-refusing debit. Work exhaustion spends the same
prepaid receipt and fails closed, even if a constructor ignores its refused
debit. The selected declaration constructors (`primitive_replay_metered`,
`append_flat_record_replay_metered`, `append_vec_replay_metered`,
`append_scalar_helper_replay_metered`, `helper_signature::admitted`, and the
test-only Option factory) now refuse with `Refused::constructor(literal)`
instead of allocating a `"... reconstruction refused"` Diagnostic.

Every authenticator on the route returns `Result<_, Refused>`:
`primitive::{members, current_module}`, `helpers::{authenticate,
authenticate_target, fact, calls, same_type}`, `helpers::local`,
`helpers::graph`, `helpers::types`, and `records::prepared`. None constructs a
Diagnostic, Vec or String before returning, and the caller transfers the
carrier with `Preparation::refuse(refused)`. The closure boundary
records its stage with `Refused::at` (one static pointer) instead of
formatting test help.

The test-only legacy owners `Preparation::{new, charge, reserve_array,
reserve_vec}` and `Session::{new, from_preparation, finish}` still allocate
`replay_error` on refusal or retry. Production no longer reaches them.

### Session refusal and fixed literal remapping

A `Session::try_from_preparation`, `validate` or `validate_scalar` refusal
returns the Session's own Diagnostic. `Preparation::refuse_session` moves it
into the failed preparation as the first receipt, unless an earlier receipt
is already held. It is never cloned, formatted or published. The route stops
at the first member refusal. It never validates or finishes a failed Session
again, so the legacy `already failed` retry Diagnostic never exists.

The outward report is a fixed literal remapping. The public `SPX-G410` form,
or the remapping caller's literal, is the carrier's prepaid exact owned text.
The Session text pointer is not transferred into that report. It stays with
the failed ledger owner and is dropped with it when the refusal is published.
`finish_paid` on a failed Session refuses inline with `Refused::FAILED` and
allocates nothing. On success it drops the unused carrier and returns the same
boxed ledger, with no refund or reset.

### Actual caller

The actual caller is `workspace_graph/source_schema/static_text/primitive.rs`
`replay_paid` and its imported `helpers::replay` twin. It creates the module's
ledger, acquires the carrier, then prepares. It transfers the carrier and the
exact failed owner on every later refusal. This contract is on the actual
production route, and no test-only envelope stands in for it.

Typed twins carry the refusal outward without remapping:
`source_schema::schema_with_proofs_paid` and `source_schema::all`, then
`WorkspaceGraphBuild::{into_change_view_paid, into_operation_view_paid,
source_graph_schemas_paid}` in `workspace_graph/paid_view.rs` through
`ViewRefusal`. Remapping callers instead pass the literal they publish:
Change base and candidate (`SPX-G184`), Structural Change (`SPX-G192`) and
Operations (`SPX-G200`) call `into_*_view_reporting(form)`. A paid carrier for
that exact form moves out unchanged, and no replay Vec is formatted after the
refusal.

`PaidRefusal::into_report(expected)` and `into_published` return
`Result<Vec<Diagnostic>, Unpaid>`. Only a paid carrier for exactly `expected`
moves out. An unpaid carrier (`UnpaidReason::Carrier`), a carrier paid for a
different form (`FormMismatch`, dropped and never relabelled) or a non-replay
view refusal (`Legacy`) reaches the Copy `Unpaid` boundary without
allocating. `Unpaid::into_unfunded` is the only operation that then allocates
the requested literal. It is a named compatibility edge, not a funded report.

These compatibility boundaries remain allocating and unfunded, outside the
qualified contract:

- `Unpaid::into_unfunded`, at the view wrappers' outward edge.
- `ViewRefusal::Legacy`: fingerprint and sidecar `SPX-G173`, builder
  structure including `reserve_builder_structure`'s own limit Vec,
  literal-site, helper and schema-selection refusals. A remapping caller drops
  it and publishes its literal through `Unpaid::into_unfunded`.
- The pass-through wrappers `into_change_view` (test-only),
  `into_operation_view` (nominal rename) and `source_graph_schemas`
  (Semantic Workspace preflight/replay). These use the public `SPX-G410` form.
  A paid G410 carrier passes through them. An unpaid one is allocated there,
  only through `Unpaid::into_unfunded`.
- Callers above `semantic_workspace` preflight and the Change, Structural
  Change and Operations entry points still return owned `Vec<Diagnostic>`.
  No public type changes in this batch. A v0.10 typed outward refusal would be
  a separate, specified API change.

The qualified imported-helper caller uses `pure_records_checked`: its local
limit is an inline Copy refusal and its charge error moves out unchanged. The
legacy purity wrappers retain their original owned-Vec contract for other
callers. The actual paid acquisition-to-purity-refusal-to-report-to-Drop owner
covers the carrier and ledger together; this removes the classifier's own
refusal allocation, without claiming whole-engine funding.

Remaining concrete gaps inside the route (not claimed as funded):

- Replay engine refusals inside the Session that are not budget refusals
  (`replay_error` in `require_entry`, `selected_callees`, `selected_context`
  and lexical/scalar replay) still allocate their own formatted Diagnostic.
  The route moves that Diagnostic as the receipt and never publishes it, but
  its allocation is unfunded.
- This protocol funds only the outward report carrier and reuses the setup's
  prepaid refusal text as the receipt. It makes no whole-allocation,
  application-fit or authority qualification claim for the replay itself.

Every public literal and code is unchanged. Each selected module ledger now
also carries one report carrier debit (`size_of::<Diagnostic>()` plus the form
length) inside the existing `MAX_CHANGE_BUILDER_BYTES` and the enclosing
active budget. No ceiling is raised. The ordinary lexical proposal stays
unapplied, and no admission, schema or authority is widened.

Owning lib selectors under `cleanup_plan::replay::selected_entry::tests::paid`:

- `paid_report_zero_and_one_short_return_the_exact_rejected_ledger_without_allocating`
- `paid_report_setup_refusal_publishes_original_backing_and_keeps_setup_ledger`
- `paid_report_constructor_refusal_keeps_the_exact_ledger_and_first_receipt`
- `paid_report_budget_refusal_moves_the_prepaid_text_once_without_allocating`
- `paid_report_constructor_exhaustion_spends_the_prepaid_receipt_and_fails_closed`
- `paid_report_session_refusal_transfers_carrier_and_retains_first_receipt_through_drop`
- `paid_finish_returns_the_same_ledger_and_a_failed_finish_allocates_nothing`
- `paid_report_remap_publishes_only_its_prepaid_form`
- `paid_stage_moves_keep_each_report_with_its_original_ledger_without_pairing_arguments`

Actual-caller selectors under
`workspace_graph::source_schema::static_text::tests::paid_report`:

- `actual_caller_zero_and_report_one_short_return_the_unpaid_ledger_without_allocating`
- `actual_caller_paid_setup_refusal_keeps_original_carrier_ledger_and_drop`
- `actual_caller_authentication_refusal_allocates_and_discards_nothing`
- `actual_caller_authentication_and_session_refusals_transfer_one_paid_report`
- `actual_caller_one_short_budget_refusal_retains_the_prepaid_receipt`
- `reporting_views_publish_the_callers_prepaid_form_and_keep_success_unchanged`

Updated owning callers of the typed constructors and authenticators:
`hir::declaration_index::primitive_replay::{flat_record, option_u8}::tests`,
`hir::borrowed_scalar::option_u8::tests`,
`cleanup_plan::replay::selected_entry::borrowed::tests`,
`loop_calls::byte_writer::resolved::selected` (census control), and the
`static_text::primitive::{helpers, records}` test modules (closure-graph
oracle, entry and prepared controls). None has been executed.
