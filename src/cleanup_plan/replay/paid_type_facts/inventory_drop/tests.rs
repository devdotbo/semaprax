use super::*;
use crate::hir::TypeFacts;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
fn run(
    bytes: usize,
    work: usize,
    program: &ResolvedProgram,
    root: &ResolvedType,
    expected: &Result<bool, Diagnostic>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(bytes, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "complete outer receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        needs_drop(
                            program,
                            root,
                            &mut super::super::tests::payer(function, &mut budget),
                        )
                    });
                let matched = &result == expected; // all six Diagnostic fields
                let physical = budget.refusal_spent || crate::bounded_output::active_refused();
                let used = work.saturating_sub(budget.remaining);
                if physical {
                    let before = current_metrics();
                    let retry = needs_drop(
                        program,
                        root,
                        &mut super::super::tests::payer(function, &mut budget),
                    );
                    assert!(retry.is_err());
                    assert_eq!(before.allocated, current_metrics().allocated);
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, physical, used)
            })
        });
    (outcome, observed, refused, paid)
}
fn control(program: &ResolvedProgram, root: &ResolvedType) {
    let expected = crate::cleanup::type_needs_resource_cleanup(program, root)
        .map_err(|message| Diagnostic::io("SPX-H006", message));
    let (ample, observed, refused, paid) =
        run(usize::MAX, MAX_REPLAY_WORK_UNITS, program, root, &expected);
    assert!(ample.0 && !ample.1 && !refused && !observed.invalid);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    let (exact, observed, refused, used) = run(paid, ample.2, program, root, &expected);
    assert!(exact.0 && !exact.1 && !refused && !observed.invalid && used == paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    for (bytes, work) in [
        (0, MAX_REPLAY_WORK_UNITS),
        (paid.saturating_sub(1), MAX_REPLAY_WORK_UNITS),
        (paid, 0),
        (paid, ample.2.saturating_sub(1)),
    ] {
        if bytes == paid && work == ample.2 {
            continue;
        }
        let (short, observed, _, used) = run(bytes, work, program, root, &expected);
        assert!(short.1 && !observed.invalid && observed.allocated <= used);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn complete_outer_bool_receiver_keeps_original_fields_cases_substitution_priority_and_drop() {
    for mut program in super::super::tests::fixtures() {
        program.declarations = program.declarations.classifier_uncached_for_test();
        let mut roots = Vec::new();
        for item in &program.types {
            roots.push(ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            });
        }
        for f in &program.functions {
            roots.push(f.return_type.clone());
            roots.extend(f.params.iter().map(|p| p.ty.clone()));
        }
        roots.extend([
            ResolvedType::Never,
            ResolvedType::String,
            ResolvedType::Str,
            ResolvedType::MutFunctionI64,
            ResolvedType::ArrayU8(usize::MAX),
        ]);
        for root in roots {
            control(&program, &root);
        }
    }
}
#[test]
fn full_outer_unknown_copy_forged_cache_diagnostics_and_generic_zero_one_seventeen_keep_old_receiver(
) {
    let base = super::super::tests::fixtures().into_iter().next().unwrap();
    for width in [0, 1, 17] {
        let root = ResolvedType::Nominal {
            declaration: DeclarationId::new("unknown.é\0suffix"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::I64],
                    result: Box::new(ResolvedType::Bytes)
                };
                width
            ],
        };
        control(&base, &root);
        let mut program = base.clone();
        program.declarations.inventory_insert_fact_for_test(
            &root,
            TypeFacts {
                copy: false,
                contains_resource: true,
                sized: true,
                needs_drop: true,
                layout_key: "full forged layout".to_owned(),
            },
        );
        control(&program, &root);
    }
    for root in [
        ResolvedType::I64,
        ResolvedType::Str,
        ResolvedType::MutFunctionI64,
        ResolvedType::Never,
    ] {
        let mut program = base.clone();
        program.declarations.inventory_insert_fact_for_test(
            &root,
            TypeFacts {
                copy: false,
                contains_resource: true,
                sized: true,
                needs_drop: true,
                layout_key: "forged".to_owned(),
            },
        );
        control(&program, &root);
    }
}
#[test]
fn original_outer_paid_receiver_remains_complete_independent_and_byte_exact() {
    assert_eq!(
        include_str!("../droppability/paid.rs"),
        include_str!("../../../../../docs/evidence/schema-summary/droppability-paid.rs.before")
    );
}
