# Declaration identity dynamic migration design

Status: design only; unimplemented; not a versioned contract.

Audience: compiler contributors and reviewers.

This document proposes a migration path. Nothing in it is implemented, and
nothing in it amends `docs/DECLARATION-IDENTITY-BACKING-V1.md`. Current
product and API behavior is unchanged by this lane. OPT785 / issue785 remains open. No funding, closure, application fit, graph, cache or physical claim is
made here.

## 1. Boundary that must not move

`DeclarationId::new(impl Into<String>)` is a legacy public constructor. The
versioned spec makes it explicitly unfunded and infallible. Consequences:

- It consults no payer, active or ambient, and it cannot refuse.
- It cannot precharge an arbitrary opaque `Into<String>` conversion. The
  conversion runs inside the call, once, so the caller's input expression and
  any conversion scratch are already allocated before the constructor sees a
  `String`.
- The conversion may allocate extra temporary scratch and may return a `String`
  whose capacity exceeds its length. The constructor cannot know either in
  advance. After the call it sees only the `String`.
- It never reopens a refused ledger and never charges a zero ledger.

The design therefore adds no ambient payer, no new ledger, no cap raise, no
refund, no sentinel, no deprecation and no breaking API. Legacy `new` stays
byte-for-byte as it is. The source controls in
`src/hir/ids/legacy_constructor_boundary_tests.rs` pin the legacy behavior
independently. They are boundary controls, not closure evidence.

## 2. Seams reused unchanged

Only existing private seams are used:

| Seam | Meaning | Use |
| --- | --- | --- |
| `DeclarationId::compiler(CompilerDeclaration)` | closed enum of fixed `'static` spellings; allocates nothing | compiler constants only |
| `DeclarationId::paid_borrowed(&str, reserve)` | caller payer asked for exactly the shared backing before it exists; no temporary `String` | caller holds borrowed text |
| `DeclarationId::paid_owned(String, reserve)` | payer asked for exactly the shared backing; the caller's `String` stays live beside it until released | caller already owns and has paid for the `String` |

Rules:

- From-static is the closed enum only. No constructor accepts an arbitrary
  `&'static str`. Leaking text to obtain a `'static` would bypass the closed
  set and is rejected. A call family whose ids are not in the enum either
  gains enum variants through a reviewed change or uses `paid_borrowed`.
- The payer is a caller-owned exact payer: a `FnOnce(usize) -> Result<(), E>`
  closure over the caller's own reservation (for example the existing
  `IdentityReservation` or the replay budget's selected physical reservation).
  The constructor adds no second charge.
- The refusal `E` is returned unchanged. Nothing is allocated on refusal.
  Conversion into a report is the caller's outward boundary: the owner that
  already holds a fixed, pre-paid failure/report channel maps `E` into it. The
  identity layer builds no Diagnostic and no report `Vec`.
- No public callback API is added. The seams stay `pub(crate)`. Public
  builders keep their legacy signatures.

## 3. What an exact payment must cover

For every migrated construction the caller must account on the actual original
owner, not on a forecast:

1. Input owner allocation. Charge the real `String::capacity()` of any text
   the caller materializes, not just `len()`. Reserve the known requested
   capacity before allocation; inspect the returned owner capacity and fail
   closed on unexpected excess using the existing producer policy. The actual
   allocator request and owner capacity must be independently observed. An
   arbitrary conversion with unknown scratch cannot satisfy this route.
2. Shared backing: `DeclarationId::shared_backing_bytes(len)`. This is two
   reference counters plus the UTF-8 bytes, padded to alignment. It is the
   Arc control block and padding.
3. Overlap. With `paid_owned`, the owner `String` and the backing are live
   together, so the peak is capacity plus backing. The ledger must have
   admitted both before the second allocation begins.
4. Clones share the one backing and request nothing.

An already-owned, unpriced `String` cannot retroactively be claimed as funded.
`paid_owned` pays only the new backing. When a caller hands over such a
`String`, the acquisition cost was never paid, so the producer must migrate to
allocate it under a payer. Until then that site stays in the legacy unfunded
domain and must not appear in any qualified list.

## 4. Pre-conversion explicit formatting

Sites that build identity text with `format!` (for example the
`{declaration}#invariant` family) must pay before formatting:

1. Compute the exact byte length with a counting `fmt::Write` that stores no
   text. This is allowed only for compiler-owned, pure formatters over `&str`
   and integers. A stateful `Display` must not be used, because the counting
   pass and the writing pass could disagree.
2. Reserve exactly the input capacity on the caller's payer before making
   the String. Construct it with `String::with_capacity(exact)` and require
   the returned capacity to equal the requested capacity; unexpected excess
   follows the existing producer's debit-and-refuse policy and is separately
   observed. This is a proposed producer, not an already implemented helper.
3. Write only after proving enough capacity for every compiler-owned piece.
   Compare the written length with the counted length. A mismatch fails closed
   before publication. Do not allow a formatter to grow an unpaid owner.
4. Call `paid_owned` with the fully paid owned buffer. Its original payer
   reserves the backing before the backing allocation, while the input remains
   live. Input reservation or backing refusal returns through the caller's
   pre-funded inline error/report boundary and drops the partial owner.

`budgeted_format` charges per append but lets `String` growth exceed the
charged bytes, so it is not an exact input-owner payment. It is not reused for
this purpose. `budgeted_clone` followed by `DeclarationId::new` pays the text
length but not the backing. Its migration is `paid_owned` over the cloned
`String`, which pays the missing backing.

## 5. Concrete caller types and steps

The census is lexical. It counts tests, saved-oracle trees
(`borrowed_root_oracle`, `old.rs`), forged-input fixtures and macro-like
repetition. It is not an AST count of production calls. It also contains
`PersistentDeclarationId::new` hits (`review.rs`, `repair.rs`, `patch`,
`cli_driver.rs`), which are a different type and are out of scope. A production
call count must be produced from the AST before any step is scheduled.

Caller families visible in the census and the intended step for each:

| Family (representative files) | Input shape | Step |
| --- | --- | --- |
| Lookups that build a key only to query (`arc_zones.rs` strong_count, `scoped_tasks.rs` lookups, graph set removal) | `&str` | S1: query with `&str` through `Borrow<str>`; no construction |
| Closed compiler spellings (`prelude::*_ID`, `iterator_ops::*`, `list_ops::*`, `cleanup::*_LIFECYCLE_ID`, `byte_ops::*`) | `'static` const | S2: `compiler` with enum variants added per closed set; otherwise S3 |
| Stable ids copied from resolved declarations (`hir/resolve_program.rs`, `closure/resolve.rs`, `function_value/resolve.rs`, `record_invariants`, `declaration_index/*`) | `String` clone of an existing field | S3: `paid_borrowed` from `&str` under the resolver's `IdentityReservation`; no temporary clone |
| Workspace graph and retained validation (`workspace_graph.rs`, `validation.rs`, `retained_validation*`, `expected_projection*`, `owned_generics.rs`, `shape_identity.rs`) | `budgeted_clone` text or `&str` | S3 for borrowed; `paid_owned` over the clone where a prepaid clone exists |
| Formatted ids (`declaration_index.rs` `#invariant`, `hir/closure.rs`, `collection_outcome/*`, `owned_collection_record.rs`, `projected_string_view.rs`, `owned_text_record.rs`) | `format!` result | S4: pre-conversion exact formatting, then `paid_owned` |
| Decoders (`agent_runtime_v2/checkpoint/value.rs`, `resumable_effects/owned_frame/{codec,v2/checkpoint}.rs`, `interpreter/resumable/checkpoint.rs`, `native_executor.rs` hex decode) | validated wire text | S3 with the decoder's caller-owned payer; the decoder-local cap stays independent of the owning payer, as in the existing codec |
| Agent generation (`project/agent_lowering.rs`, `project/agent_linked.rs`, `agent_contract_facts*`, `agent_proposal/*`, `agent_interaction_schema/shape.rs`, `agent_lifecycle/{stages,rich_stage}.rs`, `agent_lifecycle_typed_carrier/projection.rs`) | stable-id strings carried by agent facts | S5 |
| `arc_zones.rs` constructors (`ZoneSpec::root/child`, `ObjectSpec::new`, `Op::*`) and `scoped_tasks.rs` constructors | public `impl Into<String>` | S6 |
| Everything under tests, oracles, forged fixtures | various | no migration; stays legacy by design |

Steps, in order. Each is independently reviewable and none is scheduled by this
document:

- S1. Replace construct-to-query with borrowed `&str` lookup. This is
  zero-allocation, observable with the existing allocator observer, and
  changes no stored identity.
- S2. Extend the closed enum only for sets with a reviewed fixed spelling.
  Source-authored or formatted text can never name a variant.
- S3. Move borrowed-text producers to `paid_borrowed` with the producer's own
  payer. The refusal type is the producer's existing inline error.
- S4. Convert formatting sites as in section 4. Each site needs its own exact
  request census.
- S5. Agent generation. The generated stable ids originate in
  `agent_lowering` and flow into resolved agent facts and lifecycle stage
  binding. The producer that first materializes each id's text must hold a
  payer. Downstream consumers then borrow or clone the shared identity, which
  is free. Propagation is producer to HIR agent facts to stage binding. A
  consumer that receives an already-owned unpriced `String` stays legacy.
- S6. `arc_zones` and `scoped_tasks` are evidence/model modules with public
  infallible builders and no ledger. Their ids are built before any model
  exists, so `ArcZonesModel::try_new` receiving existing `ZoneSpec` or
  `ObjectSpec` values cannot retroactively fund them. Any funding needs a
  producer-level entry owned by a caller that has a payer. Until such a
  caller exists they remain an explicitly unfunded legacy domain, and the
  public builders keep their current signatures.

## 6. Minimum gates before any step may claim funding

A step counts as funded only if all of the following hold for its listed
sites:

- A literal source control shows no `DeclarationId::new(` in the migrated
  files. The control names the files and covers nothing else.
- Exact and one-short requests are observed with the real allocator observer:
  the first identity backing request equals the independently computed backing.
  Borrowed construction refuses one byte short before allocating. For owned
  construction, test input acquisition and backing acquisition separately;
  backing refusal drops the already-paid input and allocates no backing.
- Zero retry after refusal stays refused, and the first sticky receipt
  survives.
- Peak overlap is observed and equals owner capacity plus backing on the
  original owner.
- Full Drop is observed: scratch, temporary and backing all return to the
  allocator.
- The refusal reaches the caller's outward fixed report boundary without
  allocating a new report on the refused ledger.
- Existing `paid_borrowed` and `paid_owned` exact/short controls stay
  unchanged.

## 7. Unsatisfied qualification

These remain unsatisfied by this lane and by the design above:

- Application: no application fit or end-to-end funding claim.
- Graph: workspace graph construction, retained validation and expected
  projection remain only partially covered by existing narrow controls.
- Cache: only the declaration-identity decode backing is paid. Other cache
  payloads, key construction and diagnostic rendering are not claimed.
- Physical: allocator observations are requested layout bytes, not malloc
  usable bytes or RSS.

Current product and API behavior is unchanged. Issue785 remains open.
