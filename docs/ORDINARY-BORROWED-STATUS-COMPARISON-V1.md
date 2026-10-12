# Ordinary borrowed status comparison v1

Ordinary independent cleanup replay reconstructs each required failure producer
from current HIR and compares its borrowed operands with the attached status
row. It no longer constructs an owned temporary status table, cloned propagated
callee text, or arithmetic-case vectors merely to compare and discard them.

The receiver retains the existing requires/body/ensures order, postorder child
walk, depth boundary, intrinsic classifications, and contract ordinals. Row
identity, lane, producer kind, operation, ordered cases, callee spelling, phase,
and ordinal must all match. Missing, extra, or reordered rows remain refused.
An observed mismatch is retained until the entire source walk finishes, so a
later invalid operation, unknown callee, or depth error keeps its original
priority over the final table mismatch.

This changes neither admission nor authority. Selected reconstruction and its
payer remain separate. Callable cleanup parameter construction and owned error
diagnostics retain their existing implementation; this change does not claim
whole-replay funding, allocation-free errors, or application memory fit.

The owning lib controls are
`cleanup_plan::replay::ordinary_status::tests::borrowed_status_comparison_preserves_owned_oracle_and_late_error_priority`
and
`cleanup_plan::replay::ordinary_status::tests::borrowed_status_comparison_removes_actual_owned_table_backing_through_drop`.
They retain the prior owned implementation as an independent oracle. The latter
observes the entire comparison and temporary teardown for 1/32/128 authored
call/arithmetic sites, with source and attached plans prepared outside the
observer. The old one-case Vec per addition supplies an independent minimum
allocation census. Callable nodes are outside that zero-allocation witness.

These controls are authored source until the owning qualification executes.
