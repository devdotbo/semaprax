# Private Nominal Layout Closure v1

Status: implementation and owning regression definitions authored; owning executable qualification is pending.

Audience: internal library authors, compiler contributors, and reviewers.

An ordinary checked library caller may import an explicit nongeneric root type
or an exact provider function whose signature carries that root, without importing
every private record reached through its layout. For example, an explicitly imported `Decoded`
result can carry `Report`, containing `Config` and `Vec<Row>`, while source names
only `Decoded` and its cases. This is internal layout reconstruction, not a new
source import, public descriptor, capability, graph edge, or lifetime grant.

## Authority and visibility

The current source-authenticated provider supplies each persistent declaration,
case, and field identity. Authored type names, constructors, and patterns still require explicit ordinary
type imports. A function-only signature root permits inferred local values to be
forwarded and borrowed field projections to be checked against its actual layout;
it does not publish the provider type name in the caller. Descendants must independently satisfy the existing
resource-free, invariant-free, nongeneric record shape, cycle, depth,
field, owned-leaf, and exact compiler-Vec checks. A nested variant, resource,
generic descendant, borrowed leaf, altered Vec identity, or unsupported record
vector element remains refused. Existing 64-depth, 4096-field, 256-owned-leaf,
vector, and builder-byte limits remain unchanged. Inferred roots in this MVP are
records or the exact two-case record-payload plus scalar code/offset/field
refusal outcome. Other outcome payload families retain their existing explicit
import route.

One invocation-local borrowed closure resolves only descendants reached through
an admitted explicit type root or exact imported function-signature root. It prefers an existing explicit alias. Otherwise it
mints a normal source identifier fresh against all authored identifiers in the
caller: imports, declarations, parameters and generic parameters, all type
references, local bindings and reads, constructor and field names, match and
nested record patterns, closures, and traversal aliases/indices. The MVP census
covers plain record/variant/function modules. Interfaces, classes, resources,
protocols, implementations, sessions, Agent declarations, and session-following
functions refuse inferred private layouts until their full name census is
implemented. Their existing explicit-import route is unchanged.

The new names are used only in copied synthetic declarations, field types,
function prototypes, and checking-only defaults. Authored AST and `module_uses`
are never rewritten. An unimported authored `Report` annotation, `Config`
constructor, or child pattern remains unresolved; a local declaration with that
name cannot capture the provider's child. No lexer or `SPX-S106` exception
recognizes a private prefix. Original graph import/reference edges remain
source-derived and contain no invented child import.

## Storage and replay

The borrowed provider references remain tied to the held immutable source set.
A bounded two-pass traversal reserves its real frame backing before allocation;
its single shared work budget covers both passes. The private entry directory
reserves its exact requested capacity, and each private name reserves complete
String capacity before writing. The identifier census uses precharged fixed
expression/type/pattern frame vectors and one shared work counter. It does not
serialize a whole Program or recursively format Debug output. Existing complete
provider-import sets bypass private-name and directory construction. If the
optional absence scan exhausts its bounded work, inference is disabled before
creating any aliases or directory; ordinary explicit-import validation still
runs and missing descendant aliases remain refused.

Synthetic clone forecasts include every copied private declaration, its field
and case metadata, string backing, identities, and rewritten aliases/defaults.
A growing synthetic type directory reserves the full replacement while the
previous directory is live. Private monomorphic field rewriting borrows field
types directly and does not build temporary field-reference vectors.

Each fresh build and source/cache comparison reconstructs the closure from its
current authenticated caller and providers. A stored private alias or closure
set is not authority. Actual provider bodies remain the linked implementation;
checking-only defaults cannot replace them. Existing independent HIR layout,
loan, cleanup, source-fact, cache, and selected-target validation remains required.
This does not widen public Project/package ABI admission or frozen source-less
`from_parts` authority.

## Owning definitions and qualification

`workspace_graph::expected_projection::private_layout::tests::` defines four
owning bodies: omitted private descendants with real provider evaluation and
exact Vec facts; full name hygiene and source visibility; provider identity,
layout and body drift; exact/one-short frame, directory and String reservations.
The `private_layout::authority::tests::` child adds two definitions: exact/one-short
borrowed authority storage and fresh/retained signature parity with missing,
stale and forged private fields. Proven complete provider type imports require
no new backing.
The original owned-function outcome gate retains complete HIR validation and
forged Vec-parameter refusal while checking descendant omission. The existing
text-collection hostile shapes remain refused. The workspace source-boundary
gate joins the new implementation and test children.

The `private_layout::function_tests::` child adds three definitions covering
function-only forwarding and borrowed nested String/Vec projections, exact root
and descendant identities, absent sibling/ancestor/source-name authority, stale
provider pointers, both signature parity paths, and exact/one-short storage.
The same linked source has interpreter, native O0/O2 and strict CoreWasm owning
success and failed-postcondition controls with repeated invocation and zero live
native/String/Vec storage; no production source or backend ABI is substituted.
Function-root admission consumes an invocation-held, precharged ownership and
signature-site directory. A shared bounded postorder walk computes each reached
nominal declaration's ownedness once; exact current function/type pointers bind
its signature sites to the immutable caller, provider set, and authored index.
The compiler Vec wrapper retains its own ownership and flat element checks.
Binary lookups compare stable function identity and signature ordinal, then
authenticate exact held signature pointers,
debit the held work remainder and allocate no new frames, cloned IDs,
sets, or backing. Exhaustion never grants a private fallback. Primitive signature
leaves use the old scalar/resource classification without a pending Vec.

`private_layout::owner_index::tests::` adds three owning definitions: differential
comparison with the independent old walk, wide logarithmic probes, and
allocation-free repeated queries;
exact/one-short complete construction, shared lookup work, and observed absence
of legacy fallback after refusal; and duplicate
provider names/aliases plus cold/reconstructed layout and retained-HIR drift.
One lazy invocation-local registry now holds each selected caller directory across
import validation, retention and cached/uncached forecasts, synthetic stub
construction, and core retries. Exact caller, provider-slice and authored-index
pointers bind every lookup. All caller directories share one work ledger and
sticky refusal; a failed query poisons subsequent derivation and queries.
Preparation reserves actual directory, frame, name and shared-ledger backing
before allocation. Its monotonic receipt is carried into every core attempt and
retention forecast, including retained overlap. The no-inference route creates
no registry backing. There is no global cache or serialized validation lease.

Entry, nominal-fact and signature-site ordering uses allocation-free fallible
heapsort. Identity-byte comparisons and swaps debit the same remaining work
before execution; refusal never publishes an incomplete directory. The four
`private_layout::registry::tests::` definitions compare held and independent
projections, exercise a real invocation with 64 signature sites, exact/one-short
preparation and overlap, independent ordering, and interleaved callers sharing
one sticky ledger. Direct forecast controls cover cached and retry modes;
existing owning cache/retry gates remain required for actual phase qualification.

These definitions have not been executed for this tranche. No current-head
interpreter/native/Wasm, cache, benchmark, source-size saving, or matrix promotion
is established by this document. The owning grouped qualification must run the
new bodies, existing imported-library and cache/replay controls, and selected
Project/backend gates before a support claim.

Related contracts: [Project v32 Nested-Outcome Command v1](PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md),
[Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md),
[Semantic Kernel v1](SEMANTIC-KERNEL-V1.md), and
[Architecture](ARCHITECTURE.md#internal-owned-data-libraries).
