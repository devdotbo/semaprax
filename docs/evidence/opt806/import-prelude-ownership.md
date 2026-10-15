# Imported receiver process-owner isolation

Audience: compiler contributors and reviewers.

Status: source-only repair preparation; executable confirmation pending.
Base: `58ee1bcad`; branch `codex/import-prelude-drop-58`.

The frozen selector-22 receipt retained 6420 bytes after complete imported
receiver Drop. The concrete process-lifetime owner in the source is
`src/prelude.rs:68`: `DECLARATIONS: OnceLock<Vec<TypeDeclaration>>`, whose first
initializer constructs all eleven declarations even when a scalar program uses
only Option/Result. Ordinary and explicit-identity HIR resolution both request
that owner via `declarations_for_program`; its contents outlive receiver Drop.
Parse and index setup outside these measurements do not establish that owner.
Source trace identifies this candidate precisely; the authored pointer receipt
must still execute before claiming the logged 6420 bytes are confirmed to it.

The new child measures a complete independent saved owning constructor, ordinary
HIR resolution, validation and Drop. While that observation is still active it
attributes every surviving requested Layout to actual allocation base pointers
owned by the static prelude declaration graph. The observer exposes read-only
pointer matching, changes no slot or metric, introduces no unsafe code, and
performs no subtraction. Unknown payload shapes and unmatched survivors fail.
The exact 6420-byte frozen cold receipt remains asserted on 64-bit hosts; an
already initialized process has zero cold receipt. Repeating the complete saved
receiver must independently free every observed allocation and leave zero live.

Integration deltas for root-owned conflicting files (not applied here):

1. In `borrowed_imports/tests.rs`, add `#[path = "tests/initialization.rs"] mod
   initialization;` next to `old`, and call
   `initialization::complete_original_receiver_before_measurement(&programs);`
   immediately after the `sources` binding in
   `complete_active_payer_resolution_and_drop_keep_exact_short_and_sticky_refusal`.
2. In `borrowed_imports/tests/full.rs`, call
   `super::initialization::complete_original_receiver_before_measurement(&programs);`
   immediately after the `sources` binding in
   `complete_recursive_owner_receivers_pay_drop_cache_and_short_refusal`.

Every subsequent new-receiver measurement, active-payer allowance, complete
resolution/validation/cache/Drop route, exact/short/sticky assertion and source
corpus stays unchanged. Both graph wrapper authentication repairs belong to the
parent's separate branch. No blind initializer call or proxy warmup is used.

Exact executable selectors after integration:

```text
workspace_graph::expected_projection::borrowed_imports::tests::initialization::complete_original_cold_prelude_pointer_receipt_and_repeated_drop_are_independent
workspace_graph::expected_projection::borrowed_imports::tests::complete_active_payer_resolution_and_drop_keep_exact_short_and_sticky_refusal
workspace_graph::expected_projection::borrowed_imports::tests::full::complete_recursive_owner_receivers_pay_drop_cache_and_short_refusal
```

Source checks only: rustfmt and `git diff --check`. No Cargo, build, test,
allocation receipt or runtime qualification was executed for this repair.
Graft returned no nodes and could not create its cache; exact failing paths and
bounded references were used. RTK.md is absent from the checked repositories.
