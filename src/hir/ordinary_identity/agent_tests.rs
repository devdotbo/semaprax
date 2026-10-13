use super::*;
use crate::bounded_output::{
    active_remaining, reserve_active_required, with_limit_usage, with_limit_usage_refusal,
};
use crate::hir::resolve_program::agent_identity::fixture::{agent_program, census, BIG};
use crate::test_allocation_observer::measure;

// Saved ordinary public analysis receiver, without the payer dispatch.
fn saved_old(program: &Program) -> Analysis {
    let diagnostics = source_verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Analysis {
            diagnostics,
            resolved: None,
        };
    }
    match crate::hir::resolve_program::agent_identity::fixture::original_resolve(program) {
        Ok((resolved, _)) => Analysis {
            diagnostics,
            resolved: Some(resolved),
        },
        Err(diagnostic) => Analysis {
            diagnostics: vec![diagnostic],
            resolved: None,
        },
    }
}

#[test]
fn analyzed_agent_identities_keep_hir_and_pay_exact_backing_beside_setup() {
    let program = agent_program();
    let expected = saved_old(&program);
    assert!(expected.resolved.is_some());
    let c = census(&program);
    let (legacy, failed, old_used) = with_limit_usage(BIG, || saved_old(&program));
    assert!(!failed);
    let (paid, failed, new_used) = with_limit_usage(BIG, || crate::hir::analyze(&program));
    assert!(!failed);
    assert_eq!(legacy.diagnostics, expected.diagnostics);
    assert_eq!(paid.diagnostics, expected.diagnostics);
    assert_eq!(paid.resolved, expected.resolved);
    let requested: usize = c.requests.iter().sum();
    assert_eq!(
        new_used,
        old_used + IdentityReservation::setup_bytes().unwrap() + requested
    );
}

#[test]
fn public_analysis_refuses_each_agent_request_one_short_with_paid_report_and_sticky_receipt() {
    let program = agent_program();
    let c = census(&program);
    for k in c.agent_start..c.requests.len() {
        let used_before = BIG - c.before[k];
        let limit = used_before + c.requests[k] - 1;
        let expected_report =
            crate::hir::resolve_program::agent_identity::fixture::refusal_report();
        let (((), observed), failed, used, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = crate::hir::analyze(&program);
                assert!(result.resolved.is_none());
                assert_eq!(result.diagnostics.len(), 1);
                assert_eq!(result.diagnostics[0].code, "SPX-H006");
                assert_eq!(result.diagnostics, expected_report);
                assert_eq!(
                    result.diagnostics[0].message,
                    "declaration identity backing reservation refused"
                );
                assert_eq!(active_remaining(), Some(c.requests[k] - 1));
                assert!(!reserve_active_required(0));
                assert!(!reserve_active_required(1));
                drop(result);
            })
        });
        assert!(failed);
        assert_eq!(used, used_before);
        let first = first.unwrap();
        assert_eq!(first.requested, c.requests[k]);
        assert_eq!(first.remaining, c.requests[k] - 1);
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn strict_constructor_moves_the_prepaid_report_after_a_late_agent_refusal() {
    let program = agent_program();
    let c = census(&program);
    let k = c.requests.len() - 1;
    let limit = BIG - c.before[k] + c.requests[k] - 1;
    let expected_report = crate::hir::resolve_program::agent_identity::fixture::refusal_report();
    let (((), observed), failed, _, _) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut pointers = None;
            let report =
                try_resolve_verified_observed(&program, |owner| pointers = Some(owner.pointers()))
                    .unwrap()
                    .err()
                    .unwrap();
            assert_eq!(report, expected_report);
            let pointers = pointers.unwrap();
            assert_eq!(report.as_ptr(), pointers.0);
            assert_eq!(report[0].message.as_ptr(), pointers.1);
            drop(report);
        })
    });
    assert!(failed);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
