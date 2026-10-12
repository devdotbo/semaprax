use super::*;
use crate::bounded_output::{
    reserve_active_required, with_limit_usage_refusal, with_reservation_stage,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn scenario(
    observe: bool,
    refused: bool,
    unwind: bool,
) -> (
    bool,
    usize,
    Option<super::super::ReservationRefusal>,
    Option<Trace>,
) {
    take_completed();
    let (_, overflow, consumed, refusal) = with_limit_usage_refusal(10, || {
        let guard = observe.then(|| {
            let (guard, allocation) = crate::test_allocation_observer::measure(|| start(17, 19, 7));
            assert_eq!(allocation.allocations, 0);
            guard
        });
        with_reservation_stage(ReservationStage::FreshResolution, || {
            if refused {
                assert!(super::super::set_active_floor(9));
                with_limit_usage_refusal(3, || assert!(reserve_active_required(2)));
                assert!(!reserve_active_required(0));
            } else if unwind {
                let error = catch_unwind(AssertUnwindSafe(|| {
                    with_limit_usage_refusal(3, || {
                        assert!(reserve_active_required(3));
                        std::panic::panic_any(13usize);
                    });
                }));
                assert!(error.is_err());
            } else {
                with_limit_usage_refusal(3, || assert!(reserve_active_required(2)));
                with_reservation_stage(ReservationStage::HirValidation, || {
                    assert!(reserve_active_required(4))
                });
            }
        });
        let (_, allocation) = crate::test_allocation_observer::measure(|| drop(guard));
        assert_eq!(allocation.allocations, 0);
    });
    (overflow, consumed, refusal, take_completed())
}

#[test]
fn exact_budget_stage_trace_preserves_nested_settlement_refusal_and_unwind_without_storage() {
    for (refused, unwind, expected) in [(false, false, 6), (true, false, 2), (false, true, 3)] {
        let plain = scenario(false, refused, unwind);
        let watched = scenario(true, refused, unwind);
        assert_eq!(
            (plain.0, plain.1, plain.2),
            (watched.0, watched.1, watched.2)
        );
        assert!(plain.3.is_none());
        let trace = watched.3.unwrap();
        assert_eq!((trace.sum(), trace.consumed), (expected, expected));
        assert_eq!((trace.prebound, trace.layouts, trace.mode), (17, 19, 7));
        assert_eq!(
            trace.debits[ReservationStage::FreshResolution as usize + 1],
            if !refused && unwind { 3 } else { 2 }
        );
        assert_eq!(
            trace.debits[ReservationStage::HirValidation as usize + 1],
            if refused || unwind { 0 } else { 4 }
        );
    }
    take_completed();
    let error = catch_unwind(AssertUnwindSafe(|| {
        with_limit_usage_refusal(10, || {
            let _guard = start(0, 0, 6);
            assert!(reserve_active_required(3));
            std::panic::panic_any(17usize);
        });
    }));
    assert!(error.is_err());
    assert_eq!(take_completed().unwrap().consumed, 3);
    // Unwind restored the inactive observer; an unrelated ledger is not traced.
    with_limit_usage_refusal(10, || assert!(reserve_active_required(1)));
    assert!(take_completed().is_none());
    // Snapshot selection mirrors the existing max-consumed-attempt policy.
    take_completed();
    for bytes in [7, 3] {
        with_limit_usage_refusal(10, || {
            let _guard = start(bytes, 0, 6);
            assert!(reserve_active_required(bytes));
        });
    }
    assert_eq!(take_completed().unwrap().consumed, 7);
}
