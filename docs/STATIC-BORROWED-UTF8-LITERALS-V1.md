# Static borrowed UTF-8 literals v1

Status: authored direct-source implementation for OPT #740; qualification pending.
The retained Workspace/Project route is still closed. This is not a completed
issue, hosted gate, physical allocation receipt, or public provider profile.

The adjacent form `str"priority"` denotes immutable UTF-8 with type `borrow str`.
An ordinary `"priority"` continues to construct an owning String. The contextual
identifier `str` remains an identifier elsewhere; `str "priority"` is not a
literal. Both literal forms use the existing decoding and canonical escapes.
Empty input, NUL, Unicode scalars, quotes, and backslashes retain their exact
bytes. There is no interpolation, normalization, implicit String conversion,
new builtin identity, or runtime allocation operation.

A direct literal can initialize an immutable borrowed-text local, be retained
through existing immutable local aliases, and be passed to an admitted
synchronous borrowed-text parameter. Named `str_as_bytes` views use the existing
checked range and loan rules. These descriptor-only operations can occur inside
an otherwise admitted loop. Ordinary borrowed results, aggregate storage,
mutable aliases, callbacks, suspension, and view escape remain closed.

HIR stores a distinct `BorrowedUtf8Literal` leaf with immutable, module-held
`StaticUtf8` backing. Its type is Str and ownership is Borrow. `SourceExprStatic`
provenance binds the actual literal expression and exact local descriptor,
byte length and range. The descriptor does not fabricate owning storage or
acquire a cleanup finalizer. Cache decoding reconstructs the immutable payload;
independent validation rechecks the type, ownership, backing and view extent.
A graph schema, literal-looking name, hash, or attached plan cannot grant root
or backend authority. Stale source graphs are replayed against current source.

Selected programs use prelude v16 and Graph v77. The graph's
`semaprax.static-borrowed-utf8.v1` inventory is descriptive, with `authority:false`.
The current full-Program graph route composes existing helper-renewal facts.
Legacy and detached/from-parts graph entry points refuse the new root with
SPX-G410, including uninstantiated templates. The retained Workspace/Project
selector also remains closed: it discards full source before its later schema
selection and does not yet retain an independently replayable literal-site
inventory. Normal authenticated Project adoption therefore remains an open
#740 obligation; a stored flag or signature alone is insufficient.

One literal may contain at most 65536 decoded bytes (source SPX-T314).
Independent mandatory HIR validation also sums every represented decoded static
literal, ordinary String literal and literal-format template, including generic
templates and concrete instances. If any static literal is present, this
conservative occurrence sum must be at most 65536; excess or forged HIR is
SPX-H006. It takes no guessed deduplication discount. The source verifier checks
the per-literal bound; aggregate pool admission is currently the independent
HIR boundary. Old programs do not select the new prelude or pool merely because
they contain owning Strings. The mandatory HIR walk gathers selection without
an additional absence cursor or heap inventory. Detached graph refusal performs
its own bounded, charged census; it has no current full-source authority.

The compiler reserves the actual immutable carrier and String backing before
allocation. The reference interpreter clones the held immutable reference when
evaluating a literal; it does not construct an owning String or copy the literal
payload on every evaluation. Static module bytes consume no invocation-input
budget. An actual host argument remains foreign input even if its bytes or
address happen to coincide with a literal. No compiler-side reservation or
logical allocation count is a physical runtime measurement.

Native lowering emits read-only constant storage and a length-aware descriptor,
with no owning constructor or drop. Its decoded occurrence bound is not a claim
about C terminators, alignment, linker deduplication, or physical constant bytes.
Internal aggregate Core-Wasm uses the immutable interval starting at 196608 in
four fixed pages; command lowering uses 393216 in seven fixed pages, disjoint
from output and transcript storage. An empty literal points at the interval's
base, including at the exact full-pool boundary. Internal reads authenticate the
selected interval; foreign-entry fences remain unchanged. Public scalar/text/
data adapters, opaque standalone String arenas, and selected-final-UTF8
certificates explicitly refuse static literals until their own profiles support
the root. No host permission, ABI, raw input limit, runtime counter or memory
cap is increased.

Authored owning definitions, not executed in this tranche:

- `hir::static_text::tests::`: source/canonical/current graph/cache round trip,
  stale graph, exact/+1 pool, backing exact/one-short reservation, forged view,
  immutable-local refusal, interpreter reentry and frozen-profile refusal.
- `static_utf8::` in the owned-data harness: native O0/O2 allocation interception,
  Core-Wasm immutable-pool reads, public-profile refusals, and the shared
  `static_utf8_writer.spx` helper before and inside loops.
- `wasm::aggregate::tests::static_text::`: exact pool and empty-literal placement.

Remaining completion obligations include retained source/HIR site replay and
Project adoption, current-source execution of all definitions, final producer
bindings, and the complete owning gate. No efficiency or cost improvement is
claimed without paired measurements.
