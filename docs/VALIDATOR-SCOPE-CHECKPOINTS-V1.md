# Validator Scope Checkpoints v1

Status: private source implementation; OPT795 remains open. This document owns
storage and payment invariants, not execution evidence or a language extension.
[RFC 0001](RFC-0001.md) and [Resource ABI](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md)
retain their meaning. Independent hostile HIR validation acquires no authority
from resolver success, attached facts, a snapshot, or this directory.

Ordinary ownership checkpoints retain full ValidationBinding fields, ordered
lookup, original nonempty-prefix joins, freshness resets and nested publication
restoration under `hir/validation/scope_checkpoint/`. Their saved complete
iterative, recursive and contract validators remain the semantic oracles.

Generic-template Values retains ValueId ordering and exact replacement booleans.
Each node holds an existing identity Arc handle and a reference to the immutable
current HIR type; no fresh identity bytes, type clone, authentication index or
cross-invocation cache is created. Parameter, result, block, branch, contract,
match, loop and separate closure domains keep the original caller boundaries,
lookup order, collision order and complete expression/value identity checking.
Closure bodies start empty while captures are checked against their outer domain.
Contracts share a root but do not publish local bindings to sibling clauses.

The live invocation pays literal initial and local checkpoint control storage.
Every newly allocated AVL node pays its Rc reference-count header, actual node
layout and literal initializer stack before allocation. Shared paths construct
new nodes and rotation nodes transactionally; a refusal retains the original
complete root. A search path can mutate only when each node has exactly one
strong reference and no weak references. It prepays and allocates a missing leaf
before changing any node; insertion and rotations then move unique path nodes
and existing subtree handles without allocating or making a fallible payment.
Allocation-free replacements check sticky active refusal with a zero-byte
reservation before mutation; this adds no debit or allowance.
Published/shared nodes never mutate. Equal-key replacement retains the original
key identity. Drop releases carriers without refunding cumulative admission;
there is no separate quota, reset, forecast discount or allocation normalization.

The complete prior borrowed BTreeMap receiver and prior owned BTreeMap receiver
remain test-only independent policies, including complete snapshots and root
reads. The borrowed receiver isolates directory savings from earlier removal of
nominal type payload clones. Full ordered projections are observation helpers
constructed outside allocator windows, never retained production authority.

Qualification must compare complete environments, owned types, full symbolic
walks, identity sets, diagnostics and first-error order. Preserve existing
ordinary checkpoint, whole-subtree and original application owners. Additional
controls own monotonic/reverse and double-rotation insertions, snapshot forks,
replacements, nested contracts/closures, exact/one-short/sticky payment refusal,
unchanged publication on failure, actual allocation requests and final Drop.
Insertion-heavy setup and snapshot-heavy whole receiving lifetimes must be
observed separately against the prior borrowed directory; no gain is presumed.
Full application qualification and scoped Clippy remain mandatory in the
consolidated batch. Any remaining funding gap must be reported; private source
controls do not complete OPT795 or establish agent token savings.
