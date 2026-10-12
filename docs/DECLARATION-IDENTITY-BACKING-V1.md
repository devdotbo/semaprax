# Declaration identity backing v1

The private carrier has only valid text: CompilerStatic(&'static str) from a closed
compiler declaration enum, or Shared(Arc<str>). The actual 64-bit size/alignment and
Option footprint are asserted against the former String carrier. Equality, order,
hash, Borrow<str>, Display, Debug and codec wire consult exact text, never an address,
variant, fingerprint, budget or admission flag. Compiler constants grant no source,
prelude, layout or backend authority. Dynamic identities cannot select static text.

Legacy public new(Into<String>) remains infallible and explicitly unfunded. It does
not consult an ambient budget; borrowed input makes a temporary String before the
shared backing. Full dynamic construction migration remains open. No exhausted
unrelated budget can alter a pure expected-type query's declaration identity.

Private paid_borrowed reserves the shared backing on its supplied payer before
allocating and makes no temporary String. paid_owned additionally requires an already
owned/paid String; it reserves only the new backing while the old String remains
live, then releases that old owner. No refund or silent extra allowance occurs.
Refusal is Err outside the carrier, never empty text or a sentinel usable as a key.
The backing policy includes both counters, UTF-8 byte extent and alignment padding;
allocator observations pin that actual request separately from inline carriers.

Compiler Vec/Option/lifecycle/call constants use the closed enum. Existing physical
reservation routes and conservative text terms remain where retained by their
owners; static construction does not itself spend or reset a ledger. In particular,
flat-record lifecycle construction continues through ReplayBudget::reserve_bytes,
which must retain the current Selected/ActiveBuilder selection at integration.

DeclarationId codec preserves the former nested String tuple wire. Decode borrows
validated UTF-8 wire bytes, pays its decoder-local cap and then the caller's existing
active reservation before the backing allocation. Local capacity and owning refusal
remain separate; unchanged outer Diagnostic rendering is not claimed funded. The
private static representation is not serialized, so a decode returns ordinary
shared text without transferring compiler authority.

Graph set lookup/removal borrows str keys directly. Existing source association,
canonical replay, cache equality, failure priority, ceilings and forecasts are
unchanged. Shared clones allocate no identity payload; existing conservative clone
forecasts remain until a separate owner proves a reduction.

Authored owning controls cover old owned-String trait/wire parity, all empty/NUL/
Unicode distinctions, duplicate independent backings, static/shared equivalence,
borrowed and already-owned overlap, zero/exact/one-short refusal, sticky same-ledger
retry, last-owner Drop, decoder-local/owning cap separation, exhausted-budget pure
queries and graph key lookup. Their allocation scopes are narrow identity scopes,
not whole resolver/replay/graph funding evidence. Literal constructor source controls
cover only named migrated domains. No full OPT785 completion or fit claim follows.

Clone-only replay precharges price no DeclarationId text backing: both valid
static and shared variants retain their existing storage. Type-argument and
function-parameter Vecs, boxed result types, projection/case/flag directories,
renewal carriers and String-backed FunctionInstanceId clones remain charged.
The path-state clone census retains its original depth/work bounds. Zero-byte
clone requests still consult the same payer and cannot reopen a refused ledger.
Mixed retained-owner capacity, fresh construction, decode, source forecasts and
literal compiler/runtime ceilings are unchanged. Narrow authored observers cover
static/shared clone requests, nested type and conditional-state clone backing,
and real selected setup through exact/one-short refusal, zero retry and Drop.
These source controls await execution; they establish no application fit or
whole-validator/engine funding claim.

## Initial index explicit reservation seam

Initial index construction has a private callback route that requests only
actual shared identity backing bytes from the caller. It constructs the exact
borrowed source spelling internally; the callback cannot supply an identity or
verification authority. All ten original construction positions, metadata
mutations, duplicate/type/layout errors, and semantic diagnostic ordering remain
in the shared implementation. Identity refusal returns the exact caller error
inline and drops the partial index; it constructs no Diagnostic or report Vec.
The ordinary infallible wrapper remains explicitly unfunded.

The two owning controls compare the complete saved index and literal seventeen
initial identity requests, preserve empty/NUL/Unicode/duplicate behavior, and
observe full constructor/partial-index Drop. Their selected physical ledger pays
only identity backing; other index map/name/type storage and original owned
semantic diagnostics are not asserted paid by that ledger. Resolver activation
remains pending an actual caller-owned reservation and outward fixed report
boundary across verification and both invariant passes. No callback presence
changes source admission or makes an unsafe mutated index a verified subject.

### Private explicit resolver caller (identity-only, not public activation)

The private `hir::identity_resolution` route preserves source verification and
both invariant-resolution passes. It carries `Identity(E)` unchanged and uses
one caller callback for all initial index identities; source and semantic
errors retain their legacy owned channels. SourceIndex construction remains
before index construction in each pass. Ordinary public activation is described
separately below; this private callback API remains unchanged.

`bounded_output::identity_reservation::IdentityReservation` captures the
existing ActiveBuilder budget Rc without allocating a new budget. Before any
identity request the caller reserves its actual inline carrier, one Diagnostic
slot and the fixed 48-byte outward message. Setup refusal is Copy-only and
creates neither report nor diagnostic. Exact report capacity is checked before
publication; allocator excess is debited then refused, never normalized.
Identity reservations use that exact owner and cannot spend after its scope or
inside a nested budget. The existing first sticky receipt survives; publication
moves the paid String and Vec unchanged. No refund or new allowance is issued.

The two-pass tests observe partial-index Drop and original source error
priority. The report tests separately observe actual acquisition, first
identity refusal, zero/positive retries, publication and Drop, plus exact and
one-short setup. They do not assert payment for source verification, index
maps/names/type payloads, invariant cloning, resolver scratch, legacy semantic
errors or the complete public diagnostic pipeline. Public activation and those
remaining funding obligations stay open.

### Ordinary workspace module resolution (initial index identities only)

The actual non-hit module resolution in `workspace_graph` now acquires one
`IdentityReservation` on the resolved-core ledger before either the selective
or fresh route and forwards that owner through `resolve_checked` (owned input,
function reuse) or `resolve_source` (borrowed input), including both invariant
passes. Cache hits charge the stored full resolver residual without acquiring
another owner or building an index. Setup refusal stays Copy through the
`workspace_graph::identity_resolution` receiver and is adapted to the legacy
`Vec<Diagnostic>` layer only outside it; that outer graph/report layer remains
unqualified. Public `hir::resolve`/`analyze` stay explicit unpaid legacy.

Only the initial `DeclarationIndex` identities are paid. Still unpaid: the
fresh `DeclarationId::new` reconstructions in `Resolver::resolve` (entrypoint,
type, lifecycle, class method, interface, import, generic, function-template
and generic-owner identities), genuinely new agent/generated identities, other
initial-index callers (selected fused candidate, byte partition, compiler
prelude link), source/semantic diagnostics and operation constructors. No
whole-resolver funding claim is made.

### Ordinary analyzed caller activation (identity-only)

With an existing ActiveBuilder invocation and enough backing for the exact
identity/report setup, public `hir::analyze` now uses one captured affine owner
for both initial declaration-index passes. Original source verification still
runs first. Clause inventory, SourceIndex and compiler prelude discovery keep
their original order before acquisition and the first identity request. Each
identity is constructed internally from the exact borrowed current source text;
no callback can supply an ID or bypass ordinary validation. The second invariant
pass keeps the same owner and sticky first receipt.

A late identity refusal moves the already-paid capacity-one diagnostic Vec and
literal text directly into Analysis. It does not pop the report and allocate
another outward Vec. Successful construction also removes the former temporary
String copy before each shared backing allocation. Both original source warning
behavior and semantic error replacement remain unchanged.

The private acquisition failure is inline. Public zero/short-setup calls retain
their original legacy diagnostic route; allocator/capacity setup refusal also
falls back to that compatibility wrapper on the already-refused ledger. This
setup-only outward channel remains an explicit OPT781 gap, not a funded report.
Calls without an active owner preserve the old infallible index receiver. There
is no fresh ledger, additional allowance, refund, profile selection, source
forecast adjustment or physical identity sentinel.

Authored controls compare full current HIR/diagnostics with the saved ordinary
analysis across both invariant passes; a literal 29-request source census pins
removed temporary text and new report backing through complete Drop. Actual
public late refusals pin exact/one-short first identity requests and zero retries,
plus prepaid report pointers at the shared caller seam. Setup/source precedence
and the original nested/detached-owner controls remain. These are same-process,
source-only controls, not executed current-head or cold-static evidence.

This activation pays identity backing and its refusal report only. Source
verification, invariant cloning, SourceIndex proof storage, index maps/names/type
payloads, resolver continuations and legacy semantic diagnostics retain their
existing separate owners or open gaps. It establishes neither whole-resolver
funding nor an application fit claim.
