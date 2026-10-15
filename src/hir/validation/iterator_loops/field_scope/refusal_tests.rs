//! Current frontier construction through Drop, independent of semantic fixtures.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::borrow::Cow;
#[test]
fn literal_frontier_zero_exact_short_and_sticky_refusal_settle_message_and_entries() {
    let execution = crate::hir::FunctionExecutionId::Monomorphic(crate::hir::DeclarationId::new(
        "frontier.literal",
    ));
    let expression = ResolvedExpr {
        id: crate::hir::ExpressionId::new(&execution, "frontier.literal"),
        ty: ResolvedType::I64,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Int(0),
        span: crate::ast::Span::default(),
    };
    let (((), observed), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let mut pending = Pending::new(&expression, true);
            pending.push(&expression);
            assert!(pending.pop().is_none());
            let error = pending.finish().unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert_eq!(
                error.message,
                "owned iterator field-scope carrier exceeds its bound"
            );
            assert!(matches!(error.message, Cow::Borrowed(_)));
            assert_eq!(error.message_owned_capacity(), 0);
            assert!(error.path.is_none() && error.span.is_none() && error.help.is_none());
            drop(error.clone());
            drop(error);
            assert!(!crate::bounded_output::reserve_active_required(0));
        })
    });
    assert!(refused);
    assert_eq!(first.unwrap().requested, std::mem::size_of::<Pending<'_>>());
    assert_eq!(paid, 0);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.freed, 0);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut pending = Pending::new(&expression, true);
                pending.push(&expression);
                pending.push(&expression);
                let mut count = 0;
                while let Some(current) = pending.pop() {
                    assert!(std::ptr::eq(current, &expression));
                    count += 1;
                }
                match pending.finish() {
                    Ok(()) => {
                        assert_eq!(count, 3);
                        true
                    }
                    Err(error) => {
                        assert_eq!(count, 0);
                        assert_eq!(
                            error.message,
                            "owned iterator field-scope frontier exceeds its bound"
                        );
                        assert!(matches!(error.message, Cow::Borrowed(_)));
                        assert_eq!(error.message_owned_capacity(), 0);
                        drop(error.clone());
                        drop(error);
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        false
                    }
                }
            })
        })
    };
    let ((success, whole), refused, exact, first) = run(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(whole.allocated <= exact && whole.highwater <= exact);
    assert_eq!(whole.freed, whole.allocated);
    assert_eq!(whole.live, 0);
    assert!(!whole.invalid);
    let ((success, repeated), refused, paid, first) = run(exact);
    assert!(success && !refused && first.is_none());
    assert_eq!(paid, exact);
    assert_eq!(repeated, whole);
    let ((success, short), refused, paid, first) = run(exact - 1);
    assert!(!success && refused && first.is_some());
    assert!(short.allocated <= paid && short.highwater <= paid);
    assert_eq!(short.freed, short.allocated);
    assert_eq!(short.live, 0);
    assert!(!short.invalid);
}
