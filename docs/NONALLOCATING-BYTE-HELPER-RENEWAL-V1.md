# Nonallocating Byte Helper Renewal v1

This additive implementation profile addresses OPT #737. Owning regression
bodies are authored; execution and efficiency qualification remain pending.
It introduces no operation, allocation exemption, capacity increase, public
ABI or generated-name privilege.

An ordinary monomorphic, effect-free helper may return `Bytes` while accepting
exactly one `own Bytes` parameter. Other parameters are existing Copy scalars
or Copy records/variants, or authenticated borrowed Slice/str/record/Vec
carriers. Existing source and HIR type, loan, layout, field and origin checks
remain authoritative. Borrowed projected views and fixed-array slice views
use their ordinary provenance; they cannot alias the transferred byte owner.

The helper's complete body, requires and ensures, and every directly called
ordinary helper are inspected independently in source and typed HIR. The
closure must be acyclic. Each proof has at most 256 combined expression/function
cursor frames and 1,000,000 examined-node, child-prefix and callee-lookup work
units. Full fixed cursor scratch is reserved before use. Missing declarations,
indirect calls, effects, yields, closures, lexical exits and proof-bound
exhaustion fail closed. No source or attached cleanup profile substitutes for
this independent proof.

Allowed operations are existing scalar arithmetic/conversions, Copy products,
borrowed byte/text reads and views, existing byte stores, Copy Vec length,
capacity and element reads, and authenticated Vec field reads. A literal
`vec_field` selector is static proof data, not a runtime String allocation.
Bytes copying or zeroed allocation, String construction (including literals,
formatting, numeric text, UTF-8 materialization and copying), Vec construction,
clone/reopen and every unknown operation are refused within the closure.
Existing byte fill admission still governs the helper's named same-owner stores.

`buffer = helper(..., buffer, ...)` renews an available mutable whole `Bytes`
binding when the helper's sole owning parameter consumes exactly that binding.
It is admitted inside ordinary bounded loops and outside loops. All arguments
still evaluate left to right; ordinary loan replay forbids an overlapping view
or second transfer. The old record renewal restriction to bounded loops is
unchanged. Reservation preserves the existing cleanup position; failure keeps
the selected status and settles actual staged owners. Postconditions protect
the provisional result before non-result cleanup and publication.

Only functions carrying helper-renewal sites select cleanup v19. Its existing
canonical reservation/renewal vectors are independently rederived; they are
never sorted, repaired or inferred downstream. Lexical functions additionally
retain their exact lexical-control-flow v1 certificate, continuations, resume
domains and loan v3 facts. Helper-only functions have no lexical certificate.

Graph v76 composes all existing graph facts with exact declaration/site/binding
updates under `semaprax.byte-helper-renewal.v1`. Complete typed HIR is required
to authenticate the transitive closure; frozen from-parts and evidence routes
remain closed. Programs without this profile retain their prior schemas/bytes.

Owning definitions are `loop_calls::byte_writer::tests::` and
`owned_data::byte_helper_renewal::`. They cover nested helper closure proof,
ordinary/loop reservation, contract allocation and recursion refusal, hostile
actual-owner substitution, canonical and graph projections, physical native
O0/O2 and private Core-Wasm settlement, and protected-result postcondition
failure. Definitions are not current-source execution evidence.

Compiler census cost is selective at the module signature level: a module with
no `Bytes` result plus `own Bytes` parameter signature skips the new binding
cursor. A module containing a potentially relevant signature performs the
bounded per-function census even when that particular function has no proven
helper-renewal site. Prior schemas and graph bytes remain unchanged in that
case; zero additional compiler work or scratch is not claimed. This body
census reserves 514 cursor slots, matching the ordinary cleanup traversal's
512-depth allowance and framing slots. The separate transitive closure proof
retains its stricter 256 combined function/expression frames.

The remaining OPT #737 compiler-efficiency follow-up is a single independently
proved, budgeted module closure/site index reused by all per-function consumers.
It must retain actual identity and lookup backing charges, fail closed on stale
source or typed facts, preserve old ordinary allocating-helper admission, and
avoid treating attached cleanup plans as authority. This index is proposed,
not implemented or measured by this batch.

The helper-site census stores sorted borrowed identity/binding references in
an explicitly grown Vec. Each replacement backing is reserved before its
allocation while the previous backing remains charged; duplicate keys replace
only the borrowed value and incur no growth reservation. Existing legacy
renewal tree maps remain separate, so helper sites introduce no opaque tree
nodes or copied identity backing. The checked census refuses before insertion
when its backing reservation fails.
