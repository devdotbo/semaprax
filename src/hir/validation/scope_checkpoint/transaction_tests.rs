//! Full coarse receiver, rich payload and actual allocation/debit owners.
use super::*;
use crate::test_allocation_observer::{measure, Metrics};
const LIMIT: usize = 64 * 1024 * 1024;
fn rich(count: usize) -> BTreeMap<ValueId, ValidationBinding> {
    (0..count)
        .map(|index| {
            let path = vec![PlaceProjection::Field(DeclarationId::new("scope.field")); 3];
            let mut moved_places = BTreeMap::new();
            moved_places.insert(path.clone(), Availability::MaybeMoved);
            let mut definitely_partial = BTreeSet::new();
            definitely_partial.insert(path);
            let mut active_loans = BTreeSet::new();
            active_loans.insert(LoanId(index as u16));
            (
                ValueId::new(format!("scope.binding.{index:04}")),
                ValidationBinding {
                    contract_local: index % 2 == 0,
                    mutable: index % 3 == 0,
                    ty: ResolvedType::Function {
                        parameters: vec![
                            ResolvedType::Nominal {
                                declaration: DeclarationId::new("scope.nominal"),
                                arguments: vec![ResolvedType::I64, ResolvedType::Bool],
                            };
                            2
                        ],
                        result: Box::new(ResolvedType::I64),
                    },
                    ownership: OwnershipMode::Own,
                    availability: Availability::Available,
                    moved_places,
                    definitely_partial,
                    active_loans,
                },
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum Sharing {
    Unique,
    Root,
    Descendant,
}
const ROTATIONS: [([usize; 2], usize); 4] = [([2, 1], 0), ([0, 1], 2), ([2, 0], 1), ([0, 2], 1)];
fn put(
    scope: &mut Scope,
    key: &ValueId,
    value: &ValidationBinding,
    coarse: bool,
) -> Result<(), Diagnostic> {
    if coarse {
        let insertion = scope.prepare_insert_coarse()?;
        charge::reserve(charge::binding(value))?;
        insertion.insert(key.clone(), value.clone());
        Ok(())
    } else {
        let insertion = scope.prepare_insert()?;
        charge::reserve(charge::binding(value))?;
        insertion.insert(key.clone(), value.clone())
    }
}
fn seeded(input: &BTreeMap<ValueId, ValidationBinding>, order: &[usize], coarse: bool) -> Scope {
    let mut scope = Scope::default();
    for &index in order {
        let (key, value) = input.iter().nth(index).unwrap();
        put(&mut scope, key, value, coarse).unwrap();
    }
    scope
}
fn hold(scope: &Scope, sharing: Sharing, key: &ValueId) -> Option<Rc<tree::Node>> {
    match sharing {
        Sharing::Unique => None,
        Sharing::Root => scope.root.clone(),
        Sharing::Descendant => {
            let root = scope.root.as_ref().unwrap();
            if key < &root.key {
                root.left.clone()
            } else {
                root.right.clone()
            }
        }
    }
}
fn held_matches(held: &Option<Rc<tree::Node>>, expected: &BTreeMap<ValueId, ValidationBinding>) {
    if let Some(root) = held {
        // Borrowed full projection; no heap observation/report directory.
        for (key, value) in views::TreeIter::new(Some(root)) {
            assert_eq!(Some(value), expected.get(key));
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn receiving_from(
    input: &BTreeMap<ValueId, ValidationBinding>,
    seed_input: &BTreeMap<ValueId, ValidationBinding>,
    order: &[usize; 2],
    index: usize,
    sharing: Sharing,
    coarse: bool,
    copied: bool,
    limit: usize,
) -> (bool, Metrics, bool, usize, bool) {
    // Input/seed/snapshot/source backing is outside this receiving observation.
    // Newly owned payloads/carriers, staged nodes and their complete Drop are inside.
    let mut scope = seeded(seed_input, order, coarse);
    let (key, value) = input.iter().nth(index).unwrap();
    let held = hold(&scope, sharing, key);
    let prior = scope
        .iter()
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut expected = prior.clone();
    expected.insert(key.clone(), value.clone());
    let source = Scope::from_prepaid(input.clone());
    let ((accepted, observed), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = if copied {
                    if coarse {
                        scope.copy_binding_from_coarse(&source, key)
                    } else {
                        scope.copy_binding_from(&source, key)
                    }
                } else {
                    put(&mut scope, key, value, coarse)
                };
                let accepted = result.is_ok();
                if accepted {
                    assert!(scope.iter().eq(expected.iter()));
                } else {
                    assert!(scope.iter().eq(prior.iter()));
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                held_matches(&held, &prior);
                let remaining = crate::bounded_output::active_remaining();
                drop(result);
                drop(scope);
                drop(held);
                assert_eq!(crate::bounded_output::active_remaining(), remaining);
                accepted
            })
        });
    (accepted, observed, refused, paid, receipt.is_some())
}
fn receiving(
    input: &BTreeMap<ValueId, ValidationBinding>,
    order: &[usize; 2],
    index: usize,
    sharing: Sharing,
    coarse: bool,
    copied: bool,
    limit: usize,
) -> (bool, Metrics, bool, usize, bool) {
    receiving_from(input, input, order, index, sharing, coarse, copied, limit)
}
#[test]
fn every_rotation_shape_unique_shared_and_mixed_retains_full_coarse_and_map_verdicts() {
    let input = rich(4);
    for (order, index) in ROTATIONS {
        for sharing in [Sharing::Unique, Sharing::Root, Sharing::Descendant] {
            for copied in [false, true] {
                let (ok, current, refused, paid, receipt) =
                    receiving(&input, &order, index, sharing, false, copied, LIMIT);
                let (old_ok, old, old_refused, old_paid, old_receipt) =
                    receiving(&input, &order, index, sharing, true, copied, LIMIT);
                assert!(ok && old_ok && !refused && !old_refused && !receipt && !old_receipt);
                assert!(!current.invalid && !old.invalid);
                assert_eq!(current.live, 0);
                assert_eq!(old.live, 0);
                assert_eq!(
                    current, old,
                    "same physical receiving requests for {order:?}/{sharing:?}/{copied}"
                );
                assert!(current.allocated <= paid && current.highwater <= paid);
                assert!(
                    paid < old_paid,
                    "literal constructors replace the coarse node allowance"
                );
            }
        }
    }
}
#[test]
fn every_paid_byte_boundary_keeps_full_prior_root_snapshots_and_owned_payload_drop() {
    let input = rich(4);
    for (order, index) in ROTATIONS {
        for sharing in [Sharing::Unique, Sharing::Root, Sharing::Descendant] {
            for copied in [false, true] {
                let (ok, wide, refused, total, receipt) =
                    receiving(&input, &order, index, sharing, false, copied, LIMIT);
                assert!(ok && !refused && !receipt && total > 0);
                let (ok, exact, refused, paid, receipt) =
                    receiving(&input, &order, index, sharing, false, copied, total);
                assert!(ok && !refused && !receipt && paid == total);
                assert_eq!(wide, exact);
                for limit in 0..total {
                    let (ok, observed, refused, paid, receipt) =
                        receiving(&input, &order, index, sharing, false, copied, limit);
                    assert!(!ok && refused && receipt && !observed.invalid);
                    assert!(
                        paid <= limit && observed.allocated <= paid && observed.highwater <= paid
                    );
                    assert_eq!(observed.live, 0);
                    assert_eq!(observed.allocated, observed.freed);
                }
            }
        }
    }
}
#[test]
fn equal_key_replacement_keeps_every_field_and_sticky_zero_payment_without_new_nodes() {
    let prior_input = rich(4);
    let mut input = prior_input.clone();
    let mut changed = input.values().nth(1).unwrap().clone();
    changed.contract_local = !changed.contract_local;
    changed.mutable = !changed.mutable;
    changed.ownership = OwnershipMode::Borrow;
    changed.availability = Availability::Moved;
    changed.active_loans.insert(LoanId(999));
    let path = vec![PlaceProjection::Field(DeclarationId::new("scope.changed")); 2];
    changed
        .moved_places
        .insert(path.clone(), Availability::Moved);
    changed.definitely_partial.insert(path);
    if let ResolvedType::Function { result, .. } = &mut changed.ty {
        **result = ResolvedType::Bool;
    }
    let key = input.keys().nth(1).unwrap().clone();
    input.insert(key.clone(), changed);
    let order = [0, 1];
    for sharing in [Sharing::Unique, Sharing::Root, Sharing::Descendant] {
        for copied in [false, true] {
            let (ok, observed, refused, paid, receipt) = receiving_from(
                &input,
                &prior_input,
                &order,
                1,
                sharing,
                false,
                copied,
                LIMIT,
            );
            assert!(ok && !refused && !receipt && !observed.invalid && observed.live == 0);
            let (old_ok, old, old_refused, _, _) = receiving_from(
                &input,
                &prior_input,
                &order,
                1,
                sharing,
                true,
                copied,
                LIMIT,
            );
            assert!(old_ok && !old_refused);
            assert_eq!(observed, old);
            if matches!(sharing, Sharing::Unique) && copied {
                assert_eq!(paid, 0);
                assert_eq!(observed.allocated, 0);
            }
        }
    }
    let source = Scope::from_prepaid(input.clone());
    let mut scope = seeded(&prior_input, &order, false);
    let prior = scope.clone();
    // Scope.clone would force the shared route. Compare before/after using a
    // map instead so this owner separately exercises a wholly unique root.
    drop(prior);
    let before = scope
        .iter()
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let (((), observed), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(!crate::bounded_output::reserve_active_required(1));
                let error = scope.copy_binding_from(&source, &key).unwrap_err();
                assert!(scope.iter().eq(before.iter()));
                assert!(!crate::bounded_output::reserve_active_required(0));
                drop(error);
            })
        });
    assert!(refused && receipt.is_some() && !observed.invalid);
    assert_eq!(paid, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
}
#[test]
fn complete_insertion_and_snapshot_lifetimes_keep_full_old_storage_without_normalizing_requests() {
    let input = rich(128);
    let mut current = None;
    for coarse in [false, true] {
        let (((), observed), refused, paid) =
            crate::bounded_output::with_limit_usage(LIMIT, || {
                measure(|| {
                    let mut scope = Scope::default();
                    for (key, value) in input.iter().rev() {
                        put(&mut scope, key, value, coarse).unwrap();
                    }
                    assert!(scope.iter().eq(input.iter()));
                    let mut snapshots = Storage::new();
                    for _ in 0..17 {
                        snapshots.push(scope.clone()).unwrap();
                    }
                    let (key, value) = input.iter().nth(64).unwrap();
                    put(&mut scope, key, value, coarse).unwrap();
                    assert!(scope.iter().eq(input.iter()));
                    drop(snapshots);
                    drop(scope);
                })
            });
        assert!(!refused && !observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(observed.allocated <= paid && observed.highwater <= paid);
        if coarse {
            let (first, first_paid) = current.unwrap();
            assert_eq!(observed, first);
            assert!(first_paid < paid);
        } else {
            current = Some((observed, paid));
        }
    }
}

#[test]
fn weak_boundary_keeps_complete_prior_root_on_every_refusal_without_claiming_fewer_requests() {
    let input = rich(4);
    let order = [2, 1];
    let (key, value) = input.iter().next().unwrap();
    let observe = |limit| {
        let mut scope = seeded(&input, &order, false);
        let weak = Rc::downgrade(scope.root.as_ref().unwrap());
        let prior = scope
            .iter()
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut expected = prior.clone();
        expected.insert(key.clone(), value.clone());
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = put(&mut scope, key, value, false);
                let accepted = result.is_ok();
                if accepted {
                    assert!(scope.iter().eq(expected.iter()));
                    assert!(weak.upgrade().is_none());
                } else {
                    assert!(scope.iter().eq(prior.iter()));
                    let retained = weak.upgrade().unwrap();
                    assert!(views::TreeIter::new(Some(&retained)).eq(prior.iter()));
                    assert!(!crate::bounded_output::reserve_active_required(0));
                    drop(retained);
                }
                drop(result);
                drop(scope);
                drop(weak);
                accepted
            })
        })
    };
    let ((accepted, wide), refused, total, receipt) = observe(LIMIT);
    assert!(accepted && !refused && receipt.is_none() && !wide.invalid);
    assert_eq!(wide.live, 0);
    for limit in 0..total {
        let ((accepted, observed), refused, paid, receipt) = observe(limit);
        assert!(!accepted && refused && receipt.is_some() && !observed.invalid);
        assert!(observed.allocated <= paid && observed.highwater <= paid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    let ((accepted, exact), refused, paid, receipt) = observe(total);
    assert!(accepted && !refused && receipt.is_none() && paid == total);
    assert_eq!(wide, exact);
}

#[test]
fn full_coarse_insert_adapter_and_existing_key_paid_boundaries_remain_independent() {
    let original = rich(4);
    let mut scope = Scope::default();
    for (key, value) in &original {
        scope.insert_coarse(key.clone(), value.clone()).unwrap();
    }
    assert!(scope.iter().eq(original.iter()));
    let mut changed = original.clone();
    let value = changed.values_mut().nth(1).unwrap();
    value.contract_local = !value.contract_local;
    value.mutable = !value.mutable;
    value.ownership = OwnershipMode::Borrow;
    value.availability = Availability::MaybeMoved;
    value.active_loans.insert(LoanId(888));
    for sharing in [Sharing::Unique, Sharing::Root, Sharing::Descendant] {
        for copied in [false, true] {
            let (ok, wide, refused, total, receipt) = receiving_from(
                &changed,
                &original,
                &[0, 1],
                1,
                sharing,
                false,
                copied,
                LIMIT,
            );
            assert!(ok && !refused && !receipt && !wide.invalid);
            for limit in 0..total {
                let (ok, observed, refused, paid, receipt) = receiving_from(
                    &changed,
                    &original,
                    &[0, 1],
                    1,
                    sharing,
                    false,
                    copied,
                    limit,
                );
                assert!(!ok && refused && receipt && !observed.invalid);
                assert!(observed.allocated <= paid && observed.highwater <= paid);
                assert_eq!(observed.live, 0);
                assert_eq!(observed.allocated, observed.freed);
            }
            let (ok, exact, refused, paid, receipt) = receiving_from(
                &changed,
                &original,
                &[0, 1],
                1,
                sharing,
                false,
                copied,
                total,
            );
            assert!(ok && !refused && !receipt && paid == total);
            assert_eq!(wide, exact);
        }
    }
}

#[test]
fn sealed_owner_refusal_precedes_binding_carrier_and_preserves_full_incoming_drop() {
    let input = rich(4);
    let mut scope = seeded(&input, &[2, 1], false);
    let prior = scope
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let (key, value) = input.iter().next().unwrap();
    let (((), observed), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let insertion = scope.prepare_insert().unwrap();
                charge::reserve(charge::binding(value)).unwrap();
                let incoming = value.clone();
                let before = crate::test_allocation_observer::current_metrics();
                let remaining = crate::bounded_output::active_remaining().unwrap();
                assert!(!crate::bounded_output::reserve_active_required(
                    remaining + 1
                ));
                let error = insertion.insert(key.clone(), incoming).unwrap_err();
                let after = crate::test_allocation_observer::current_metrics();
                assert_eq!(before.allocated, after.allocated);
                assert_eq!(before.allocations, after.allocations);
                assert_eq!(before.reallocations, after.reallocations);
                assert!(scope.iter().eq(prior.iter()));
                assert!(!crate::bounded_output::reserve_active_required(0));
                drop(error);
                drop(scope);
            })
        });
    assert!(refused && receipt.is_some() && !observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= paid && observed.highwater <= paid);
}

#[test]
fn multi_node_unique_prefix_shared_suffix_preserves_full_fields_at_every_paid_boundary() {
    let input = rich(15);
    let key = input.keys().next().unwrap().clone();
    let mut changed = input.clone();
    let value = changed.get_mut(&key).unwrap();
    value.mutable = !value.mutable;
    value.contract_local = !value.contract_local;
    value.availability = Availability::Moved;
    value.active_loans.insert(LoanId(777));
    for copied in [false, true] {
        let observe = |limit| {
            let mut scope = Scope::from_prepaid(input.clone());
            let held = scope
                .root
                .as_ref()
                .unwrap()
                .left
                .as_ref()
                .unwrap()
                .left
                .clone()
                .unwrap();
            let source = Scope::from_prepaid(changed.clone());
            let value = changed.get(&key).unwrap();
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                measure(|| {
                    let result = if copied {
                        scope.copy_binding_from(&source, &key)
                    } else {
                        put(&mut scope, &key, value, false)
                    };
                    let accepted = result.is_ok();
                    assert!(scope.iter().eq(if accepted {
                        changed.iter()
                    } else {
                        input.iter()
                    }));
                    for (id, binding) in views::TreeIter::new(Some(&held)) {
                        assert_eq!(Some(binding), input.get(id));
                    }
                    if !accepted {
                        assert!(!crate::bounded_output::reserve_active_required(0));
                    }
                    drop(result);
                    drop(scope);
                    drop(held);
                    accepted
                })
            })
        };
        let ((ok, wide), refused, total, receipt) = observe(LIMIT);
        assert!(ok && !refused && receipt.is_none() && !wide.invalid && wide.live == 0);
        for limit in 0..total {
            let ((ok, observed), refused, paid, receipt) = observe(limit);
            assert!(!ok && refused && receipt.is_some() && !observed.invalid);
            assert!(observed.allocated <= paid && observed.highwater <= paid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
        let ((ok, exact), refused, paid, receipt) = observe(total);
        assert!(ok && !refused && receipt.is_none() && paid == total);
        assert_eq!(wide, exact);
    }
}
