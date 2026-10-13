# HIR module permit directory v1

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

Qualification pending. Complete HIR validation and its independent core caller
prepare exact-current Program-bound storage outside the pure validator. This
extends their function directory with a sorted borrowed permit-reference Vec.
The combined requested backing is reserved once on the existing active builder
before allocating either vector. Empty permits add no allocation. Actual allocator
capacity mismatch/excess follows the same sticky fail-closed policy; the first
receipt is not replaced. Original literal limits and forecasts are unchanged.

Each function still checks declared effects in original order, then its unsafe
boundary, then independent lifecycle effects and remaining body/contracts.
Only set construction changes: repeated module permit BTreeSets are replaced by
binary searches of the held directory. Duplicate strings remain in its metadata
and retain old membership; no duplicate/declaration checks are removed. Program
and function identities remain the existing exact pointer-sealed directory.
Closure revalidation shares lookup data, never a HIR verdict. The old independent
core/reference route still builds the old per-function borrowed-string trees.

New lib owners:
`hir::validation::function_lookup::permits::tests::shared_permits_match_all_function_old_effect_diagnostics_duplicates_and_current_program`
and
`hir::validation::function_lookup::permits::tests::caller_paid_permit_directory_replaces_per_function_nodes_with_exact_short_sticky_and_drop`.
They compare complete old/current core diagnostics for a 64-function fixture,
first missing-effect and earlier signature ordering, duplicate/unsorted permits
and cloned Program refusal. One observer encloses real caller preparation,
all-function membership and Drop: exact function-reference plus permit-reference
backing; zero/exact/one-short and zero-unit retry preserve the same sticky owner.
The old tree observation uses actual std allocations and only the independent
literal minimum of one node per nonempty per-function set, not a guessed node
layout. Immutable Program, fixed ledger and observer metadata are excluded.

These are source controls, not execution evidence. This adds no source authority,
physical APIs in pure validation, public route, cache schema, forecast discount,
application fit or timing claim. Other validator/replay storage payment remains
open. Sorting and string comparisons are real work; no new work-meter claim is
made by this storage change.
