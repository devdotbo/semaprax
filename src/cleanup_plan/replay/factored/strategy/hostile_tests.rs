//! Above-cap sources traverse production authentication before factored proof.
use super::*;

fn large_generated() -> (ResolvedProgram, ResolvedFunction) {
    large_generated_with_storage(false)
}
fn large_generated_with_storage(cleanup: bool) -> (ResolvedProgram, ResolvedFunction) {
    let choices = (0..17)
        .map(|index| format!("let choice_{index}=if n==0 {{0}}else{{1}};\n"))
        .collect::<String>();
    let source = lexical_plan_tests::GENERATED.replace(
        "        if item==1 {continue;};",
        &format!("{choices}        if item==1 {{continue;}};"),
    );
    let source = if cleanup {
        source.replace(
            "    let mut building=",
            "    let first=\"first\"; let second=\"second\";\n    let mut building=",
        )
    } else {
        source
    };
    let fixture = lexical_plan_tests::fixture(&source);
    let mut budget = ReplayBudget::new();
    let candidate = Candidate::reconstruct(&fixture.0, &fixture.1, &mut budget).unwrap();
    assert!(matches!(candidate.hir, Census::Exceeded { .. }));
    assert_eq!(candidate.hir, candidate.plan);
    assert_eq!(candidate.strategy, Strategy::LexicalFactored);
    fixture
}

fn rejected(
    program: &ResolvedProgram,
    original: &ResolvedFunction,
    forged: &ResolvedFunction,
    unchanged_classes: bool,
) {
    if unchanged_classes {
        let mut budget = ReplayBudget::new();
        let before = Candidate::reconstruct(program, original, &mut budget).unwrap();
        let mut budget = ReplayBudget::new();
        let after = Candidate::reconstruct(program, forged, &mut budget).unwrap();
        assert_eq!(
            before.plan, after.plan,
            "attack preserves exact terminal classes"
        );
        assert_eq!(
            after.hir, after.plan,
            "equal route totals confer no authority"
        );
    }
    assert_eq!(
        validate_structure(program, forged).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn above_cap_lexical_sites_owners_and_nearest_targets_fail_production_authentication() {
    let (program, function) = large_generated();
    let owner = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    for attack in 0..3 {
        let mut forged = function.clone();
        let certificate = forged
            .cleanup_plan
            .lexical_control
            .as_mut()
            .unwrap()
            .exits
            .iter_mut()
            .find(|exit| exit.kind == crate::ast::LexicalExitKind::Continue)
            .unwrap();
        match attack {
            0 => certificate.site = ExpressionId::new(&owner, "forged.continue.site"),
            1 => {
                let crate::hir::ControlTarget::Loop { owner, .. } = &mut certificate.target else {
                    panic!("loop target")
                };
                *owner =
                    crate::hir::FunctionExecutionId::Monomorphic(DeclarationId::new("wrong.owner"));
            }
            2 => {
                let crate::hir::ControlTarget::Loop { site, .. } = &mut certificate.target else {
                    panic!("loop target")
                };
                *site = function.body.id.clone();
            }
            _ => unreachable!(),
        }
        rejected(&program, &function, &forged, true);
    }
}

#[test]
fn above_cap_generated_advancement_resumes_and_latch_ownership_remain_exact() {
    let (program, function) = large_generated();
    for attack in 0..4 {
        let mut forged = function.clone();
        let control = forged.cleanup_plan.lexical_control.as_mut().unwrap();
        match attack {
            0 => {
                let certificate = control
                    .exits
                    .iter_mut()
                    .find(|exit| exit.kind == crate::ast::LexicalExitKind::Continue)
                    .unwrap();
                assert!(certificate.resume.take().is_some());
            }
            1 => {
                let resume = control
                    .resumes
                    .iter_mut()
                    .find(|resume| !resume.blocks.is_empty())
                    .unwrap();
                resume.blocks.pop().unwrap();
            }
            2 => {
                let certificate = control
                    .loops
                    .iter_mut()
                    .find(|certificate| !certificate.entry_order.is_empty())
                    .unwrap();
                certificate.entry_order.clear();
            }
            3 => {
                let exit = forged
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| matches!(exit.continuation, ExitContinuation::LoopLatch { .. }))
                    .unwrap();
                let ExitContinuation::LoopLatch {
                    target: crate::hir::ControlTarget::Loop { site, .. },
                } = &mut exit.continuation
                else {
                    panic!("loop latch")
                };
                *site = function.body.id.clone();
            }
            _ => unreachable!(),
        }
        rejected(&program, &function, &forged, attack != 0);
    }
}

#[test]
fn above_cap_sticky_failure_atomic_commit_result_and_cleanup_order_remain_exact() {
    let (program, function) = large_generated_with_storage(true);
    for attack in 0..4 {
        let mut forged = function.clone();
        match attack {
            0 => {
                let selected = forged
                    .cleanup_plan
                    .blocks
                    .iter()
                    .flat_map(|block| &block.transitions)
                    .find_map(|transition| match transition {
                        CleanupTransition::SelectFailure { source } => Some(source.clone()),
                        _ => None,
                    })
                    .unwrap();
                let other = forged
                    .cleanup_plan
                    .status_sources
                    .iter()
                    .find(|source| source.id != selected)
                    .unwrap()
                    .id
                    .clone();
                let transition = forged
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.transitions)
                    .find(|transition| {
                        matches!(transition, CleanupTransition::SelectFailure { .. })
                    })
                    .unwrap();
                let CleanupTransition::SelectFailure { source } = transition else {
                    unreachable!()
                };
                *source = other;
            }
            1 => {
                let transition = forged.cleanup_plan.blocks.iter_mut().flat_map(|block| &mut block.transitions)
                    .find(|transition| matches!(transition, CleanupTransition::CallCommit { arguments, .. } if !arguments.is_empty())).unwrap();
                let CleanupTransition::CallCommit { arguments, .. } = transition else {
                    unreachable!()
                };
                arguments.clear();
            }
            2 => {
                let exit = forged
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| {
                        matches!(exit.continuation, ExitContinuation::FunctionReturn { .. })
                    })
                    .unwrap();
                let ExitContinuation::FunctionReturn {
                    source: CleanupResultSource::Scalar { expression },
                    ..
                } = &mut exit.continuation
                else {
                    panic!("scalar return")
                };
                assert_ne!(expression, &function.body.id);
                *expression = function.body.id.clone();
            }
            3 => {
                let exit = forged
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| exit.finalize_in_order.len() > 1)
                    .unwrap();
                exit.finalize_in_order.reverse();
            }
            _ => unreachable!(),
        }
        rejected(&program, &function, &forged, true);
    }
    // Hostile proof metadata never repairs the authoritative immutable source.
    validate_structure(&program, &function).unwrap();
}
