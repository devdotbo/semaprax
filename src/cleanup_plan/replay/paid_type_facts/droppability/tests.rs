//! Full old walks, hostile payloads and actual receiving/Drop on one payer.
use super::super::{tests::fixtures, tests::payer, ReplayBudget, ResolvedProgram};
use super::*;
use crate::bounded_output::{active_remaining, reserve_active_required, with_limit_usage_refusal};
use crate::hir::{TypeFacts, TypeFactsPayer};
use crate::test_allocation_observer::{current_metrics, measure};

fn semantic(result: Result<bool, Failure>) -> Result<bool, String> {
    result.map_err(|failure| match failure {
        Failure::Semantic(message) => message,
        Failure::Physical(error) => panic!("unexpected physical refusal: {error:?}"),
    })
}
fn roots(program: &ResolvedProgram) -> Vec<ResolvedType> {
    let mut roots: Vec<_> = program
        .types
        .iter()
        .map(|item| ResolvedType::Nominal {
            declaration: item.id.clone(),
            arguments: Vec::new(),
        })
        .collect();
    for function in &program.functions {
        roots.push(function.return_type.clone());
        roots.extend(function.params.iter().map(|parameter| parameter.ty.clone()));
    }
    roots.extend([
        ResolvedType::Never,
        ResolvedType::Bytes,
        ResolvedType::String,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::StringMap,
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
        ResolvedType::ArrayU8(37),
        ResolvedType::TypeParameter {
            owner: program.functions[0].id.clone(),
            index: 0,
        },
        ResolvedType::Nominal {
            declaration: program.functions[0].id.clone(),
            arguments: vec![ResolvedType::Bytes],
        },
        ResolvedType::Function {
            parameters: vec![ResolvedType::Bytes; 128],
            result: Box::new(ResolvedType::I64),
        },
    ]);
    for item in &program.types {
        for arguments in [
            vec![ResolvedType::Bytes],
            vec![ResolvedType::Str],
            vec![ResolvedType::Bytes, ResolvedType::I64],
        ] {
            roots.push(ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments,
            });
        }
    }
    roots
}
pub(super) fn parity(program: &ResolvedProgram, ty: &ResolvedType) {
    let old = crate::cleanup::type_needs_resource_cleanup(program, ty);
    assert_eq!(
        old,
        semantic(super::needs_drop(program, ty, |ty| Ok(program
            .declarations
            .type_flags(ty))))
    );
    let function = &program.functions[0];
    let mut saved_budget = ReplayBudget::new();
    assert_eq!(
        old,
        semantic(super::saved_paid::needs_drop(
            program,
            ty,
            &mut payer(function, &mut saved_budget)
        ))
    );
    let mut budget = ReplayBudget::new();
    assert_eq!(
        old,
        semantic(super::needs_drop_paid(
            program,
            ty,
            &mut payer(function, &mut budget)
        ))
    );
    let expected = old.map_err(|message| super::super::replay_error(function, message));
    let mut budget = ReplayBudget::new();
    assert_eq!(
        expected,
        super::super::needs_drop(program, function, ty, &mut budget)
    );
}
#[test]
fn paid_outer_full_success_and_hostile_corpus_preserves_complete_original() {
    for program in fixtures() {
        for ty in roots(&program) {
            parity(&program, &ty);
        }
        let mut changed = program.clone();
        for ty in roots(&program) {
            changed.declarations.classifier_cache_for_test(
                &ty,
                TypeFacts {
                    copy: false,
                    contains_resource: true,
                    sized: false,
                    needs_drop: true,
                    layout_key: "hostile-exact-cache-precedence".to_owned(),
                },
            );
            parity(&changed, &ty);
        }
        // A cached outer fact must not hide the receiver's independent foreign
        // owner/missing argument substitution or its original first field error.
        for item_index in 0..program.types.len() {
            for index in [0, u32::MAX] {
                let mut changed = program.clone();
                let item = &mut changed.types[item_index];
                let declaration = item.id.clone();
                let fields = match &mut item.kind {
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => fields,
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        let Some(case) = cases.iter_mut().find(|case| !case.fields.is_empty())
                        else {
                            continue;
                        };
                        &mut case.fields
                    }
                    ResolvedTypeDeclarationKind::Resource { .. } => continue,
                };
                let Some(first) = fields.first_mut() else {
                    continue;
                };
                first.ty = ResolvedType::TypeParameter {
                    owner: program.functions[0].id.clone(),
                    index,
                };
                let ty = ResolvedType::Nominal {
                    declaration,
                    arguments: Vec::new(),
                };
                changed.declarations.classifier_cache_for_test(
                    &ty,
                    TypeFacts {
                        copy: false,
                        contains_resource: true,
                        sized: false,
                        needs_drop: true,
                        layout_key: "outer-needs-drop-retains-substitution-error".to_owned(),
                    },
                );
                parity(&changed, &ty);
            }
        }
    }
}

fn owned_walk() -> (ResolvedProgram, ResolvedType) {
    let mut program = fixtures().into_iter().next().unwrap();
    let item = program
        .types
        .iter_mut()
        .find(|item| {
            matches!(&item.kind,
        ResolvedTypeDeclarationKind::Record { fields } if !fields.is_empty())
        })
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut item.kind else {
        unreachable!()
    };
    let child = ResolvedType::Function {
        parameters: vec![ResolvedType::Bytes; 128],
        result: Box::new(ResolvedType::I64),
    };
    let mut field = fields[0].clone();
    field.ty = child.clone();
    *fields = vec![field; 16].into();
    let root = ResolvedType::Nominal {
        declaration: item.id.clone(),
        arguments: Vec::new(),
    };
    for ty in [&root, &child] {
        program.declarations.classifier_cache_for_test(
            ty,
            TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "force-complete-owned-pending-visited-payload".to_owned(),
            },
        );
    }
    (program, root)
}
#[test]
fn paid_outer_exact_short_zero_sticky_drop_accounts_actual_backing_separately() {
    let (program, ty) = owned_walk();
    parity(&program, &ty);
    let function = &program.functions[0];
    let expected = crate::cleanup::type_needs_resource_cleanup(&program, &ty).unwrap();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = super::super::needs_drop(&program, function, &ty, &mut budget);
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = active_remaining();
                    assert!(!reserve_active_required(0));
                    let start = current_metrics();
                    drop(super::super::needs_drop(
                        &program,
                        function,
                        &ty,
                        &mut budget,
                    ));
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
    let (((), wide), refused, acquired, _) = observe(usize::MAX);
    assert!(!refused && acquired > 0 && !wide.invalid && wide.live == 0);
    assert_eq!(wide.allocated, wide.freed);
    // Acquired bytes include controls and conservative absent-key BTree node
    // upper bounds; they are not an assertion of physical allocation equality.
    assert!(wide.allocated <= acquired && wide.highwater <= acquired);
    let (((), exact), refused, used, _) = observe(acquired);
    assert!(!refused && used == acquired && !exact.invalid && exact.live == 0);
    assert_eq!(exact.allocated, exact.freed);
    let (((), short), refused, _, first) = observe(acquired - 1);
    assert!(refused && first.is_some() && !short.invalid && short.live == 0);
    assert_eq!(short.allocated, short.freed);
    let (((), zero), refused, used, first) = observe(0);
    assert!(refused && first.is_some() && used == 0 && zero.allocated == 0 && zero.live == 0);
}

// Semantic key-floor oracle deliberately has no physical ledger: the two full
// key bodies get separate identical logical budgets. Scope: this is independent
// UNPAID logical-frontier parity against the original identity_key (whose
// format! is the budgeted macro), not a funding claim and not a closed unpriced
// comparison. Physical funding is exercised separately with the real payer
// (see key_tests) through Drop.
pub(super) struct SemanticPayer;
impl TypeFactsPayer for SemanticPayer {
    fn bytes(&mut self, _: usize) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
        values.reserve(additional);
        Ok(())
    }
    fn node<K, V>(&mut self) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn clone_type(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        Ok(ty.clone())
    }
    fn boxed_type(&mut self, ty: ResolvedType) -> Result<Box<ResolvedType>, Diagnostic> {
        Ok(Box::new(ty))
    }
    fn construction_refusal(&mut self, message: &'static str) -> Diagnostic {
        Diagnostic::io_static("SPX-H006", message)
    }
}
#[test]
fn paid_outer_key_preserves_raw_and_capped_original_frontiers_at_selected_floors() {
    let program = fixtures().into_iter().next().unwrap();
    let mut values = roots(&program);
    let owner = &program.functions[0].id;
    values.push(ResolvedType::Function {
        parameters: vec![
            ResolvedType::Nominal {
                declaration: owner.clone(),
                arguments: vec![ResolvedType::Bytes; 3]
            };
            3
        ],
        result: Box::new(ResolvedType::TypeParameter {
            owner: owner.clone(),
            index: 3,
        }),
    });
    for ty in values {
        for limit in [0, 1, 2, 3, 8, 15, 64, 127, 4096, usize::MAX] {
            for floor in [0, limit.min(3)] {
                let old = with_limit_usage_refusal(limit, || {
                    assert!(crate::bounded_output::set_active_floor(floor));
                    ty.identity_key()
                });
                let paid = with_limit_usage_refusal(limit, || {
                    assert!(crate::bounded_output::set_active_floor(floor));
                    super::key::identity(&ty, &mut SemanticPayer).unwrap()
                });
                assert_eq!(
                    old, paid,
                    "original raw/capped key frontier at {limit}/{floor}"
                );
            }
        }
    }
}
#[test]
fn paid_outer_raw_semantic_and_final_replay_text_use_the_same_existing_payer() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let message = "type `unknown:完整` has no cleanup facts";
    let expected = super::super::replay_error(function, message);
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut payer = payer(function, &mut budget);
                let result =
                    super::text::raw(&mut payer, format_args!("{message}")).and_then(|message| {
                        super::text::replay_diagnostic(function, &message, &mut payer)
                    });
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.as_ref().unwrap(), &expected);
                }
                drop(result);
                drop(payer);
                drop(budget);
            })
        })
    };
    let (((), metrics), refused, used, _) = observe(usize::MAX);
    assert!(
        !refused
            && metrics.live == 0
            && metrics.allocated == metrics.freed
            && metrics.allocated <= used
    );
    let (((), exact), refused, charged, _) = observe(used);
    assert!(!refused && charged == used && exact.live == 0);
    let (((), short), refused, _, first) = observe(used - 1);
    assert!(refused && first.is_some() && short.live == 0 && short.allocated == short.freed);
    let (((), zero), refused, used, first) = observe(0);
    assert!(refused && first.is_some() && used == 0 && zero.allocated == 0);
}
#[test]
fn paid_outer_receiving_unwind_drops_owned_result_and_budget_without_new_backing() {
    let (program, ty) = owned_walk();
    let function = &program.functions[0];
    let payload: Box<dyn std::any::Any + Send> = Box::new("preexisting unwind payload");
    let ((payload, metrics), refused, acquired, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut budget = ReplayBudget::new();
                let _result =
                    super::super::needs_drop(&program, function, &ty, &mut budget).unwrap();
                let mut held = payer(function, &mut budget);
                let message =
                    super::text::raw(&mut held, format_args!("held semantic unwind text")).unwrap();
                let _diagnostic =
                    super::text::replay_diagnostic(function, &message, &mut held).unwrap();
                std::panic::resume_unwind(payload);
            }));
            assert!(result.is_err());
            // Return the caller-owned preexisting panic carrier for Drop outside measure.
            result.err().unwrap()
        })
    });
    drop(payload);
    assert!(!refused && acquired > 0 && !metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
