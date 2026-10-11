//! Full trace replay is independent of receiving-only profile acceptance.
use super::*;

fn run(
    program: &crate::hir::ResolvedProgram,
    work: usize,
    bytes: usize,
) -> (
    Result<(usize, usize), ()>,
    crate::test_allocation_observer::Metrics,
) {
    crate::test_allocation_observer::measure(|| {
        (|| {
            let anchor = &program.functions[0];
            let mut preparation = Preparation::with_limits(anchor, work, bytes)?;
            let index = declarations(program, &mut preparation)?;
            let provider = Provider {
                program,
                declarations: index,
            };
            let mut session = Session::from_preparation(&provider, anchor, preparation)?;
            for function in &program.functions[..2] {
                session.borrowed_replay(function)?;
            }
            let (remaining, charged) = session.metrics();
            let receipt = (work - remaining, charged);
            drop(session);
            drop(provider);
            Ok::<_, Diagnostic>(receipt)
        })()
        .map_err(|diagnostic| {
            assert_eq!(diagnostic.code, "SPX-H006");
        })
    })
}

#[test]
fn actual_option_helpers_full_replay_matches_ordinary_and_metering_through_drop() {
    let program = program();
    for function in &program.functions[..2] {
        validate_structure(&program, function).unwrap();
    }
    let (wide, allocation) = run(&program, MAX_REPLAY_WORK_UNITS, 1 << 24);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
    let (exact, allocation) = run(&program, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
    for (work, bytes) in [(work - 1, bytes), (work, bytes - 1)] {
        let (answer, allocation) = run(&program, work, bytes);
        assert!(answer.is_err());
        assert!(!allocation.invalid && allocation.live == 0);
    }
}

#[test]
fn option_profile_cannot_bless_forged_case_or_result_trace_and_failure_is_sticky() {
    for mutation in 0..4 {
        let mut program = program();
        let other_result = program.functions[0].body.id.clone();
        let function = &mut program.functions[1];
        match mutation {
            0 => {
                let case = function
                    .cleanup_plan
                    .edges
                    .iter_mut()
                    .find_map(|edge| {
                        if let EdgeCondition::VariantCase { case, .. } = &mut edge.condition {
                            Some(case)
                        } else {
                            None
                        }
                    })
                    .expect("actual Option decision edge");
                *case = DeclarationId::new("forged.option.case");
            }
            1 => {
                let matches = function
                    .cleanup_plan
                    .edges
                    .iter_mut()
                    .find_map(|edge| {
                        if let EdgeCondition::VariantCase { matches, .. } = &mut edge.condition {
                            Some(matches)
                        } else {
                            None
                        }
                    })
                    .expect("actual Option decision edge");
                *matches = !*matches;
            }
            2 => {
                let source = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find_map(|exit| {
                        if let ExitContinuation::CommitResult { source } = &mut exit.continuation {
                            Some(source)
                        } else {
                            None
                        }
                    })
                    .expect("actual scalar result commit");
                *source = CleanupResultSource::Scalar {
                    expression: other_result,
                };
            }
            _ => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| {
                        matches!(&exit.continuation, ExitContinuation::CommitResult { .. })
                    })
                    .expect("actual scalar result commit");
                exit.continuation = ExitContinuation::ReturnUnit;
            }
        }
        let anchor = &program.functions[0];
        let mut preparation =
            Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, 1 << 24).unwrap();
        let index = declarations(&program, &mut preparation).unwrap();
        let provider = Provider {
            program: &program,
            declarations: index,
        };
        let mut session = Session::from_preparation(&provider, anchor, preparation).unwrap();
        session.borrowed_replay(anchor).unwrap();
        assert_eq!(
            session
                .borrowed_replay(&program.functions[1])
                .unwrap_err()
                .code,
            "SPX-H006",
            "trace mutant {mutation}"
        );
        let remaining = session.metrics().0;
        assert_eq!(
            session.borrowed_replay(anchor).unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(session.metrics().0, remaining);
        assert_eq!(session.finish().err().unwrap().code, "SPX-H006");
    }
}
