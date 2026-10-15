//! Genuine original4096 factory/join, never a constructed Selected or None.
use super::*;
use crate::hir::selected_fused_candidate::{validate_selected, Selected};
use crate::test_allocation_observer::{current_metrics, Metrics};

fn old(program: &ResolvedProgram, selected: Selected<'_>) -> Result<bool, Diagnostic> {
    crate::hir::byte_partition::refuse_default(program)?;
    crate::hir::agent_validation::validate(program)?;
    let output = crate::hir::inspection::validate_with_static_text(program, Some(selected))?;
    crate::loan_plan::validate_program(program)?;
    Ok(output)
}
fn old_stages(
    program: &ResolvedProgram,
    selected: Selected<'_>,
    stages: &mut [Metrics; 3],
) -> Result<bool, Diagnostic> {
    crate::hir::byte_partition::refuse_default(program)?;
    crate::hir::agent_validation::validate(program)?;
    stages[0] = current_metrics();
    let output = crate::hir::inspection::validate_with_static_text(program, Some(selected))?;
    stages[1] = current_metrics();
    crate::loan_plan::validate_program(program)?;
    stages[2] = current_metrics();
    Ok(output)
}
#[test]
fn genuine_selected_4096_join_preserves_saved_final_boundary_plan_and_error_priority() {
    let source = crate::parse(
        include_str!("../../../../../tests/fixtures/record-traversal-maximum.spx"),
        "selected-final-loans.spx",
    )
    .unwrap();
    let owner = validate_selected(&source).unwrap_or_else(|error| panic!("factory: {error:?}"));
    let program = owner.program();
    assert!(crate::hir::validate(program).is_err());
    owner
        .with_selected_for_test(program, |joined, selected| {
            assert!(joined.still_current(&source, program));
            assert_eq!(joined.flow().rows, 4096);
            assert!(selected.held_by(program));
            assert_eq!(
                crate::hir::validate_selected_with_static_text(program, Some(selected)).unwrap(),
                old(program, selected).unwrap()
            );
            let mut stages = [Metrics::default(); 3];
            let (_, saved) = measure(|| old_stages(program, selected, &mut stages).unwrap());
            let (_, current) = measure(|| {
                crate::hir::validate_selected_with_static_text(program, Some(selected)).unwrap()
            });
            let (_, removed) = measure(|| crate::loan_plan::validate_program(program).unwrap());
            // Copy snapshots are captured without allocations; render them
            // only after each complete constructor/replay/Drop observation.
            println!("selected-final original stages={stages:?} saved={saved:?} current={current:?} removed={removed:?}");
            // Opposite measurement order exposes address-dependent scratch
            // allocation separately; no tolerance or normalized debit follows.
            let (_, current_first) = measure(|| {
                crate::hir::validate_selected_with_static_text(program, Some(selected)).unwrap()
            });
            let mut second_stages = [Metrics::default(); 3];
            let (_, saved_second) = measure(|| old_stages(program, selected, &mut second_stages).unwrap());
            let (_, removed_second) = measure(|| crate::loan_plan::validate_program(program).unwrap());
            println!("selected-final opposite stages={second_stages:?} saved={saved_second:?} current={current_first:?} removed={removed_second:?}");
            assert_eq!(saved.allocations, current.allocations + removed.allocations);
            assert_eq!(
                saved.reallocations,
                current.reallocations + removed.reallocations
            );
            assert_eq!(saved.allocated, current.allocated + removed.allocated);
            assert_eq!(saved_second.allocations, current_first.allocations + removed_second.allocations);
            assert_eq!(saved_second.reallocations, current_first.reallocations + removed_second.reallocations);
            assert_eq!(saved_second.allocated, current_first.allocated + removed_second.allocated);
            assert_eq!(saved, saved_second);
            assert_eq!(current, current_first);
            assert_eq!(removed, removed_second);
            for observed in [saved, current, removed, current_first, saved_second, removed_second] {
                assert!(!observed.invalid);
                assert_eq!(observed.live, 0);
                assert_eq!(observed.allocated, observed.freed);
            }
        })
        .unwrap();
    let main = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "app.main")
        .unwrap();
    assert!(!program.functions[main].loan_plan.loans.is_empty());
    for mutation in 0..3 {
        let mut candidate = program.clone();
        let function = &mut candidate.functions[main];
        match mutation {
            0 => function.loan_plan.schema = "forged.selected",
            1 => {
                assert!(!function.loan_plan.endpoints.is_empty());
                function.loan_plan.endpoints.pop();
            }
            2 => {
                function.loan_plan.schema = "forged.selected";
                function.cleanup_plan.schema = "forged.cleanup";
            }
            _ => unreachable!(),
        }
        // The real join must succeed; join refusal cannot stand for a loan
        // validator or earlier-cleanup rejection under Some(selected).
        owner
            .with_selected_for_test(&candidate, |joined, selected| {
                assert!(joined.still_current(&source, &candidate));
                assert_eq!(joined.flow().rows, 4096);
                assert_eq!(
                    receipt(&candidate).finish(&candidate),
                    crate::loan_plan::validate_program(&candidate)
                );
                let expected = old(&candidate, selected).unwrap_err();
                let actual =
                    crate::hir::validate_selected_with_static_text(&candidate, Some(selected))
                        .unwrap_err();
                assert_eq!(actual, expected);
                if mutation == 2 {
                    assert!(!actual.message.contains("forged shared-loan"));
                }
            })
            .unwrap();
    }
}
