# Nominal and frozen resolver fixture ownership

Base: `020c60c18`. Only two existing test modules change. No production source,
full saved receiver, original hostile source, global limit, cap, or Drop observer
changes. Existing builder inventory includes both modules; no new Rust descendant.

The nominal original unbound `foreign.owner`/`u32::MAX` function-result tree remains
complete. Its unused outer nominal argument does not require substitution. Once
that tree is a declared field, exact substitution refuses it before active cycle
comparison in both current and complete saved classifiers. The test now checks
that original input as `OutsideProfile`. The original `Recursive` assertion stays
and is reached with an otherwise identical complete tree whose function result is
closed to `i64`. Existing reordered cycle/late-failure, missing-field, global-bound,
and original generic-Bytes source negatives remain unchanged.

The original literal `1+true` remains a separately registered owner. Both complete
expression receivers lower its full operand types, compare the entire HIR, and
retain their result through actual observed Drop at all four original roots.
Source verification selects exactly `SPX-T208`; the full checked-source diagnostic
vector equals the resolving entry point's refusal vector (all diagnostic fields,
order, and source path). The refusal corpus preserves its false assertions with
`1+missing`, which reaches the actual right-operand resolution refusal. Every
other original refusal source remains byte-identical. Both complete receivers,
observer accounting, all six diagnostic-field parity, and first-error cases remain.

Source-only verification: rustfmt and git diff --check. No Cargo, compilation,
tests, full gate, or current-head runtime success is claimed. Exact pending owning
selectors and saved-source file digests are in pending-selectors.json.
