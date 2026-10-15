# Borrowed Byte Slice For Traversal v1

Audience: language users, standard-library authors, and compiler contributors.

The additive [v2 private source profile](BORROWED-BYTE-SLICE-FOR-TRAVERSAL-V2.md)
adds narrowly scoped byte-to-byte nesting; the complete prior v1 contract and
receiver remain independent controls. V2 qualification is pending.

Status: source implementation authored for issue #780 (OPT780); **unexecuted**.
No build, test, runtime, fit, or performance evidence exists yet, and nothing
here claims hosted, physical-device, or production support. The focused
selectors below are authored, not passing.

This contract widens the existing source `for` statement of
[Owned Bounded Vec For Traversal v1](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md) to
one more exact source type: an immutable borrowed `Slice<u8>` of
[Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md). It adds no
syntax, intrinsic, host import, ABI, or public carrier.

## Exact source profile

```spx
for byte in view {
    body
}
```

`view` names one simple immutable binding whose exact type is `Slice<u8>`: a
`borrow Slice<u8>` parameter, or a local view from `bytes_as_slice`,
`array_as_slice`, `str_as_bytes`, `byte_range`, or an alias of one. Every such
view keeps its existing provenance, root-length bound, and external-root limit.
`byte` is a fresh immutable Copy `u8`. A computed source, a `let mut` view, a
shadowing or reassigned item, and a nested traversal in the body remain refused
exactly as for the Vec form (`SPX-T284`, `SPX-T209`, `SPX-U101`).

The loop snapshots `byte_len(view)` once, then visits indices `0usize` through
`len - 1usize` in ascending order, exactly once each. The authored body keeps
its current lexical control: `continue` advances the counter exactly once,
while `break` and `return` leave without advancing it. Lazy operands, body
failure, sticky status selection, and cleanup follow the ordinary `while`
rules. No buffer or view is copied or materialized.

## Owner freeze

The traversal freezes the view's authenticated root, through any whole or
subrange alias, for its entire lexical extent under its own loan. Moving,
consuming, renewing, or assigning that owner in the body is refused
(`SPX-T265`). The freeze does not rely on the view's own alias loan: the body
usually never names the view, so last-use expiry inside the body would
otherwise reopen the owner while the next iteration still reads through the
alias. A parameter view has no local owner; its caller's loan already spans
the invocation.

HIR repeats the freeze twice more, independently of the source verifier and of
the view's liveness, so forged or detached HIR cannot bypass it:

- The validator census requires the view to carry byte-slice provenance. That
  provenance is itself replayed against a fresh derivation before validation
  succeeds. The census refuses any assignment or rebinding of the provenance
  root anywhere in the authored body (`SPX-H006`), for every root kind,
  including fixed arrays. Provenance is derived for monomorphic functions only,
  so a traversal lowered inside a generic instance is refused.
- Loan planning starts an ordinary `TraversalFreeze` loan before the complete
  lowered region and ends it only after the region. Its origin is the view's
  place resolved through every whole and subrange alias to its owner. A move,
  transfer, or mutation of an owned root anywhere in the traversal overlaps it
  (`SPX-H006`), and attached loan evidence without it is forged. Plan schema
  selection is unchanged from
  [Scoped Record Vec Traversal v2](SCOPED-RECORD-VEC-TRAVERSAL-V2.md):
  `semaprax.loan-plan.v2` without lexical exits, `v3` with them. Borrow-only
  and Copy roots retain no loan, exactly as for their views.

## Lowering

The resolver reuses the Vec `for` paths unchanged. A `for` at `s{index}`
becomes `s{index}.value`: `.s0` binds the `byte_len` snapshot, `.s1` the
`usize` counter, and `.s2` is the loop. The item at `.value.s2.body.s0` is one
*receiver*:

```text
match byte_get(view, counter) { Some { value } => value }
```

`byte_get` keeps its `Option<u8>` result. The receiver has exactly one arm,
`Some`, whose payload is the item. Because the counter is below the snapshot
of an immutable frozen view, `None` is inconsistent: it selects no arm and
reaches each backend's existing exhaustive-match invariant refusal. There is
no forged `u8` byte call, default byte, skipped element, or unchecked load.
The authored body sits at `.value.s2.body.s1.value`, so workspace call, type,
and statement-segment reconstruction and every stable `@id` are unchanged.

A loop whose body has a lexical exit carries the additive latch
`SourceForStep { kind: BorrowBytes, step }`, where `step` is the exact unit
advance.

## Owning versions

**Graph v83.** A program containing a byte receiver selects
`semaprax.graph.v83`, layered over every v81/v80/v79/v78/v77 fact. Only v83
spells this profile's additive values: the latch's
`"source_kind":"borrow_bytes"` inside `semaprax.lexical-control-flow.v1`, each
receiver match's `"exhaustive":false` (it selects only `Some`), and its
`traversal_freeze` loans. The legacy renderer refuses such a program
(`SPX-G410`, "requires Graph v83"). Frozen evidence routes refuse v83. The
semantic workspace manifest and the workspace source-schema selector register
the exact spelling. A program without a receiver keeps its exact prior schema
and bytes.

**Checked-HIR cache v8.** The HIR cache encodes the latch kind as
`SourceForKind` tag `3`; tags `0` through `2` are unchanged and tag `4` is
refused. The wire that may carry tag `3`, receivers and their freeze loans is
owned by `semaprax.project-checked-module-hir.v9`
([Project Semantic Cache v1](PROJECT-SEMANTIC-CACHE-V1.md)). No v7 payload is
decoded under v8, and decoded HIR is replayed by HIR validation either way.
The frontend canonical-AST cache is unchanged, because there is no new syntax.

## Partial `Option<u8>` consumers

The receiver is the only partial match HIR admits. Every consumer that could
observe it was audited:

- HIR validation's two exhaustiveness checks and the Graph `exhaustive`
  projection exempt only `partial_receiver`, the complete authenticated shape:
  Value mode, exactly one guard-free `Some { value }` arm over `core.option`,
  an untyped `byte_get` over a bare `Slice<u8>` place and a bare `usize`
  place, and an arm that reads exactly its `u8` payload. Any single deviation
  loses the exemption and is refused as an ordinary non-exhaustive or
  ill-typed match.
- The census binds each exempt receiver to one complete region.
- The interpreter refuses a match that selects no arm ("refutable match
  selected no arm"). Native C11 emits `spx_runtime_invariant_failure` after
  the arm chain. Ordinary Core-Wasm emits `unreachable` after the last arm.
- The fused construction lowerings for native, Core-Wasm and HIR, and the
  borrowed-scalar primitive replay, admit an `Option<u8>` match only with
  exactly two arms, `Some` and `None`, so they refuse a receiver. The fused
  interpreter refuses when no arm matches.
- Cleanup-plan Option handling applies only to `Try` operands, which a
  receiver is not.

## Independent validation

The spelling has no authority. `hir::validation::byte_traversal` replays every
fact from ordinary HIR: the `byte_len` snapshot over a bare `Slice<u8>` place,
the zero seed, the strictly ascending bound, a receiver over the same view at
the counter, the unit step, the `BorrowBytes` latch when present, and a body
that cannot rebind or assign the view, snapshot, counter, item, or receiver
payload. The receiver is the only partial match HIR admits. A per-function
census refuses any receiver that is not the item read of one such complete
region (`SPX-H006`), so hostile or cached HIR cannot use it elsewhere. Cached
HIR is replayed again after decoding.

## Construction accounting

The byte form constructs its `byte_len` snapshot and its receiver in place on
the shared paths; no Vec carrier is built and then discarded. It has eighteen
synthesized expressions, against the Vec form's sixteen. Raw AST has no source
type, so the workspace prebound keeps sixteen per `for`. No count is padded
and no quota is raised. Instead:

- The receiver's two further expression identities debit their own shared
  carriers before allocation, as every `ExpressionId` does.
- Before anything is constructed, the lowering debits the receiver's exact
  remaining retained backing on the active builder ledger. That is its five
  identity texts, its payload `ValueId` and name, the five declaration
  identities (`core.bytes.get`, the `Option<u8>` result type, and
  `core.option`, `core.option.some`, `core.option.some.value`), the result
  type's argument vector, and its boxed scrutinee, argument, arm and field
  vectors.
- A refusal is sticky, constructs nothing, and reports `SPX-H006`.

## Authored focused evidence

These selectors are authored and **unexecuted**; none has passed:

```sh
cargo test --locked -p semaprax --test language byte_slice_for_traversal
cargo test --locked -p semaprax --test useful_data byte_slice_for_runtime
cargo test --locked -p semaprax --lib hir::cache_codec::byte_traversal_tests
cargo test --locked -p semaprax --lib hir::validation::byte_traversal::tests
cargo test --locked -p semaprax --lib hir::resolve_for::bytes::tests
cargo test --locked -p semaprax --lib interpreter::byte_traversal_peak_tests
cargo test --locked -p semaprax --lib graph::evidence_schema
cargo test --locked -p semaprax --lib nested_cleanup_versions_are_closed_and_legacy_selection_is_unchanged
cargo test --locked -p semaprax --lib identity_prebound_carriers_include_traversal_statements_and_every_child
cargo test --locked -p semaprax --test project semantic_cache
cargo test --locked -p semaprax --test hir_module_boundaries
```

They cover:

- Canonical round-trip and exact lowering.
- Empty, singleton, all 256 byte values, subrange, owned-buffer, and String
  roots, checked against an independent manual loop and a Rust oracle on the
  interpreter, native C11 `-O0`/`-O2`, and Core-Wasm.
- `continue`, `break`, `return`, lazy operands, and a body failure.
- Owner move, renewal, and subrange-alias source refusals.
- HIR owner assignment, refused independently by the census and by the loan
  freeze. The freeze loan spans the whole region on the resolved owner, and
  attached evidence without it is forged.
- Detached views without provenance, nested receivers, and every single-fact
  deviation from the receiver shape.
- Hostile latch, step, seed, advance, index, default-`None`,
  detached-receiver, and counter-rewind HIR.
- AST/HIR codec transport round-trips with latch drift, the tag-`4` refusal and
  the v9 literal.
- Actual `ProjectFrontendCache` cold/warm full-revision parity for a mixed
  byte-traversal/row-loan provider and consumer, a current provider body edit,
  mutable-view refusal matching a cold build, and restoration after refusal:
  `semantic_cache::byte_traversal::warm_byte_traversal_cache_preserves_mixed_graph_and_current_provider_refusals`.
  This owner is authored and unexecuted; codec transport alone is not warm-cache
  evidence.
- Graph v83 selection, forged-schema refusal, and an unchanged Vec-traversal
  graph.
- The receiver's exact debited backing, its complete Drop, an exact limit, a
  one-short sticky refusal, and an up-front refusal that allocates nothing.
- On the interpreter, a traversal peak over its control that is identical at
  256- and 16384-byte owned roots (no view or buffer copy). Every runtime
  allocation, including the owned root, is released by the end of evaluation.
  A static Core-Wasm check finds no copy or grow instruction.
  Literal fixed-array let initializers reuse their exact planned expression
  slot as the binding slot; ordinary expression emission and its canonical
  transitions still run. Nonliteral Copy and assignment boundaries keep
  separate storage. Empty arrays have no payload to copy.

The native-Rust interop builder's own pre-resolve reservation
(`FOR_GENERATED_EXPRESSION_COUNT`, sixteen at the Vec suffix) does not cover a
byte receiver. That reservation alone does not establish deterministic rejection or
acceptance: live lowering also debits receiver storage. That route is not yet
supported or qualified.

## Nonclaims

This profile adds no iterator object, enumeration index, mutable or owned
element, `Bytes` or `str` source without an explicit view, computed source,
mutation during traversal, nested traversal, generic-instance traversal,
native-Rust interop builder route, public ABI, or performance claim. Its actual
gain is to be measured live later.

Checked-HIR cache compatibility is now v10 after the selective CopyDiscard
statement extension; the byte traversal wire and Graph v83 semantics above are
unchanged. See [finite owned-record discard loops](FINITE-OWNED-RECORD-DISCARD-LOOPS-V1.md).
