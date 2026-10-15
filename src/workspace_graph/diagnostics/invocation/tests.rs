//! Diagnostic ownership only; the full lifecycle observer remains independent.
use super::*;
use crate::test_allocation_observer::measure;

fn populate(path: &str) {
    begin_precharge_pass();
    record_precharge_module(path, 17);
    record_sequential_precharge_refusal(path, 2, 3, 5, true);
    begin_precharge_module(path);
}
fn same(left: &Diagnostic, right: &Diagnostic) {
    assert_eq!(left.code, right.code);
    assert_eq!(left.message, right.message);
    assert_eq!(left.help, right.help);
    assert_eq!(left.path, right.path);
    assert_eq!(left.span, right.span);
}

#[test]
fn invocation_diagnostics_preserve_saved_error_text_and_settle_success_and_error_paths() {
    let _restore = enter();
    // Saved unguarded rendering uses the original diagnostic functions.
    let expected = {
        let _phase = precharge_scope();
        populate("src/app.spx");
        limit_error("builder_bytes", 13)
    };
    begin_precharge_pass();
    for fails in [false, true] {
        let (_, observed) = measure(|| {
            let scope = enter();
            {
                let _phase = precharge_scope();
                populate("src/app.spx");
                if fails {
                    let diagnostic = limit_error("builder_bytes", 13);
                    same(&diagnostic, &expected);
                    // Returned diagnostics own their text before scope Drop.
                    drop(scope);
                    same(&diagnostic, &expected);
                    drop(diagnostic);
                    return;
                }
            }
            drop(scope);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        PRECHARGE_DOMINANT.with(|v| assert!(v.borrow().is_none()));
        PRECHARGE_PENDING.with(|v| assert!(v.borrow().is_none()));
        PRECHARGE_SEQUENTIAL.with(|v| assert!(v.borrow().is_none()));
    }
}

#[test]
fn nested_and_unwinding_invocations_restore_exact_outer_attribution_without_cloning() {
    let _restore = enter();
    let _phase = precharge_scope();
    populate("outer.spx");
    let expected = limit_error("builder_bytes", 13);
    let pointer = PRECHARGE_DOMINANT.with(|v| v.borrow().as_ref().unwrap().0.as_ptr());
    let (_, empty) = measure(|| drop(enter()));
    assert_eq!(empty.allocations, 0);
    assert_eq!(empty.allocated, 0);
    assert!(!empty.invalid);
    let (_, observed) = measure(|| {
        let _inner = enter();
        assert!(BUILDER_PHASE.with(Cell::get) == BuilderPhase::Live);
        populate("inner.spx");
        {
            let _nested = enter();
            populate("nested.spx");
        }
        PRECHARGE_PENDING.with(|v| assert_eq!(v.borrow().as_deref(), Some("inner.spx")));
    });
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(!observed.invalid);
    let unwind = std::panic::catch_unwind(|| {
        let _inner = enter();
        populate("unwind.spx");
        panic!("diagnostic scope unwind");
    });
    assert!(unwind.is_err());
    assert!(BUILDER_PHASE.with(Cell::get) == BuilderPhase::Precharge);
    assert_eq!(PRECHARGE_MODULES.with(Cell::get), 1);
    assert_eq!(PRECHARGE_BYTES.with(Cell::get), 17);
    PRECHARGE_DOMINANT.with(|v| assert_eq!(v.borrow().as_ref().unwrap().0.as_ptr(), pointer));
    PRECHARGE_PENDING.with(|v| assert_eq!(v.borrow().as_deref(), Some("outer.spx")));
    PRECHARGE_SEQUENTIAL.with(|v| {
        let value = v.borrow();
        let value = value.as_ref().unwrap();
        assert_eq!(value.path, "outer.spx");
        assert_eq!((value.earlier, value.retained, value.additional), (2, 3, 5));
        assert!(value.resolving);
    });
    same(&limit_error("builder_bytes", 13), &expected);
}

#[test]
fn diagnostic_scope_does_not_restore_or_refund_the_sticky_active_budget() {
    use crate::bounded_output::{self, ReservationStage};
    let (_, overflow, used, refusal) = bounded_output::with_limit_usage_refusal(3, || {
        bounded_output::with_reservation_stage(ReservationStage::ResolvedCore, || {
            assert!(bounded_output::reserve_active_required(2));
            {
                let _scope = enter();
                assert!(!bounded_output::reserve_active_required(2));
            }
            assert!(!bounded_output::reserve_active_required(0));
            assert!(!bounded_output::reserve_active_required(1));
        });
    });
    assert!(overflow);
    assert_eq!(used, 2);
    let refusal = refusal.unwrap();
    assert_eq!(
        (refusal.requested, refusal.remaining, refusal.floor),
        (2, 1, 0)
    );
    assert_eq!(refusal.stage, Some(ReservationStage::ResolvedCore));
}
