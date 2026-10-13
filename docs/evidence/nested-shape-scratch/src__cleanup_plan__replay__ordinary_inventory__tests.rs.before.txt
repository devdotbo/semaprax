//! Complete old/current receivers; direct paid storage and legacy residuals.
use super::*;
use crate::bounded_output::{active_remaining, reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

fn literal(file: &'static str) -> &'static str {
    file.split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0
}
fn program(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(source, "inventory-receiver.spx").unwrap()).unwrap()
}
fn fixtures() -> [ResolvedProgram; 2] {
    [
        program(literal(include_str!(
            "../../nested_record_destructure_tests.rs"
        ))),
        program(literal(include_str!("../../replay_tests.rs"))),
    ]
}
fn complete(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    saved: bool,
) -> (Result<(), Diagnostic>, usize, usize, usize) {
    let mut budget = ReplayBudget::new();
    let run = |budget: &mut ReplayBudget| {
        reserve_program_skeleton_work(program, std::iter::once(function), budget)?;
        validate_structure_with_budget(program, function, budget, None)
    };
    let result = if saved {
        with_saved_receiver(|| run(&mut budget))
    } else {
        run(&mut budget)
    };
    (
        result,
        budget.remaining,
        budget.skeleton_remaining,
        budget.skeleton_spent,
    )
}
#[test]
fn complete_original_nested_and_resource_variant_receivers_preserve_verdicts_and_work() {
    for program in fixtures() {
        for function in &program.functions {
            assert_eq!(
                complete(&program, function, true),
                complete(&program, function, false)
            );
            let mut changed = function.clone();
            if let Some(slot) = changed.cleanup_plan.slots.first_mut() {
                slot.storage_index = slot.storage_index.wrapping_add(1);
                assert_eq!(
                    complete(&program, &changed, true),
                    complete(&program, &changed, false)
                );
            }
            let mut changed = function.clone();
            changed.cleanup_plan.slots.pop();
            assert_eq!(
                complete(&program, &changed, true),
                complete(&program, &changed, false)
            );
        }
    }
}
#[test]
fn late_unknown_entry_storage_precedes_retained_earlier_entry_mismatch() {
    let programs = fixtures();
    let program = &programs[0];
    let mut function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "envelope.consume")
        .unwrap()
        .clone();
    function
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .clear();
    function
        .cleanup
        .entry_state
        .live_owned_parameters
        .push(crate::cleanup::CleanupStorageId(u32::MAX));
    let old = owned::validate_inventory_coverage(program, &function);
    let current = validate_inventory_coverage(program, &function, &mut ReplayBudget::new());
    assert_eq!(old, current);
    assert!(current
        .unwrap_err()
        .message
        .contains("entry references unknown storage"));
}
#[test]
fn one_paid_fixed_cursor_reuses_original_capacity_without_per_expression_requests() {
    let program = program("module inventory.cursor; @id(\"main\") fn main()->i64 {0}");
    let function = &program.functions[0];
    let (((), old), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut flags = 0;
            let mut slots = Vec::new();
            for _ in 0..17 {
                collect_supplemental_slots(
                    &program,
                    function,
                    &function.body,
                    &mut flags,
                    &mut slots,
                )
                .unwrap();
            }
            assert!(slots.is_empty());
        })
    });
    assert!(!refused && !old.invalid && old.live == 0);
    assert_eq!(old.allocations, 17);
    let (((), current), refused, used, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut cursor = supplemental::receiving::Scratch::new(function, &mut budget).unwrap();
            let mut flags = 0;
            let before = crate::test_allocation_observer::current_metrics();
            for _ in 0..17 {
                supplemental::receiving::walk(
                    &program,
                    function,
                    &function.body,
                    &mut flags,
                    &mut cursor,
                    &mut budget,
                    |_, _, _| unreachable!(),
                )
                .unwrap();
            }
            let after = crate::test_allocation_observer::current_metrics();
            assert_eq!(after.allocations, before.allocations);
            assert_eq!(after.allocated, before.allocated);
            drop(cursor);
            drop(budget);
        })
    });
    assert!(!refused && !current.invalid && current.live == 0);
    assert_eq!(current.allocated, current.freed);
    assert!(current.allocated <= used);
}
#[test]
fn whole_current_inventory_exact_short_and_sticky_refusal_settle_through_receiving_drop() {
    let program = program("module inventory.payer; @id(\"main\") fn main()->i64 {0}");
    let function = &program.functions[0];
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = validate_inventory_coverage(&program, function, &mut budget);
                let before = active_remaining();
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    assert!(!reserve_active_required(0));
                    let before_retry = crate::test_allocation_observer::current_metrics();
                    drop(validate_inventory_coverage(&program, function, &mut budget));
                    let after_retry = crate::test_allocation_observer::current_metrics();
                    assert_eq!(after_retry.allocated, before_retry.allocated);
                    assert_eq!(after_retry.allocations, before_retry.allocations);
                    assert_eq!(before, active_remaining());
                } else {
                    result.as_ref().unwrap();
                }
                drop(result);
                drop(budget);
            })
        })
    };
    let (((), wide), refused, required, _) = observe(usize::MAX);
    assert!(!refused && required > 0 && !wide.invalid && wide.live == 0);
    assert_eq!(wide.allocated, wide.freed);
    assert!(wide.allocated <= required && wide.highwater <= required);
    let (((), exact), refused, used, _) = observe(required);
    assert!(!refused && used == required && !exact.invalid && exact.live == 0);
    assert_eq!(exact.allocated, exact.freed);
    assert!(exact.allocated <= used && exact.highwater <= used);
    let (((), short), refused, _, first) = observe(required - 1);
    assert!(refused && first.is_some() && !short.invalid && short.live == 0);
    assert_eq!(short.allocated, short.freed);
    let (((), zero), refused, used, first) = observe(0);
    assert!(refused && first.is_some() && used == 0 && !zero.invalid && zero.live == 0);
    assert_eq!(zero.allocated, 0);
}
#[test]
fn paid_substitution_preserves_complete_original_callable_and_nominal_payloads() {
    let program = program("module inventory.substitute; @id(\"main\") fn main()->i64 {0}");
    let function = &program.functions[0];
    let owner = DeclarationId::new("substitution.owner");
    let template = ResolvedType::Function {
        parameters: vec![ResolvedType::TypeParameter {
            owner: owner.clone(),
            index: 0,
        }],
        result: Box::new(ResolvedType::Nominal {
            declaration: owner.clone(),
            arguments: vec![ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 0,
            }],
        }),
    };
    let arguments = [ResolvedType::Function {
        parameters: vec![ResolvedType::I64; 17],
        result: Box::new(ResolvedType::I64),
    }];
    let expected = crate::hir::substitute_type(&template, &owner, &arguments).unwrap();
    let (((), whole), refused, used, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let actual =
                substitute::substitute_type(&template, &owner, &arguments, function, &mut budget)
                    .unwrap();
            assert_eq!(actual, expected);
            drop(actual);
            drop(budget);
        })
    });
    assert!(!refused && !whole.invalid && whole.live == 0);
    assert_eq!(whole.allocated, whole.freed);
    assert!(whole.allocated <= used && whole.highwater <= used);
}

#[test]
fn duplicate_inventory_ids_keep_the_complete_old_last_insertion_receiver() {
    let programs = fixtures();
    let program = &programs[1];
    let mut function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "tokens.discard")
        .unwrap()
        .clone();
    assert_eq!(function.cleanup.slots.len(), 2);
    assert_ne!(
        function.cleanup.slots[0].origin,
        function.cleanup.slots[1].origin
    );
    let first = function.cleanup.slots[0].id;
    function.cleanup.slots[1].id = first;
    function.cleanup_plan.slots[1].id.0 = first.0;
    function.cleanup.entry_state.live_owned_parameters = vec![first];
    function
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .truncate(1);
    let old = owned::validate_inventory_coverage(program, &function);
    let current = validate_inventory_coverage(program, &function, &mut ReplayBudget::new());
    assert_eq!(old, current);
    assert!(current
        .unwrap_err()
        .message
        .contains("entry ownership disagrees"));
}

#[test]
fn complete_original_inventory_construction_receiving_and_drop_reports_both_real_costs() {
    for program in fixtures() {
        for function in &program.functions {
            let observe = |saved| {
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let result = if saved {
                            with_saved_receiver(|| {
                                validate_inventory_coverage(&program, function, &mut budget)
                            })
                        } else {
                            validate_inventory_coverage(&program, function, &mut budget)
                        };
                        assert!(result.is_ok());
                        drop(result);
                        drop(budget);
                    })
                })
            };
            let (((), old), old_refused, old_used, _) = observe(true);
            let (((), current), current_refused, current_used, _) = observe(false);
            assert!(!old_refused && !current_refused);
            assert!(!old.invalid && !current.invalid && old.live == 0 && current.live == 0);
            assert_eq!(old.allocated, old.freed);
            assert_eq!(current.allocated, current.freed);
            // Full observations remain full, including the explicitly unpaid
            // classifier/cache-miss and synthetic-provider residuals. They are
            // reported independently; no counter discount or whole-funding claim.
            eprintln!(
                "inventory receiver {} old={old:?}/{old_used} current={current:?}/{current_used}",
                function.id
            );
        }
    }
}

#[test]
fn complete_paid_shape_receivers_keep_original_flags_exact_short_sticky_and_drop() {
    for program in fixtures() {
        for function in &program.functions {
            for slot in &function.cleanup.slots {
                let mut expected_flags = 0;
                let expected =
                    expected_shape_for_type(&program, function, &slot.ty, &mut expected_flags)
                        .unwrap();
                let observe = |limit| {
                    with_limit_usage_refusal(limit, || {
                        measure(|| {
                            let mut budget = ReplayBudget::new();
                            let mut flags = 0;
                            let result = nested_shape::expected_shape_for_type_paid(
                                &program,
                                function,
                                &slot.ty,
                                &mut flags,
                                &mut budget,
                            );
                            if crate::bounded_output::active_refused() {
                                assert!(result.is_err());
                                let before = crate::test_allocation_observer::current_metrics();
                                let remaining = active_remaining();
                                assert!(!reserve_active_required(0));
                                drop(nested_shape::expected_shape_for_type_paid(
                                    &program,
                                    function,
                                    &slot.ty,
                                    &mut flags,
                                    &mut budget,
                                ));
                                let after = crate::test_allocation_observer::current_metrics();
                                assert_eq!(before.allocations, after.allocations);
                                assert_eq!(before.allocated, after.allocated);
                                assert_eq!(remaining, active_remaining());
                            } else {
                                assert_eq!(result.as_ref().unwrap(), &expected);
                                assert_eq!(flags, expected_flags);
                            }
                            drop(result);
                            drop(budget);
                        })
                    })
                };
                let (((), wide), refused, required, _) = observe(usize::MAX);
                assert!(!refused && required > 0 && !wide.invalid && wide.live == 0);
                assert_eq!(wide.allocated, wide.freed);
                let (((), exact), refused, used, _) = observe(required);
                assert!(!refused && used == required && !exact.invalid && exact.live == 0);
                assert_eq!(exact.allocated, exact.freed);
                let (((), short), refused, _, first) = observe(required - 1);
                assert!(refused && first.is_some() && !short.invalid && short.live == 0);
                assert_eq!(short.allocated, short.freed);
                let (((), zero), refused, used, first) = observe(0);
                assert!(refused && first.is_some() && used == 0 && !zero.invalid && zero.live == 0);
                assert_eq!(zero.allocated, 0);
                eprintln!("direct shape {} slot{} whole={wide:?} debit={required}; classifier residual retained", function.id, slot.id.0);
            }
        }
    }
}
