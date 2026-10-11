use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn resolver_stage_guards_allocate_nothing_preserve_exact_debits_and_first_refusal() {
    let stages = [
        ReservationStage::HirLowering,
        ReservationStage::HirLoanPlans,
        ReservationStage::HirInstanceLoanPlans,
        ReservationStage::HirCleanupInventories,
        ReservationStage::HirInstanceCleanupInventories,
        ReservationStage::HirCleanupPlans,
        ReservationStage::HirYieldProfiles,
        ReservationStage::HirValidation,
    ];
    for stage in stages {
        let (observed, overflow, charged, refusal) = with_limit_usage_refusal(3, || {
            let (_, observed) = crate::test_allocation_observer::measure(|| {
                with_reservation_stage(ReservationStage::FreshResolution, || {
                    let _guard = reservation_stage_guard(stage);
                    assert!(reserve_active_required(3));
                });
            });
            observed
        });
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 0);
        assert_eq!((overflow, charged, refusal), (false, 3, None));
        let (_, overflow, charged, refusal) = with_limit_usage_refusal(2, || {
            with_reservation_stage(ReservationStage::FreshResolution, || {
                with_reservation_stage(stage, || assert!(!reserve_active_required(3)));
                assert!(!reserve_active_required(0));
            });
        });
        assert!(overflow);
        assert_eq!(charged, 0);
        assert_eq!(refusal.unwrap().stage, Some(stage));
        assert!(!stage.label().is_empty());
    }
}

#[test]
fn resolver_stage_unwind_restores_current_budget_without_erasing_sticky_evidence() {
    let (_, overflow, charged, refusal) = with_limit_usage_refusal(1, || {
        with_reservation_stage(ReservationStage::FreshResolution, || {
            let panic = catch_unwind(AssertUnwindSafe(|| {
                let _guard = reservation_stage_guard(ReservationStage::HirLoanPlans);
                panic!("stage unwind witness");
            }));
            assert!(panic.is_err());
            assert!(!reserve_active_required(2));
            with_reservation_stage(ReservationStage::HirValidation, || {
                assert!(!reserve_active_required(0));
            });
        });
    });
    assert!(overflow);
    assert_eq!(charged, 0);
    assert_eq!(
        refusal.unwrap().stage,
        Some(ReservationStage::FreshResolution)
    );
    assert!(active().is_none());
}

#[test]
fn resolver_first_inner_refusal_survives_unwind_and_nested_budget_restore() {
    let (_, overflow, charged, refusal) = with_limit_usage_refusal(1, || {
        with_reservation_stage(ReservationStage::FreshResolution, || {
            let caught = catch_unwind(AssertUnwindSafe(|| {
                let _guard = reservation_stage_guard(ReservationStage::HirCleanupPlans);
                assert!(!reserve_active_required(2));
                panic!("refused stage unwind");
            }));
            assert!(caught.is_err());
            let child = with_limit_usage_refusal(0, || {
                assert!(!reserve_active_required(1));
            });
            assert_eq!(child.3.unwrap().stage, None);
            assert!(!reserve_active_required(0));
        });
    });
    assert!(overflow);
    assert_eq!(charged, 0);
    assert_eq!(
        refusal.unwrap().stage,
        Some(ReservationStage::HirCleanupPlans)
    );
    assert!(active().is_none());
}
