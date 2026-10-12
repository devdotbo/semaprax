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
