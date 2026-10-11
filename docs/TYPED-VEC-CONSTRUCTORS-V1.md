# Typed Vec Constructors v1

Status: authored implementation; focused executable qualification pending.
Audience: SEMAPRAX authors and compiler contributors.

`vec_of<T>(item, ...)` constructs an already admitted Vec element profile with
an explicit homogeneous element type. Empty construction has capacity zero.
The existing capacity, carrier, allocation-event, payload, source depth and
work limits remain. Square-bracket byte arrays retain their existing meaning;
nonbyte square-bracket literals retain T262.

This is source constructor syntax, not a variadic ordinary Call. Execution
first calls existing `vec_with_capacity<T>(count)`, then evaluates each original
item once in order and executes one existing `vec_push<T>` at a time. Each push
stages arguments left to right and transfers them together at its ordinary
commit. Capacity failure evaluates no item; item or push failure retains the
selected status, settles staged owners and the committed prefix, and evaluates
no suffix. Postconditions, non-result cleanup and publication retain their
ordinary boundaries. No bulk allocator or String clone rule is introduced.

The parser forecasts generated statements, expression and type carriers,
type-clone backing, generated names, whole-file identifier inventory and
provisional-name replacement backing before constructing the chain. Its
cumulative generated-storage reservation is bounded by the existing 64 MiB
builder ceiling even on standalone parse routes. Active budgets are additionally
charged. Original item depth plus generated ancestors is checked against the
existing source depth; bounded borrowed cursors avoid prefix rescans. P220
refuses malformed type arity, count, reserved-name collisions and oversized
expansion before generated storage is published.

The AST marker retains element, literal count and its normalized ordinary
Block (appended cache tag 29). HIR carries element/count/statements/tail directly
on one node (appended cache tag 37), with the same expression/result identity
and one owning Temporary. It is not an owning wrapper around another Block.
Both source verifiers authenticate exact expansion and whole-function hygiene;
T318 refuses forged markers. Names skip all token identifiers, including later
text, and canonical hiding requires independent no-read/no-collision checks.
HIR authenticates builtin identities, exact element arguments, capacity,
previous-owner chain, binding modes and tail. Original items cannot read or
borrow any generated partial-vector owner. The borrowed internal-owner inventory is reserved before construction and sorted
with fallible in-place comparisons; item node visits and identity-byte
comparisons have separate bounded counters. Normal HIR/loan validation separately
checks child identities, types, staged calls and freezes.

Selective Graph v81 wraps the existing v80→v79→v78→v77 composition and retains
constructor metadata alongside ordinary children. Retained replay matches
current source sites and type identity independently; a matching ordinary HIR
Block grants no constructor provenance. Marker-free programs retain their
existing profiles. Frozen native interop/public export profiles explicitly
refuse the new syntax; ordinary supported private engines reuse Block execution
and existing calls without a new runtime operation or ABI.

Owning definitions: `hir::vec_constructor::tests::` covers source/cache/canonical
roundtrip, graph, one result carrier, source and HIR forgeries, hygiene, empty
capacity and old diagnostics; `owned_leaf_vec::vec_constructor::` covers
interpreter/native O0/O2/private Wasm execution, failure/cleanup and reentry;
`public_api_descriptor::vec_constructor::` covers frozen closure refusal.
The original ShiftSim15 and Catalog23 corpus bodies remain unchanged; their
committed eight-server and two-item examples adopt the constructor. Exact old
example sources remain independent controls. No token, turn, runtime or
physical-work gain is claimed before matched observations and executable gates.

Agent advice uses `help language author:vec-constructor` and exact diagnostic
lookups for P220/T318 through the compiler-checked quick-reference table. The
ordinary diagnostic-help producer regenerates its JSON companion after source
integration; help does not independently authenticate a constructor.
