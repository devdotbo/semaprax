# Consuming owned-record Vec renewal v1

Status: authored additive profile and owning definitions; qualification pending.
Audience: compiler ownership implementers and agents authoring consuming loops.

This profile composes the existing checked consuming iterator with an outer
mutable `Vec<Row>` whose element is an admitted monomorphic owned leaf record.
It does not raise collection, byte payload, allocation-event or fuel limits.
The owning [record iterator](OWNED-RECORD-ITERATOR-V3.md) and
[ordinary Vec renewal](VEC-LOOP-RENEWAL-V1.md) remain the underlying contracts.

## Admitted route

An ordinary `for own item in vec_into_iter<Row>(inputs)` may assign
`output = vec_push<Row>(output, item)` to an already live outer mutable Vec of
that exact record type. The ordinary argument route may instead compute a
fallible owned row, such as `vec_clone_at<Row>(later, 0usize)`. This does not
permit otherwise closed constructors or allocation in a source loop: every
argument retains its existing source/HIR/effect/ownership checks.

Independent classification recovers the actual initialized mutable Step slot,
exact Done/Yield cases and field identities, owned yielded row, remainder and
one `iter_next` latch from the generated iterator structure. The receiver is an
unprojected live outer owner; exact builtin declaration, type arguments, result
type and all argument ownership modes are checked. No generated display name,
attached cleanup schema, supplied graph fact or arbitrary While is authority.
Scalar, Copy-row, generic, primitive String/Bytes and non-push operations are
outside this additive classifier; their previous profiles remain unchanged.

## Reservation and failure

The existing cleanup-plan v15 protocol reserves the receiver's original history
position before evaluating the right side. Arguments stage left to right and
transfer only at the existing grouped call commit. Successful replacement is
published at the original position, preserving every unrelated live owner,
including an owner declared later. A failed argument or full-vector push settles
the staged receiver, current item, retained remainder and untouched owners in
canonical order, without replacing the selected status or publishing a result.
Cleanup vectors are neither sorted nor repaired downstream. No resource refund
or ownership grant is obtained from this profile.

Graph v79 is selective. Its ordered `consuming_owned_record_renewal` v1 inventory
records execution, condition, body, item, call site, receiver and exact element
type, and composes finite-scratch v78, static-text v77 and every prior selected
fact. Source/HIR/cleanup replay rederive the actual authority; graph evidence
cannot grant it. Frozen from-parts projections refuse the new route because they
lack the complete current declaration/source authority. Existing v15 cleanup
wire encoding and runtime/backend operations are reused unchanged.

The owning definitions cover source/canonical/graph/cache identity, stale record
layout and origin, forged renewal transitions/schema/site fields, the untouched
later cell, normalized index/capacity refusal, native O0/O2 owner settlement and
the strict Core-Wasm host. These definitions are not execution evidence. No
allocation, copy, token, wall-time or agent-cost improvement is claimed.
