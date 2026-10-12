use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn cfg_owner_exact_setup_short_and_paid_first_refusal_settle_through_drop() {
    assert_eq!(MESSAGE.len(), 46);
    let request = size_of::<CfgReservation>() + MESSAGE.len();
    let (((), observed), refused, used, first) = with_limit_usage_refusal(request, || {
        // The existing Budget/Rc context is acquired outside this observer.
        measure(|| {
            let owner = CfgReservation::acquire().unwrap();
            let pointer = owner.text_pointer();
            let reason = owner.reserve(1).unwrap_err();
            assert_eq!(owner.reserve(0), Err(reason));
            assert_eq!(owner.reserve(usize::MAX), Err(reason));
            assert_eq!(owner.first(), Some(reason));
            let report = owner.diagnostic();
            assert_eq!(report.message.as_ptr(), pointer);
            assert_eq!(report.code, "SPX-H006");
            assert_eq!(report.message, MESSAGE);
            drop(report);
        })
    });
    assert!(refused);
    assert_eq!(used, request);
    assert_eq!(first.unwrap().requested, 1);
    assert_eq!(observed.allocations, 1);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocated, MESSAGE.len());
    assert_eq!(observed.freed, MESSAGE.len());
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((reason, observed), refused, used, first) = with_limit_usage_refusal(request - 1, || {
        measure(|| CfgReservation::acquire().err().unwrap())
    });
    assert!(matches!(reason, Refused::Budget(_)));
    assert!(refused);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, request);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.live, 0);
}

#[test]
fn cfg_owner_nested_and_escaped_zero_requests_cannot_change_first_receipt() {
    let request = size_of::<CfgReservation>() + MESSAGE.len();
    let (owner, refused, used) = with_limit_usage(request + 1, || {
        let owner = CfgReservation::acquire().unwrap();
        let (reason, inner_refused, inner_used) = with_limit_usage(0, || {
            owner.reserve(0).unwrap_err()
        });
        assert_eq!(reason, Refused::Detached);
        assert!(!inner_refused);
        assert_eq!(inner_used, 0);
        assert_eq!(owner.reserve(0), Err(Refused::Detached));
        assert_eq!(owner.first(), Some(Refused::Detached));
        owner
    });
    assert!(refused);
    assert_eq!(used, request);
    assert_eq!(owner.reserve(0), Err(Refused::Detached));
    drop(owner);
    let (reason, observed) = measure(|| CfgReservation::acquire().err().unwrap());
    assert_eq!(reason, Refused::NoOwner);
    assert_eq!(observed.allocations, 0);
}
