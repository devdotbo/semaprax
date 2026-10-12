# HIR borrowed continuation paths v1

Iterative validation keeps each Enter frame's original owned expected path and
full NUL, owner/domain/length/path identity, uniqueness and type checks. Only
after this complete header succeeds does it borrow the exact verified path
suffix from that current immutable expression ID for continuation frames.
Checked subtraction and UTF-8 indexing use the verified byte length; a parsed
prefix, equal unrelated ID or guessed boundary never authenticates a child.

Child expected paths still use the unchanged formatting and special argument
path producers. The synthetic Bytes-field `.arg.0` call prefix stays owned in
the call continuation's Cow; ordinary call prefixes are borrowed. Scopes,
publication, traversal order, errors, identity encoding and current HIR checks
are unchanged. No reference escapes the validation call. This is a storage
change, not a cache verdict, depth limit, physical authority or new arena.

The exact real frame-layout assertion remains mandatory. The test-only owned
capacity observation counts only genuinely owned Enter/synthetic call paths;
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
