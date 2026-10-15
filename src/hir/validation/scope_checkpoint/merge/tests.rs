//! Full paid original joins, canonical/noncanonical projection states and Drop.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};

fn input(count: usize, normalize: bool) -> BTreeMap<ValueId, ValidationBinding> {
    (0..count)
        .map(|i| {
            let path = vec![PlaceProjection::Field(DeclarationId::new("join.field")); 2];
            let mut moved_places = BTreeMap::new();
            moved_places.insert(
                path.clone(),
                if normalize {
                    Availability::Moved
                } else {
                    Availability::MaybeMoved
                },
            );
            let definitely_partial = if normalize {
                BTreeSet::from([path])
            } else {
                BTreeSet::new()
            };
            (
                ValueId::new(format!("join.{i:04}")),
                ValidationBinding {
                    contract_local: i % 2 == 0,
                    mutable: i % 3 == 0,
                    ty: ResolvedType::Function {
                        parameters: vec![
                            ResolvedType::Nominal {
                                declaration: DeclarationId::new("join.payload"),
                                arguments: vec![ResolvedType::I64; 8],
                            };
                            2
                        ],
                        result: Box::new(ResolvedType::Bool),
                    },
                    ownership: OwnershipMode::Own,
                    availability: Availability::Available,
                    active_loans: BTreeSet::from([LoanId(i as u16)]),
                    moved_places,
                    definitely_partial,
                },
            )
        })
        .collect()
}
fn seed(input: &BTreeMap<ValueId, ValidationBinding>) -> Result<Scope, Diagnostic> {
    let mut result = Scope::default();
    for (id, binding) in input {
        let prepared = result.prepare_insert()?;
        charge::reserve(charge::binding(binding))?;
        // ValueId clones share the original Arc; this acquires no string backing.
        prepared.insert(id.clone(), binding.clone())?;
    }
    Ok(result)
}
fn join(
    target: &mut Scope,
    left: &Scope,
    right: &Scope,
    branches: bool,
    original: bool,
) -> Result<(), Diagnostic> {
    let ids = target.ids();
    match (branches, original) {
        (false, false) => conditional(target, left, &ids),
        (false, true) => saved::saved_conditional(target, left, &ids),
        (true, false) => self::branches(target, left, right, &ids),
        (true, true) => saved::saved_branches(target, left, right, &ids),
    }
}
fn receive(
    input: &BTreeMap<ValueId, ValidationBinding>,
    branches: bool,
    original: bool,
    expected: &BTreeMap<ValueId, ValidationBinding>,
) -> Result<bool, Diagnostic> {
    let mut target = seed(input)?;
    let left = target.clone();
    let right = target.clone();
    let root = target.root.clone();
    join(&mut target, &left, &right, branches, original)?;
    assert!(target.iter().eq(expected.iter()));
    assert!(left.iter().eq(input.iter()));
    assert!(right.iter().eq(input.iter()));
    let retained = match (&root, &target.root) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    };
    drop(root);
    drop(right);
    drop(left);
    drop(target);
    Ok(retained)
}
fn observed(
    input: &BTreeMap<ValueId, ValidationBinding>,
    branches: bool,
    original: bool,
    limit: usize,
    expected: &BTreeMap<ValueId, ValidationBinding>,
) -> (
    (bool, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| match receive(input, branches, original, expected) {
            Ok(retained) => {
                let _ = retained;
                true
            }
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                assert!(!crate::bounded_output::reserve_active_required(0));
                let sticky = charge::reserve(Some(0)).unwrap_err();
                assert_eq!(sticky, error);
                drop(sticky);
                drop(error.clone());
                drop(error);
                false
            }
        })
    })
}
fn complete(metrics: &Metrics, paid: usize) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.freed, metrics.allocated);
    assert!(metrics.allocated <= paid);
    assert!(metrics.highwater <= paid);
}

#[test]
fn canonical_nonempty_noop_keeps_complete_old_fields_and_removes_actual_payload_cow() {
    for count in [1, 2, 17] {
        for branches in [false, true] {
            let input = input(count, false);
            assert!(input.values().all(|v| !v.moved_places.is_empty()));
            let ((old, old_metrics), old_refused, old_paid, old_first) =
                observed(&input, branches, true, usize::MAX, &input);
            let ((current, new_metrics), refused, paid, first) =
                observed(&input, branches, false, usize::MAX, &input);
            assert!(old && current);
            assert!(!old_refused && !refused);
            assert_eq!(old_first, None);
            assert_eq!(first, None);
            complete(&old_metrics, old_paid);
            complete(&new_metrics, paid);
            assert!(new_metrics.allocated < old_metrics.allocated);
            assert!(new_metrics.allocations < old_metrics.allocations);
            assert!(paid < old_paid);
            assert!(receive(&input, branches, false, &input).unwrap());
            assert!(!receive(&input, branches, true, &input).unwrap());
        }
    }
}

#[test]
fn equal_nonempty_prefix_normalization_and_changed_outputs_keep_complete_original_scope() {
    for count in [1, 2, 17] {
        for branches in [false, true] {
            let input = input(count, true);
            let ids = input.keys().cloned().collect::<Vec<_>>();
            let mut expected = input.clone();
            if branches {
                HirValidator::join_branches(&mut expected, &input, &input, &ids)
            } else {
                HirValidator::join_conditional(&mut expected, &input, &ids)
            }
            assert_ne!(expected, input);
            assert!(expected
                .values()
                .all(|v| v.definitely_partial.contains(&Vec::new())));
            let ((success, metrics), refused, paid, first) =
                observed(&input, branches, false, usize::MAX, &expected);
            assert!(success);
            assert!(!refused);
            assert_eq!(first, None);
            complete(&metrics, paid);
            assert!(!receive(&input, branches, false, &expected).unwrap());
            receive(&input, branches, true, &expected).unwrap();
        }
    }
}

#[test]
fn noops_use_only_written_fields_and_changed_loan_availability_still_commit() {
    let input = input(2, false);
    for branches in [false, true] {
        let mut target = seed(&input).unwrap();
        let before = target.clone();
        let mut left = target.clone();
        let mut right = target.clone();
        let id = target.keys().next().unwrap().clone();
        for source in [&mut left, &mut right] {
            let value = source.get_mut(&id).unwrap().unwrap();
            value.contract_local = !value.contract_local;
            value.mutable = !value.mutable;
        }
        join(&mut target, &left, &right, branches, false).unwrap();
        assert_eq!(target, before);
        assert!(Rc::ptr_eq(
            target.root.as_ref().unwrap(),
            before.root.as_ref().unwrap()
        ));
        let value = left.get_mut(&id).unwrap().unwrap();
        value.availability = Availability::Moved;
        insert_loan(value, LoanId(99)).unwrap();
        let mut old = before.clone();
        join(&mut old, &left, &right, branches, true).unwrap();
        join(&mut target, &left, &right, branches, false).unwrap();
        assert_eq!(target, old);
        assert_ne!(target, before);
    }
}

#[test]
fn complete_join_receiver_exact_short_zero_and_sticky_are_paid_through_drop() {
    for branches in [false, true] {
        for normalize in [false, true] {
            let input = input(17, normalize);
            let ids = input.keys().cloned().collect::<Vec<_>>();
            let mut expected = input.clone();
            if branches {
                HirValidator::join_branches(&mut expected, &input, &input, &ids)
            } else {
                HirValidator::join_conditional(&mut expected, &input, &ids)
            }
            let ((success, baseline), refused, exact, first) =
                observed(&input, branches, false, usize::MAX, &expected);
            assert!(success);
            assert!(!refused);
            assert_eq!(first, None);
            complete(&baseline, exact);
            assert!(exact > 0);
            let ((success, repeated), refused, paid, first) =
                observed(&input, branches, false, exact, &expected);
            assert!(success);
            assert!(!refused);
            assert_eq!(paid, exact);
            assert_eq!(first, None);
            assert_eq!(repeated, baseline);
            for limit in [0, exact - 1] {
                let ((success, metrics), refused, paid, first) =
                    observed(&input, branches, false, limit, &expected);
                assert!(!success);
                assert!(refused);
                assert!(first.is_some());
                complete(&metrics, paid);
            }
        }
    }
}

#[test]
fn sticky_join_scratch_refusal_leaves_complete_target_and_first_receipt_unchanged() {
    let input = input(2, false);
    for branches in [false, true] {
        let mut target = seed(&input).unwrap();
        let left = target.clone();
        let right = target.clone();
        let before = target.clone();
        let ((), refused, paid, first) = with_limit_usage_refusal(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            let error = join(&mut target, &left, &right, branches, false).unwrap_err();
            let repeated = join(&mut target, &left, &right, branches, false).unwrap_err();
            assert_eq!(error, repeated);
            assert_eq!(target, before);
            assert_eq!(error.code, "SPX-H006");
            assert_eq!(
                error.message,
                "ownership scope checkpoint exceeds the active validation budget"
            );
            assert_eq!(error.severity, crate::diagnostic::Severity::Error);
            assert_eq!(error.path, None);
            assert_eq!(error.span, None);
            assert_eq!(error.help, None);
        });
        assert!(refused);
        assert_eq!(paid, 0);
        assert_eq!(first.unwrap().requested, 1);
    }
}

#[test]
fn complete_saved_paid_merge_producer_is_byte_exact() {
    let mut original = include_str!("saved.rs").to_owned();
    for name in ["availability", "borrows", "conditional", "branches"] {
        original = original.replace(&format!("fn saved_{name}("), &format!("fn {name}("));
    }
    assert_eq!(
        original,
        include_str!("../../../../../docs/evidence/validator-join-noop-795/merge.rs.before")
    );
}

fn late_join(
    target: &mut Scope,
    source: &Scope,
    branches: bool,
    original: bool,
) -> Result<(), Diagnostic> {
    let mut publication = Publication::take(target)?;
    let mut candidate = publication.published.clone();
    join(&mut candidate, source, source, branches, original)?;
    publication.publish(&candidate)?;
    let mut failed_child = candidate.clone();
    let id = failed_child.keys().next().unwrap().clone();
    failed_child.get_mut(&id)?.unwrap().availability = Availability::Moved;
    Err(hir_error_static(
        "selected late join error keeps the complete publication",
    ))
}

#[test]
fn late_error_restores_complete_publication_and_selected_diagnostic_through_drop() {
    for branches in [false, true] {
        for original in [false, true] {
            let input = input(2, false);
            let (((), metrics), refused, paid, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let mut target = seed(&input).unwrap();
                        let source = target.clone();
                        let error =
                            late_join(&mut target, &source, branches, original).unwrap_err();
                        assert_eq!(
                            error,
                            hir_error_static(
                                "selected late join error keeps the complete publication"
                            )
                        );
                        assert!(target.iter().eq(input.iter()));
                        assert!(source.iter().eq(input.iter()));
                        drop(error);
                        drop(source);
                        drop(target);
                    })
                });
            assert!(!refused);
            assert_eq!(first, None);
            complete(&metrics, paid);
        }
    }
}

#[test]
fn unchanged_availability_and_borrow_producers_keep_complete_saved_receiver_fields() {
    let input = input(2, false);
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut source = seed(&input).unwrap();
        let id = source.keys().next().unwrap().clone();
        let value = source.get_mut(&id).unwrap().unwrap();
        value.availability = Availability::Moved;
        insert_loan(value, LoanId(99)).unwrap();
        let mut current = seed(&input).unwrap();
        let mut original = current.clone();
        let ids = current.ids();
        availability(&mut current, &source, &ids).unwrap();
        saved::saved_availability(&mut original, &source, &ids).unwrap();
        assert_eq!(current, original);
        borrows(&mut current, &source, &ids).unwrap();
        saved::saved_borrows(&mut original, &source, &ids).unwrap();
        assert_eq!(current, original);
        assert_eq!(current.get(&id).unwrap().availability, Availability::Moved);
        assert!(current.get(&id).unwrap().active_loans.contains(&LoanId(99)));
    });
    assert!(!refused);
    assert_eq!(first, None);
}
