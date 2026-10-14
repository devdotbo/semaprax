use super::*;
use crate::bounded_output::{self, ReservationStage};
use crate::test_allocation_observer::measure;

#[test]
fn actual_prepare_stage_keeps_full_old_storage_diagnostics_numeric_receipts_and_drop() {
    let program = super::super::tests::fixture();
    let seed = ConstructorSeed::authenticate(&program, None).unwrap();
    let (expected, deferred) = owned_before::prepare_original(&seed, &program).unwrap();
    drop(deferred);
    let run = |saved, limit| {
        bounded_output::with_limit_usage_refusal(limit, || {
            bounded_output::with_reservation_stage(ReservationStage::FreshResolution, || {
                measure(|| {
                    let result = if saved {
                        owned_before::prepare_original(&seed, &program)
                    } else {
                        prepare_with_comparison(&seed, &program)
                    };
                    let diagnostic = match result {
                        Ok((storage, deferred)) => {
                            assert_eq!(storage.ids, expected.ids);
                            assert_eq!(storage.liveness, expected.liveness);
                            assert_eq!(storage.origins, expected.origins);
                            drop(deferred);
                            drop(storage);
                            None
                        }
                        Err(error) => {
                            assert!(!bounded_output::reserve_active_required(0));
                            let retry = if saved {
                                owned_before::prepare_original(&seed, &program)
                            } else {
                                prepare_with_comparison(&seed, &program)
                            }
                            .err()
                            .unwrap();
                            assert_eq!(retry.code, "SPX-H006");
                            assert!(retry.message.is_empty());
                            drop(retry);
                            Some(error)
                        }
                    };
                    // Retain only fixed diagnostic equality coordinates outside
                    // the observer; owned error backing is freed inside it.
                    let failed = diagnostic.is_some();
                    if let Some(error) = diagnostic {
                        let expected = if limit == 0 {
                            ""
                        } else {
                            "canonical loan CFG storage reservation refused"
                        };
                        assert_eq!(error.code, "SPX-H006");
                        assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                        assert_eq!(error.message, expected);
                        assert!(
                            error.path.is_none() && error.span.is_none() && error.help.is_none()
                        );
                        drop(error);
                    }
                    failed
                })
            })
        })
    };
    let (_, overflow, exact, first) = run(false, usize::MAX);
    assert!(!overflow && first.is_none());
    for limit in [usize::MAX, exact, exact - 1, 0] {
        let old = run(true, limit);
        let current = run(false, limit);
        assert_eq!(current.0, old.0);
        assert_eq!((current.1, current.2), (old.1, old.2));
        let mut normalized = current.3;
        if let Some(receipt) = &mut normalized {
            assert_eq!(receipt.stage, Some(ReservationStage::CanonicalLoanStorage));
            receipt.stage = Some(ReservationStage::FreshResolution);
        }
        assert_eq!(normalized, old.3);
        let metrics = current.0 .1;
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
        assert!(metrics.allocated <= current.2 && metrics.highwater <= current.2);
    }
}
