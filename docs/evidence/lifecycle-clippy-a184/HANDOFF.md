# Minimal lifecycle production Clippy correction

Status: source-only; no Cargo, build, test or Clippy execution.
Audience: root final pin/metadata/Clippy integration lane.

From frozen a184d: mark only extend_funded and its recursive collect as cfg(test), after exhaustive literal caller confirmation that collection_tests, lifecycle_receiver_saved, completed/tests and completed/before_inline are all test-only. The real completed collector uses inline_path. Complete old bodies and the independent test receivers are retained.

Remove only the two needless & borrows on inline Active contains/remove. Both whole lifecycle auditors now strip exactly the two named cfg attributes, requiring a unique match for each, then compare the complete old source/hash exactly as before. The older completed archive also requires the exact pre-existing test-only Lifecycle operation_guard adapter to be inverted once; that named unique inverse repairs its already-stale guard without changing instrumentation or archived bytes. Original/archive hashes are unchanged. Update only current inline_path and inline_path/tests SHA bindings. No dead-code allow, declaration removal, payer/authority change, or semantic relaxation.

Source-only checks reconstruct both whole old lifecycle owners byte for byte, verify exactly the two inline borrow substitutions, check every current child SHA, rustfmt syntax and git diff whitespace. Root must rerun affected production Clippy and owning checks after integrating with actual pin/metadata changes; no green result is claimed here. Root checkout and its live diagnostic producer were never modified.
