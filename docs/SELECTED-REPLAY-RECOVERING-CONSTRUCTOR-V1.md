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

The compatibility `Preparation::new` and `ReplayBudget::from_reservation`
wrappers still convert the inline reason into an allocating owned Diagnostic.
They are not zero-allocation or caller-funded report boundaries. `PaidReport`
and full outward report propagation remain unimplemented until an actual caller
owns and transfers the carrier and text payment. This neutral constructor adds
no ordinary lexical admission, G410 bypass, new ceiling or allowance.

Additional owning lib selectors under
`cleanup_plan::replay::selected_entry::tests::setup`:

- `inline_setup_zero_and_each_one_short_return_the_exact_sticky_ledger_without_allocating`
- `inline_setup_exact_preparation_preserves_first_prepaid_text_pointer_receipt_and_drop`

These controls observe setup and refusal through Drop, with independent literal
carrier requests, zero/one-short setup, first receipt and same text/ledger
pointers. They do not establish funding for the still-absent outer report owner.
