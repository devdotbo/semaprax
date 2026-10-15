//! Complete old baseline and persistent sharing, refusal and lifetime owners.
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
enum Flags {
    Clear,
    True,
    Mixed,
}
fn input(count: usize, flags: Flags) -> BTreeMap<ValueId, ValidationBinding> {
    let mut values = rich(count);
    for (index, value) in values.values_mut().enumerate() {
        value.contract_local = match flags {
            Flags::Clear => false,
            Flags::True => true,
            Flags::Mixed => index % 2 == 0,
        };
    }
    values
}
fn expected(input: &BTreeMap<ValueId, ValidationBinding>) -> BTreeMap<ValueId, ValidationBinding> {
    let mut values = input.clone();
    for value in values.values_mut() {
        value.contract_local = false;
    }
    values
}
fn observe(
    input: &BTreeMap<ValueId, ValidationBinding>,
    shared: bool,
    old: bool,
    limit: usize,
) -> (bool, Metrics, bool, usize, bool) {
    let outer = Scope::from_prepaid(input.clone());
    let snapshot = if shared { Some(outer.clone()) } else { None };
    let expected = expected(input);
    let ((ok, observed), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = if old {
                    Scope::fresh_baseline(&outer)
                } else {
                    Scope::fresh_baseline_persistent(&outer)
                };
                let ok = result.is_ok();
                if let Ok(baseline) = &result {
                    assert!(baseline.iter().eq(expected.iter()));
                    if !old {
                        for (key, value) in outer.iter() {
                            let before = tree::value(outer.root.as_deref(), key).unwrap();
                            let after = tree::value(baseline.root.as_deref(), key).unwrap();
                            assert_eq!(Rc::ptr_eq(before, after), !value.contract_local);
                        }
                        if input.values().all(|value| !value.contract_local) {
                            match (&outer.root, &baseline.root) {
                                (Some(before), Some(after)) => assert!(Rc::ptr_eq(before, after)),
                                (None, None) => {}
                                _ => panic!("complete clear root"),
                            }
                        }
                    }
                } else {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                assert!(outer.iter().eq(input.iter()));
                if let Some(snapshot) = &snapshot {
                    assert!(snapshot.iter().eq(input.iter()));
                }
                let remaining = crate::bounded_output::active_remaining();
                drop(result);
                drop(snapshot);
                drop(outer);
                assert_eq!(crate::bounded_output::active_remaining(), remaining);
                ok
            })
        });
    (ok, observed, refused, paid, receipt.is_some())
}
#[test]
fn full_clear_true_mixed_and_shared_baselines_match_complete_old_copy_every_field() {
    for count in [0, 1, 17, 128] {
        for flags in [Flags::Clear, Flags::True, Flags::Mixed] {
            let input = input(count, flags);
            for shared in [false, true] {
                let (ok, current, refused, paid, receipt) = observe(&input, shared, false, LIMIT);
                let (old_ok, old, old_refused, old_paid, old_receipt) =
                    observe(&input, shared, true, LIMIT);
                assert!(ok && old_ok && !refused && !old_refused && !receipt && !old_receipt);
                assert!(!current.invalid && !old.invalid && current.live == 0 && old.live == 0);
                assert!(current.allocated <= paid && current.highwater <= paid);
                assert!(old.allocated <= old_paid && old.highwater <= old_paid);
                assert_eq!(current.allocated, current.freed);
                if matches!(flags, Flags::Clear) {
                    assert_eq!(current, Metrics::default());
                    assert_eq!(paid, 0);
                    if count != 0 {
                        assert!(old.allocations > 0);
                    }
                }
            }
        }
    }
}
#[test]
fn every_paid_byte_refusal_and_exact_short_preserve_full_outer_root_and_drop() {
    for flags in [Flags::Clear, Flags::True, Flags::Mixed] {
        let input = input(3, flags);
        for shared in [false, true] {
            let (ok, wide, refused, total, receipt) = observe(&input, shared, false, LIMIT);
            assert!(ok && !refused && !receipt && !wide.invalid);
            for limit in 0..total {
                let (ok, observed, refused, paid, receipt) = observe(&input, shared, false, limit);
                assert!(!ok && refused && receipt && !observed.invalid);
                assert!(paid <= limit && observed.allocated <= paid && observed.highwater <= paid);
                assert_eq!(observed.live, 0);
                assert_eq!(observed.allocated, observed.freed);
            }
            let (ok, exact, refused, paid, receipt) = observe(&input, shared, false, total);
            assert!(ok && !refused && !receipt && paid == total);
            assert_eq!(wide, exact);
        }
    }
}
#[test]
fn clear_zero_heap_and_empty_baselines_still_observe_sticky_first_refusal() {
    for count in [0, 17] {
        let input = input(count, Flags::Clear);
        let outer = Scope::from_prepaid(input.clone());
        let snapshot = outer.clone();
        let (((), observed), refused, paid, receipt) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                measure(|| {
                    assert!(!crate::bounded_output::reserve_active_required(1));
                    let error = Scope::fresh_baseline_persistent(&outer).unwrap_err();
                    assert!(outer.iter().eq(input.iter()) && snapshot.iter().eq(input.iter()));
                    assert!(!crate::bounded_output::reserve_active_required(0));
                    drop(error);
                    drop(snapshot);
                    drop(outer);
                })
            });
        assert!(refused && receipt.is_some());
        assert_eq!(paid, 0);
        assert_eq!(observed, Metrics::default());
    }
}
#[test]
fn every_clause_owns_independent_full_metadata_after_shared_clear_and_hostile_resets() {
    for flags in [Flags::Clear, Flags::True, Flags::Mixed] {
        let input = input(17, flags);
        let outer = Scope::from_prepaid(input.clone());
        let expected = expected(&input);
        let key = input.keys().next().unwrap().clone();
        let mut changed = expected.clone();
        let value = changed.get_mut(&key).unwrap();
        value.contract_local = true;
        value.mutable = !value.mutable;
        value.ownership = OwnershipMode::Borrow;
        value.availability = Availability::Moved;
        if let ResolvedType::Function { result, .. } = &mut value.ty {
            **result = ResolvedType::Bool;
        }
        for availability in value.moved_places.values_mut() {
            *availability = Availability::Moved;
        }
        let (((), observed), refused, paid) =
            crate::bounded_output::with_limit_usage(LIMIT, || {
                measure(|| {
                    let baseline = Scope::fresh_baseline_persistent(&outer).unwrap();
                    let sibling = baseline.clone();
                    let mut clause = baseline.clone();
                    let value = clause.get_mut(&key).unwrap().unwrap();
                    value.contract_local = true;
                    value.mutable = !value.mutable;
                    value.ownership = OwnershipMode::Borrow;
                    value.availability = Availability::Moved;
                    if let ResolvedType::Function { result, .. } = &mut value.ty {
                        **result = ResolvedType::Bool;
                    }
                    for availability in value.moved_places.values_mut() {
                        *availability = Availability::Moved;
                    }
                    assert!(clause.iter().eq(changed.iter()));
                    assert!(baseline.iter().eq(expected.iter()));
                    assert!(sibling.iter().eq(expected.iter()));
                    assert!(outer.iter().eq(input.iter()));
                    drop(clause);
                    drop(sibling);
                    drop(baseline);
                    drop(outer);
                })
            });
        assert!(!refused && !observed.invalid && observed.live == 0);
        assert!(observed.allocated <= paid && observed.highwater <= paid);
    }
}
const CONTRACT_SOURCE: &str = r#"module contract.hir;
@id("inspect") fn inspect(input:own Bytes)->bool {byte_len(bytes_as_slice(input))==2usize}
@id("factory") fn factory(n:usize)->Bytes {bytes_zeroed(2usize)}
@id("probe") fn probe(input:own Bytes)->Bytes
 requires {let local=bytes_zeroed(2usize);inspect(local)}
 ensures {let checked=bytes_zeroed(2usize);byte_len(bytes_as_slice(checked))==2usize}
 {bytes_zeroed(1usize)}
@id("main") fn main()->i64{0}
"#;
const PAYLOAD_SOURCE: &str = r#"module function.payload;
@id("payload") fn payload(input:own Box<i64>)->Box<i64>
 requires true ensures true {box_new<i64>(box_into_inner<i64>(input))}
@id("main") fn main()->i64 {let value=payload(box_new<i64>(3));box_into_inner<i64>(value)}
"#;

fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "fresh-baseline.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
#[test]
fn complete_production_function_chain_keeps_saved_map_verdicts_and_full_iterative_corpus() {
    for source in [
        crate::hir::iterative_validator_tests::SOURCE,
        CONTRACT_SOURCE,
        PAYLOAD_SOURCE,
    ] {
        let program = program(source);
        for function in &program.functions {
            let execution = FunctionExecutionId::Monomorphic(function.id.clone());
            let mut current = HirValidator::new(&program).unwrap();
            let mut old = current.clone();
            let expected = old.validate_function_owned_reference(function, &execution);
            let actual = current.validate_function(function, &execution);
            HirValidator::assert_validation_oracle(
                &actual,
                &expected,
                &current,
                &old,
                &BTreeMap::new(),
                &BTreeMap::new(),
                function.id.as_str(),
            );
            HirValidator::assert_scope_auxiliary_oracle(&current, &old);
            actual.unwrap();
        }
    }
}

#[test]
fn later_true_flags_are_reinspected_without_prior_clear_root_authority() {
    let input = input(17, Flags::Clear);
    let mut outer = Scope::from_prepaid(input.clone());
    let key = input.keys().nth(8).unwrap().clone();
    let mut hostile = input.clone();
    hostile.get_mut(&key).unwrap().contract_local = true;
    let (((), observed), refused, paid) = crate::bounded_output::with_limit_usage(LIMIT, || {
        measure(|| {
            let first = Scope::fresh_baseline_persistent(&outer).unwrap();
            outer.get_mut(&key).unwrap().unwrap().contract_local = true;
            assert!(outer.iter().eq(hostile.iter()));
            let second = Scope::fresh_baseline_persistent(&outer).unwrap();
            assert!(first.iter().eq(input.iter()));
            assert!(second.iter().eq(input.iter()));
            assert!(outer.iter().eq(hostile.iter()));
            drop(second);
            drop(first);
            drop(outer);
        })
    });
    assert!(!refused && !observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= paid && observed.highwater <= paid);
}
#[test]
fn unwind_drops_changed_baseline_and_clause_roots_without_mutating_full_outer_or_siblings() {
    for flags in [Flags::Clear, Flags::True, Flags::Mixed] {
        let input = input(17, flags);
        let outer = Scope::from_prepaid(input.clone());
        let snapshot = outer.clone();
        let expected = expected(&input);
        let key = input.keys().next().unwrap().clone();
        let mut weak_root = None;
        let mut weak_clause = None;
        let (caught, refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let baseline = Scope::fresh_baseline_persistent(&outer).unwrap();
                weak_root = baseline.root.as_ref().map(Rc::downgrade);
                let sibling = baseline.clone();
                let mut clause = baseline.clone();
                clause.get_mut(&key).unwrap().unwrap().availability = Availability::Moved;
                weak_clause = tree::value(clause.root.as_deref(), &key).map(Rc::downgrade);
                assert!(baseline.iter().eq(expected.iter()) && sibling.iter().eq(expected.iter()));
                panic!("persistent baseline and isolated clause unwind");
            }))
        });
        assert!(caught.is_err() && !refused);
        drop(caught);
        assert!(outer.iter().eq(input.iter()) && snapshot.iter().eq(input.iter()));
        assert!(weak_clause.unwrap().upgrade().is_none());
        assert_eq!(
            weak_root.unwrap().upgrade().is_some(),
            matches!(flags, Flags::Clear)
        );
        // Panic infrastructure is outside the separate allocator/payment owner.
        // Weak handles prove destruction of the newly owned baseline/clause.
    }
}
