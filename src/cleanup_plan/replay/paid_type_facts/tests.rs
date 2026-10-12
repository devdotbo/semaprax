//! Complete old receivers and actual caller-owned construction/receiving/Drop.
use super::*;
use crate::bounded_output::{active_remaining, reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

fn literal(file: &'static str) -> &'static str {
    file.split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0
}
fn fixtures() -> [ResolvedProgram; 3] {
    let replay = include_str!("../../replay_tests.rs");
    let try_source = replay
        .split_once("const TRY_SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    [
        literal(include_str!("../../nested_record_destructure_tests.rs")),
        literal(replay),
        try_source,
    ]
    .map(|source| crate::hir::resolve(&crate::parse(source, "classifier.spx").unwrap()).unwrap())
}

fn payer<'a>(function: &'a ResolvedFunction, budget: &'a mut ReplayBudget) -> Payer<'a> {
    Payer {
        function,
        budget,
        ledger: ClassifierLedger::capture(),
    }
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
        with_saved_classifier(|| run(&mut budget))
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
fn complete_original_nested_resource_receivers_preserve_diagnostics_and_work() {
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
fn full_uncached_owned_classifier_layouts_match_complete_original_classifier() {
    for program in fixtures() {
        let index = program.declarations.classifier_uncached_for_test();
        let function = &program.functions[0];
        for item in &program.types {
            let ty = ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            };
            let expected = index.classifier_old_facts_for_test(&ty);
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_compute_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                expected
            );
            assert_eq!(
                flags(&index, function, &ty, &mut budget).unwrap(),
                index.type_flags(&ty)
            );
        }
        // Original generic declaration and every concrete function boundary
        // remain complete source receivers, including owned variant instances,
        // wrong arity and unsupported argument/substitution frontiers.
        let mut roots = Vec::new();
        for function in &program.functions {
            roots.push(function.return_type.clone());
            roots.extend(
                function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty.clone()),
            );
        }
        if let Some(declaration) = index.type_id("GenericChoice") {
            for arguments in [
                vec![ResolvedType::Bytes],
                vec![ResolvedType::I64],
                vec![ResolvedType::String],
                vec![ResolvedType::Bytes, ResolvedType::Bytes],
                vec![ResolvedType::TypeParameter {
                    owner: function.id.clone(),
                    index: 0,
                }],
            ] {
                roots.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments,
                });
            }
        }
        for ty in roots {
            let expected = index.classifier_old_facts_for_test(&ty);
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_compute_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                expected
            );
        }
        for ty in [
            ResolvedType::ArrayU8(17),
            ResolvedType::Function {
                parameters: vec![ResolvedType::I64, ResolvedType::Bytes],
                result: Box::new(ResolvedType::I64),
            },
            ResolvedType::TypeParameter {
                owner: function.id.clone(),
                index: 0,
            },
        ] {
            let expected = index.classifier_old_facts_for_test(&ty);
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_compute_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                expected
            );
        }
    }
}
#[test]
fn hostile_cached_facts_and_declined_compound_census_keep_original_precedence() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let mut index = program.declarations.classifier_uncached_for_test();
    let mut types = vec![ResolvedType::Nominal {
        declaration: program.types[0].id.clone(),
        arguments: Vec::new(),
    }];
    // Full owned-key fallback after the old bounded borrowed census declines.
    types.push(ResolvedType::Function {
        parameters: vec![ResolvedType::I64; 128],
        result: Box::new(ResolvedType::Never),
    });
    for ty in types {
        index.classifier_cache_for_test(
            &ty,
            crate::hir::TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "forged-layout-retains-priority".to_owned(),
            },
        );
        let expected = index.type_flags(&ty);
        let mut budget = ReplayBudget::new();
        assert_eq!(flags(&index, function, &ty, &mut budget).unwrap(), expected);
    }
}
#[test]
fn whole_classifier_exact_short_zero_and_sticky_receiving_drop_use_one_payer() {
    let program = fixtures().into_iter().next().unwrap();
    let index = program.declarations.classifier_uncached_for_test();
    let function = &program.functions[0];
    let ty = ResolvedType::Nominal {
        declaration: program.types[2].id.clone(),
        arguments: Vec::new(),
    };
    let expected = index.classifier_old_facts_for_test(&ty);
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = index.classifier_compute_for_test(&ty, payer(function, &mut budget));
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = active_remaining();
                    assert!(!reserve_active_required(0));
                    let start = current_metrics();
                    drop(index.classifier_compute_for_test(&ty, payer(function, &mut budget)));
                    let end = current_metrics();
                    assert_eq!(start.allocated, end.allocated);
                    assert_eq!(start.allocations, end.allocations);
                    assert_eq!(before, active_remaining());
                } else {
                    assert_eq!(result.as_ref().unwrap(), &expected);
                }
                drop(result);
                drop(budget);
            })
        })
    };
    let (((), wide), refused, used, _) = observe(usize::MAX);
    assert!(!refused && used > 0 && !wide.invalid && wide.live == 0);
    assert_eq!(wide.allocated, wide.freed);
    assert!(wide.allocated <= used && wide.highwater <= used);
    let (((), exact), refused, exact_used, _) = observe(used);
    assert!(!refused && exact_used == used && !exact.invalid && exact.live == 0);
    assert_eq!(exact.allocated, exact.freed);
    let (((), short), refused, _, first) = observe(used - 1);
    assert!(refused && first.is_some() && !short.invalid && short.live == 0);
    assert_eq!(short.allocated, short.freed);
    let (((), zero), refused, charged, first) = observe(0);
    assert!(refused && first.is_some() && charged == 0 && !zero.invalid && zero.live == 0);
    assert_eq!(zero.allocated, 0);
}
#[test]
fn classifier_current_invocation_binding_refuses_cross_invocation_before_allocation() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let (((), observed), refused, used, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(function).unwrap();
            let mut held = payer(function, &mut budget);
            // Each invocation's TLS setup is outside the refusal observation. This
            // control proves the payer cannot migrate to that new invocation; it
            // does not claim that the TLS budget factory is funded by this payer.
            let ((), _, _, _) = with_limit_usage_refusal(0, || {
                let before = current_metrics();
                let result = held.bytes(1);
                assert!(result.is_err());
                drop(result);
                let after = current_metrics();
                assert_eq!(before.allocated, after.allocated);
                assert_eq!(before.allocations, after.allocations);
            });
            assert!(!reserve_active_required(0));
            drop(held);
            drop(budget);
        })
    });
    assert!(refused && used > 0 && !observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
#[test]
fn ordinary_droppability_callback_preserves_full_old_semantic_errors_and_stops_on_physical_error() {
    for program in fixtures() {
        for item in &program.types {
            let ty = ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            };
            let old = crate::cleanup::type_needs_resource_cleanup(&program, &ty);
            let current = droppability::needs_drop(&program, &ty, |ty| {
                Ok(program.declarations.type_flags(ty))
            })
            .map_err(|failure| match failure {
                droppability::Failure::Semantic(text) => text,
                droppability::Failure::Physical(_) => panic!("semantic-only callback"),
            });
            assert_eq!(old, current);
            let function = &program.functions[0];
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(function).unwrap();
            let mut calls = 0;
            let result = droppability::needs_drop(&program, &ty, |_| {
                calls += 1;
                Err(budget.budget_refusal(function, "classifier physical callback refusal", ""))
            });
            assert_eq!(calls, 1);
            assert!(matches!(result, Err(droppability::Failure::Physical(_))));
        }
    }
}

#[test]
fn full_owned_identity_substitution_and_admission_oracles_keep_every_payload() {
    for program in fixtures() {
        let index = &program.declarations;
        let function = &program.functions[0];
        let owner = &function.id;
        let mut types: Vec<_> = program
            .types
            .iter()
            .map(|item| ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            })
            .collect();
        if let Some(option) = index.type_id("Option") {
            types.push(ResolvedType::Nominal {
                declaration: option.clone(),
                arguments: vec![ResolvedType::String],
            });
        }
        if let Some(result) = index.type_id("Result") {
            types.push(ResolvedType::Nominal {
                declaration: result.clone(),
                arguments: vec![ResolvedType::String, ResolvedType::I64],
            });
        }
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::I64; 128],
            result: Box::new(ResolvedType::I64),
        });
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 0,
            }],
            result: Box::new(ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 1,
            }),
        });
        for ty in types {
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_key_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                Some(ty.identity_key())
            );
            for arguments in [
                vec![],
                vec![ResolvedType::Bytes],
                vec![ResolvedType::Bytes, ResolvedType::I64],
            ] {
                assert_eq!(
                    index
                        .classifier_substitute_for_test(
                            &ty,
                            owner,
                            &arguments,
                            payer(function, &mut budget)
                        )
                        .unwrap(),
                    crate::hir::substitute_type(&ty, owner, &arguments).ok()
                );
            }
            assert_eq!(
                index.classifier_admission_for_test(&ty),
                (
                    crate::hir::ResolvedImportResultKind::is_owned_container_type(&ty, index),
                    crate::hir::type_reachability::is_admitted_concrete_owned_byte_variant(
                        index, &ty
                    )
                )
            );
        }
    }
}
