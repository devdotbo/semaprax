//! Independent literal actual layouts for the alias/binding/seen receivers.
//! Value IDs and complete input maps live outside the storage observer. Their
//! Arc handles transfer into the new nodes without cloning text backing.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::hir::DeclarationId;
use crate::test_allocation_observer::measure;
use std::mem::size_of;

#[test]
fn alias_binding_parent_receiver_literal_actual_heap_debit_exact_short_sticky_drop() {
    let ids: Vec<_> = (0..4)
        .map(|i| ValueId::intrinsic_parameter("paid.parent.é", i))
        .collect();
    let alias_heap = 3 * size_of::<tree::Node<ValueId, Place>>();
    let binding_heap = size_of::<tree::Node<ValueId, LoanId>>();
    let seen_heap = 4 * size_of::<tree::Node<ValueId, ()>>();
    let heap = alias_heap + binding_heap + seen_heap;
    let exact = size_of::<CfgReservation>()
        + 46
        + size_of::<Map<ValueId, Place>>()
        + size_of::<Map<ValueId, LoanId>>()
        + size_of::<Set<ValueId>>()
        + 2 * heap;
    let old_aliases: BTreeMap<_, _> = (0..3)
        .map(|i| {
            (
                ids[i].clone(),
                Place {
                    root: ids[i + 1].clone(),
                    projections: Vec::new(),
                },
            )
        })
        .collect();
    let old_bindings = BTreeMap::from([(ids[3].clone(), LoanId(7))]);
    let (old, refused, old_debit) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        paid_plan_legacy::resolve_parent_paid(
            &old_aliases,
            &old_bindings,
            ids[0].clone(),
            Some(&owner),
        )
        .unwrap()
    });
    assert!(!refused);
    assert_eq!(old, Some(LoanId(7)));
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                let mut aliases = Map::new(&owner).unwrap();
                let mut bindings = Map::new(&owner).unwrap();
                for i in 0..3 {
                    map_insert(
                        &mut aliases,
                        ids[i].clone(),
                        Place {
                            root: ids[i + 1].clone(),
                            projections: Vec::new(),
                        },
                        Some(&owner),
                    )
                    .unwrap();
                }
                map_insert(&mut bindings, ids[3].clone(), LoanId(7), Some(&owner)).unwrap();
                let result = paid_plan::resolve_parent_paid(
                    &aliases,
                    &bindings,
                    ids[0].clone(),
                    Some(&owner),
                );
                let success = match result {
                    Ok(parent) => {
                        assert_eq!(parent, old);
                        true
                    }
                    Err(_) => {
                        let first = owner.first().unwrap();
                        assert_eq!(owner.reserve(0), Err(first));
                        assert_eq!(owner.reserve(1), Err(first));
                        // A refused transient seen node cannot mutate either input.
                        assert_eq!(aliases.get(&ids[0]).unwrap().root, ids[1]);
                        assert_eq!(bindings.get(&ids[3]), Some(&LoanId(7)));
                        assert!(map_insert(
                            &mut bindings,
                            ids[3].clone(),
                            LoanId(99),
                            Some(&owner)
                        )
                        .is_err());
                        assert_eq!(bindings.get(&ids[3]), Some(&LoanId(7)));
                        false
                    }
                };
                drop(aliases);
                drop(bindings);
                drop(owner);
                success
            })
        })
    };
    let ((success, actual), refused, used, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(first, None);
    assert_eq!(used, exact);
    assert_eq!(actual.allocations, 9); // refusal text, four input nodes, four seen nodes
    assert_eq!(actual.reallocations, 0);
    assert_eq!(actual.allocated, 46 + heap);
    assert_eq!(actual.freed, actual.allocated);
    assert_eq!(actual.live, 0);
    assert!(!actual.invalid);
    let ((success, short), refused, used, first) = run(exact - 1);
    assert!(!success);
    assert!(refused);
    assert!(first.is_some());
    assert_eq!(used, exact - 2 * size_of::<tree::Node<ValueId, ()>>());
    assert_eq!(short.allocations, 8);
    assert_eq!(short.freed, short.allocated);
    assert_eq!(short.live, 0);
    assert!(!short.invalid);
    eprintln!("parent receiver old debit={old_debit} current debit={exact} actual={actual:?}");
}

#[test]
fn alias_parent_cycle_missing_and_binding_priority_match_complete_old_receiver() {
    let ids: Vec<_> = (0..4)
        .map(|i| ValueId::intrinsic_parameter("parent.#\0é", i))
        .collect();
    for cycle in [false, true] {
        for binding in [None, Some(0), Some(1), Some(3)] {
            let mut old_aliases = BTreeMap::new();
            for i in 0..3 {
                old_aliases.insert(
                    ids[i].clone(),
                    Place {
                        root: ids[i + 1].clone(),
                        projections: Vec::new(),
                    },
                );
            }
            if cycle {
                old_aliases.insert(
                    ids[3].clone(),
                    Place {
                        root: ids[1].clone(),
                        projections: Vec::new(),
                    },
                );
            }
            let mut old_bindings = BTreeMap::new();
            if let Some(i) = binding {
                old_bindings.insert(ids[i].clone(), LoanId(17));
            }
            let ((), refused, _) = with_limit_usage(usize::MAX, || {
                let owner = CfgReservation::acquire().unwrap();
                let mut aliases = Map::new(&owner).unwrap();
                let mut bindings = Map::new(&owner).unwrap();
                for (key, value) in &old_aliases {
                    map_insert(&mut aliases, key.clone(), value.clone(), Some(&owner)).unwrap();
                }
                for (key, value) in &old_bindings {
                    map_insert(&mut bindings, key.clone(), *value, Some(&owner)).unwrap();
                }
                for root in &ids {
                    let old = paid_plan_legacy::resolve_parent_paid(
                        &old_aliases,
                        &old_bindings,
                        root.clone(),
                        Some(&owner),
                    )
                    .unwrap();
                    assert_eq!(
                        paid_plan::resolve_parent_paid(
                            &aliases,
                            &bindings,
                            root.clone(),
                            Some(&owner)
                        )
                        .unwrap(),
                        old
                    );
                }
                assert_eq!(owner.first(), None);
            });
            assert!(!refused);
        }
    }
}

#[test]
fn borrowed_alias_seen_receiver_literal_heap_exact_short_drop_and_full_old_verdict() {
    let ids: Vec<_> = (0..4)
        .map(|i| ValueId::intrinsic_parameter("origin.é.#", i))
        .collect();
    let heap =
        3 * size_of::<tree::Node<ValueId, Place>>() + 3 * size_of::<tree::Node<&ValueId, ()>>();
    let exact = size_of::<CfgReservation>()
        + 46
        + size_of::<Map<ValueId, Place>>()
        + size_of::<Set<&ValueId>>()
        + 2 * heap;
    let old_aliases: BTreeMap<_, _> = (0..3)
        .map(|i| {
            (
                ids[i].clone(),
                Place {
                    root: ids[i + 1].clone(),
                    projections: Vec::new(),
                },
            )
        })
        .collect();
    let place = || Place {
        root: ids[0].clone(),
        projections: Vec::new(),
    };
    let (expected, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(&owner);
        let result = origin::resolve(&old_aliases, place(), &mut work).unwrap();
        assert_eq!(work.used, 3);
        result
    });
    assert!(!refused);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                let mut aliases = Map::new(&owner).unwrap();
                for (key, value) in &old_aliases {
                    map_insert(&mut aliases, key.clone(), value.clone(), Some(&owner)).unwrap();
                }
                let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
                work.allocation = Some(&owner);
                let result = origin::funded::resolve(&aliases, place(), &mut work);
                assert_eq!(work.used, 3);
                let success = match result {
                    Ok(result) => {
                        assert_eq!(result, expected);
                        drop(result);
                        true
                    }
                    Err(_) => {
                        let first = owner.first().unwrap();
                        assert_eq!(owner.reserve(0), Err(first));
                        assert_eq!(owner.reserve(1), Err(first));
                        assert_eq!(aliases.get(&ids[0]), old_aliases.get(&ids[0]));
                        false
                    }
                };
                drop(aliases);
                drop(owner);
                success
            })
        })
    };
    let ((success, actual), refused, used, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(first, None);
    assert_eq!(used, exact);
    assert_eq!(actual.allocations, 7);
    assert_eq!(actual.reallocations, 0);
    assert_eq!(actual.allocated, 46 + heap);
    assert_eq!(actual.freed, actual.allocated);
    assert_eq!(actual.live, 0);
    assert!(!actual.invalid);
    let ((success, short), refused, used, first) = run(exact - 1);
    assert!(!success);
    assert!(refused);
    assert!(first.is_some());
    assert_eq!(used, exact - 2 * size_of::<tree::Node<&ValueId, ()>>());
    assert_eq!(short.allocations, 6);
    assert_eq!(short.freed, short.allocated);
    assert_eq!(short.live, 0);
    assert!(!short.invalid);
}

#[test]
fn borrowed_alias_projection_cycle_and_work_diagnostics_equal_complete_old_constructor() {
    let ids: Vec<_> = (0..4)
        .map(|i| ValueId::intrinsic_parameter("origin.#\0é", i))
        .collect();
    for cycle in [false, true] {
        let mut old_aliases = BTreeMap::new();
        for i in 0..3 {
            old_aliases.insert(
                ids[i].clone(),
                Place {
                    root: ids[i + 1].clone(),
                    projections: vec![PlaceProjection::VariantField {
                        case: DeclarationId::new(format!("case.é.{i}")),
                        field: DeclarationId::new(format!("field.#\0.{i}")),
                    }],
                },
            );
        }
        if cycle {
            old_aliases.insert(
                ids[3].clone(),
                Place {
                    root: ids[1].clone(),
                    projections: Vec::new(),
                },
            );
        }
        let ((), refused, _) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut aliases = Map::new(&owner).unwrap();
            for (key, value) in &old_aliases {
                map_insert(&mut aliases, key.clone(), value.clone(), Some(&owner)).unwrap();
            }
            for limit in [0, 1, 3, 5, 9, MAX_LOAN_PLAN_WORK_V1] {
                let place = || Place {
                    root: ids[0].clone(),
                    projections: vec![PlaceProjection::Field(DeclarationId::new("assignment.é"))],
                };
                let mut old_work = WorkCounter::new(limit);
                old_work.allocation = Some(&owner);
                let old = origin::resolve(&old_aliases, place(), &mut old_work);
                let mut work = WorkCounter::new(limit);
                work.allocation = Some(&owner);
                assert_eq!(origin::funded::resolve(&aliases, place(), &mut work), old);
                assert_eq!(work.used, old_work.used);
                assert_eq!(owner.first(), None);
            }
        });
        assert!(!refused);
    }
}
