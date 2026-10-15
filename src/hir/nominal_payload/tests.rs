//! Actual backing/last-owner Drop and refusal before acquisition controls.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::{ast::Span, hir::DeclarationId, test_allocation_observer::measure};
fn field(ty: ResolvedType) -> ResolvedFieldDeclaration {
    ResolvedFieldDeclaration {
        id: DeclarationId::new("field"),
        name: "x".to_owned(),
        index: 0,
        ty,
        span: Span::default(),
    }
}
fn reserve(bytes: usize) -> Result<(), ()> {
    if reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(())
    }
}
#[test]
fn actual_nominal_owner_moves_complete_vec_and_shares_without_backing_clone() {
    let mut fields = vec![field(ResolvedType::I64)];
    let pointer = fields.as_ptr();
    let original = fields.clone();
    let mut debit = 0;
    let (payload, metrics) = measure(|| {
        NominalPayload::from_funded_vec(&mut fields, |bytes| {
            debit += bytes;
            Ok::<_, ()>(())
        })
        .unwrap()
    });
    assert!(fields.is_empty());
    assert_eq!(payload.as_ptr(), pointer);
    assert_eq!(payload.as_slice(), original.as_slice());
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 1);
    assert!(debit >= metrics.allocated);
    let ((), sharing) = measure(|| {
        let copy = payload.clone();
        assert!(payload.same_owner(&copy));
        assert_eq!(copy.as_ptr(), pointer);
        drop(copy);
    });
    assert_eq!(sharing.allocated, 0);
    assert_eq!(sharing.live, 0);
    assert!(!sharing.invalid);
    let ((), drop) = measure(|| drop(payload));
    assert_eq!(drop.allocated, 0);
    assert!(!drop.invalid);
}
#[test]
fn zero_one_short_exact_and_sticky_refusal_preserve_complete_input_before_allocation() {
    let mut probe = vec![field(ResolvedType::I64)];
    let mut exact = 0;
    let paid = NominalPayload::from_funded_vec(&mut probe, |n| {
        exact += n;
        Ok::<_, ()>(())
    })
    .unwrap();
    drop(paid);
    for cap in [0, exact - 1, exact] {
        let mut fields = vec![field(ResolvedType::I64)];
        let original = fields.clone();
        let pointer = fields.as_ptr();
        let ((result, retry, allocation), overflowed, used, first) =
            with_limit_usage_refusal(cap, || {
                let (result, allocation) =
                    measure(|| NominalPayload::from_funded_vec(&mut fields, reserve));
                let retry = if result.is_err() {
                    Some(reserve_active_required(0))
                } else {
                    None
                };
                (result, retry, allocation)
            });
        assert!(!allocation.invalid);
        if cap < exact {
            assert!(result.is_err());
            assert!(overflowed);
            assert_eq!(used, 0);
            assert_eq!(retry, Some(false));
            assert_eq!(allocation.allocated, 0);
            assert_eq!(fields, original);
            assert_eq!(fields.as_ptr(), pointer);
            let receipt = first.unwrap();
            assert_eq!(receipt.requested, exact);
            assert_eq!(receipt.remaining, cap);
        } else {
            assert!(result.is_ok());
            assert!(!overflowed);
            assert_eq!(used, exact);
            assert!(first.is_none());
            assert!(fields.is_empty());
        }
        drop(result);
    }
}
#[test]
fn shared_mutation_refuses_without_touching_either_full_owner() {
    let mut fields = vec![field(ResolvedType::I64)];
    let mut payload = NominalPayload::from_funded_vec(&mut fields, |_| Ok::<_, ()>(())).unwrap();
    let other = payload.clone();
    let pointer = payload.as_ptr();
    let ((result, metrics), overflowed, _, _) =
        with_limit_usage_refusal(0, || measure(|| payload.try_make_mut(reserve).map(|_| ())));
    assert!(result.is_err());
    assert!(overflowed);
    assert_eq!(metrics.allocated, 0);
    assert!(!metrics.invalid);
    assert!(payload.same_owner(&other));
    assert_eq!(payload.as_ptr(), pointer);
    payload.try_make_mut(|_| Ok::<_, ()>(())).unwrap()[0].ty = ResolvedType::Bool;
    assert_eq!(other[0].ty, ResolvedType::I64);
    assert_eq!(payload[0].ty, ResolvedType::Bool);
}
#[test]
fn nested_512_last_owner_drop_uses_only_its_preacquired_disposal_backing() {
    let mut ty = ResolvedType::I64;
    for _ in 0..512 {
        ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("nested"),
            arguments: vec![ty],
        };
    }
    // Keep construction and both drops in one real observer registry. A new
    // measure() resets its registry and cannot account frees of prior phases.
    let ((), total) = measure(|| {
        let mut fields = vec![field(ty)];
        let mut charged = 0;
        let before = crate::test_allocation_observer::current_metrics();
        let payload = NominalPayload::from_funded_vec(&mut fields, |n| {
            charged += n;
            Ok::<_, ()>(())
        })
        .unwrap();
        let construction = crate::test_allocation_observer::current_metrics();
        assert!(!construction.invalid);
        assert!(charged >= construction.allocated - before.allocated);
        let copy = payload.clone();
        let before_drop = crate::test_allocation_observer::current_metrics();
        drop(payload);
        let not_last = crate::test_allocation_observer::current_metrics();
        assert_eq!(not_last.allocated, before_drop.allocated);
        assert_eq!(not_last.freed, before_drop.freed);
        drop(copy);
        let last = crate::test_allocation_observer::current_metrics();
        assert_eq!(last.allocated, not_last.allocated);
        assert!(!last.invalid);
        assert!(last.freed > not_last.freed);
        assert_eq!(last.live, 0);
        assert_eq!(last.freed, last.allocated);
    });
    assert!(!total.invalid);
    assert_eq!(total.live, 0);
    assert_eq!(total.allocated, total.freed);
}
