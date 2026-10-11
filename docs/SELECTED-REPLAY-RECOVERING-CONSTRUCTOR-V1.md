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
carrier reservation fails, nor change initial Preparation construction's error
channel. No ordinary scalar lexical authority proposal is applied here.

Owning lib selectors under `cleanup_plan::replay::selected_entry::tests::recovering`:

- `recovering_constructor_exact_work_backing_and_first_report_drop_keep_one_owner`
- `recovering_constructor_structural_refusal_keeps_paid_first_diagnostic_and_temporary_drop`
