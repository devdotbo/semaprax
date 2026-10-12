# HIR borrowed continuation paths v1

Iterative validation retains each Enter frame's exact expected path as a borrowed
parent plus a static suffix and up to two decimal indices. It preserves the
full NUL, owner/domain/length/path identity, uniqueness and type checks. Only
after this complete header succeeds does it borrow the exact verified path
suffix from that current immutable expression ID for continuation frames.
Checked subtraction and UTF-8 indexing use the verified byte length; a parsed
prefix, equal unrelated ID or guessed boundary never authenticates a child.

Child expected labels stream the original bytes and checked byte length into
identity comparison. Contract roots and local binding probes use the same label.
Call continuations borrow the authenticated parent; child construction replays
the exact String-slot, fused Bytes-field and literal-format argument prefixes
from the current expression shape. Record-pattern traversal retains its existing
owned nested path storage, rendering only at that receiving boundary. Scopes,
publication, traversal order, diagnostic priority and current HIR checks remain
unchanged. Expected label allocation/refusal points disappear; diagnostic text,
owned literal builders and their refusal carriers keep their original owners.
No reference escapes validation and no child identity authenticates its own label.

The exact real frame-layout assertion remains mandatory. The test-only owned
capacity observation excludes the removed Enter/synthetic call path buffers;
static forecasts and literal builder quotas are not discounted here.

Owning lib controls:
`hir::validation::continuation_path::tests::borrowed_continuation_prefixes_preserve_recursive_bytes_header_priority_and_scopes`
and
`hir::validation::continuation_path::tests::borrowed_authenticated_prefixes_remove_actual_continuation_clone_payload_through_drop`.
The first preserves the saved recursive full diagnostic/identity/scope oracle,
including Unicode and empty roots, NUL/owner/length header forgeries, statement
indices 9/10 and 99/100, sibling and late-error state. Existing direct-child,
failed isolated branch/lazy/match and depth512 publication controls remain
required. The second observes actual current-ID borrowing versus the original
String clone through Drop at 1/32/1000 repetitions, with exact literal requested
payload counts, monomorphic/generic encodings and current backing pointers.
Source/IDs are prepared outside that observer. This narrower clone boundary
makes no whole-validator allocation, funding or application-fit claim.

Qualification is pending until these owning controls execute.

Additional source-authored controls (unexecuted):
`hir::validation::path_label::tests::borrowed_labels_match_exact_owned_bytes_without_allocation_or_debit`
checks streamed byte length and monomorphic/generic identities, forged trailing
bytes/lengths, decimal boundaries and Unicode roots, with zero live debit and an
actual allocation-through-drop comparison against the retained owned formatter.
`hir::validation::path_label::tests::borrowed_child_labels_preserve_both_complete_saved_walkers_and_header_priority`
compares full diagnostics, expression/value membership and scope against both
complete saved iterative and recursive walkers over successful and hostile HIR.
The depth512/current-cursor controls, whole-validator observations, literal builder
quotas and static forecasts remain required and unchanged. This source checkpoint
makes no measured reduction, application-fit or execution claim.

`hir::validation::path_label::tests::streamed_argument_labels_preserve_all_original_special_prefixes`
compares current Call/String-slot, LiteralFormat and both VecFieldRead shapes
against the original argument-path producer. The removed-buffer refusal control
is `hir::validation::path_label::tests::removed_label_buffer_refusal_retains_the_owned_formatter_control`.
The complete new owner and test module are joined into
`tests/hir_module_boundaries.rs`'s authority and formatting source audit.
