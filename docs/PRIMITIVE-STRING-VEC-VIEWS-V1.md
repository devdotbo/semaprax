# Primitive String Vec views v1

Status: authored OPT-757 implementation batch; backend and application qualification pending. No allocation, runtime, or agent-efficiency result is claimed.
Audience: coding agents, compiler contributors and reviewers.

## Closed source and HIR contract

`vec_str_at(words,index)` reads an existing primitive `Vec<string>` slot as
`borrow str`. Source has zero explicit type arguments. The compiler-owned Call
identity is `core.vec.str-at`, with exactly `[String]` as its fixed HIR type
argument, two operands, no generic instance, and `Str/Borrow` result. This fixed
specialization is independently validated; an ordinary generic prelude stub
cannot replace its borrowed signature. Public return/storage/FFI permission for
borrowed strings is unchanged.

The first operand is an available whole named Own or Borrow Vec root. The
second is an ordinary Value `usize` expression. Temporary/call-result roots,
record projections, nominal substitutes, generic templates, shadowed builtin
identities, extra type arguments and forged result modes remain refused. Root
and index evaluate left to right under ordinary staging and failure rules. An
index expression cannot consume or mutate the already borrowed Vec.

`str_as_bytes(vec_str_at(words,index))` normalizes to the compiler-only
`core.vec.str-at-bytes` Call with the same fixed type argument and operands,
returning `Slice<u8>/Borrow`. The outer expression keeps its own identity; the
operand identities retain the inner source call paths `.arg.0.arg.0` and
`.arg.0.arg.1`. This identity has no authored source spelling. Named immutable
Str aliases compose with the ordinary `str_as_bytes` route.

## Lifetime, failure and extent

Both forms borrow the existing String allocation, produce no clone or owner,
and create scoped whole-Vec freeze authority. Simultaneous immutable views and
aliases remain valid; consuming, replacing, reserving, sorting, clearing or
moving that root while its views remain live is refused. Borrow-root parameters
need not create a new owner-root loan, but their current origin and scope are
still authenticated. Indexed byte provenance uses the distinct
`OwnedVectorStringSlot` root with exact current Call producer and index, never a
fabricated nominal field.

An out-of-range index selects existing Vec domain code 2 before publication.
Failure selection remains sticky; argument staging and canonical cleanup are
unchanged. Owned String extent is authenticated independently from foreign Str
extent. The separate existing `str_as_bytes` bound remains unchanged. No family,
capacity, fuel, foreign-memory, host quota or public export permission is added.

## Backend ownership boundary

The interpreter retains the existing immutable Vec backing for a view. At an
owning caller transfer it independently checks the current execution's exact
Before endpoint, all owner-root alias/child loans and the original producer;
only dead references are released. No String/Vec copy or physical finalizer is
performed by that release. A cheap presence flag grants no loan authority.
Native and private Core Wasm use the authenticated primitive String leaf
slot, preserving its original pointer/token and exact slot generation. Native
commits are matched in the current certified resume context. Fused and named
byte views keep the existing 128 KiB Slice boundary; plain owned Str extent is
not capped by that boundary. Foreign roots retain 64 KiB admission. Runtime
closure profiles and standalone/public borrowed export permissions are unchanged.

Authored owning selectors are `interpreter::string_slot::tests::` (two bodies)
and `owned_leaf_vec::string_slots::` (two bodies). They include same-source
NUL/Unicode/empty/repeated reads, last-use aliases before an owning user call,
lazy skipped uses, exact native allocations/private host copies, long Str,
exact Slice extent, provider corruption and Vec/2 cleanup/reentry controls.
They have not been executed in this implementation lane; executable
qualification and application adoption remain pending.

## Versioned projections

Only programs using these calls select Prelude v17 and Graph v80. The v80
wrapper preserves v79 consuming renewal, v78 finite scratch, v77 static text
and older facts. Frozen selectors refuse slot views. Retained source replay
reconstructs exact normal/fused callee sites, and ordinary HIR and loan replay
rederive signature, whole root, index identity, availability and generation.
No new AST or runtime expression node is serialized: existing Call encoding
carries the exact identities and type arguments. Byte provenance is rederived.

The owning frontend namespace is `vec_ops::string_slot::tests::`; backend
execution, physical zero-clone observations and original Catalog/ShiftSim
application qualification remain required before this feature is called implemented.
Catalog and ShiftSim now author primitive-slot reads, with exact pre-adoption
Tag/Server wrapper sources retained. Their existing 23/15-case owning bodies
execute both sources with unchanged expected channels and statuses. These
application definitions are unexecuted in this adoption lane; conversion
removal changes allocation ordinals and claims no physical or fuel result.
Existing `vec_get`, `vec_clone_at` and `vec_field` contracts retain their meanings.
See [Scoped Vec Field Reads v1](SCOPED-VEC-FIELD-READS-V1.md),
[Owned Leaf Collections v1](OWNED-LEAF-COLLECTIONS-V1.md),
[Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md) and
[RFC 0003](RFC-0003.md) for the owning failure, staging and replay rules.
