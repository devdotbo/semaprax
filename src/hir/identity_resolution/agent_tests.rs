use super::*;
use crate::bounded_output::identity_reservation::IdentityReservation;
use crate::bounded_output::{
    active_remaining, reserve_active_required, with_limit_usage, with_limit_usage_refusal,
};
use crate::hir::resolve_program::agent_identity::fixture::{
    agent_program, agent_texts, agents, backing, census, module_name, saved_original, BIG,
};
use crate::test_allocation_observer::measure;

#[derive(Debug)]
struct Stop;

#[test]
fn agent_identities_are_the_final_exact_borrowed_requests_and_keep_full_hir() {
    let program = agent_program();
    let texts: Vec<&str> = agents(&program).iter().flat_map(agent_texts).collect();
    assert!(!texts.is_empty());
    let c = census(&program);
    let expected: Vec<usize> = texts.iter().map(|text| backing(text.len())).collect();
    assert_eq!(&c.requests[c.agent_start..], expected.as_slice());
    let ordinary =
        crate::hir::resolve_program::agent_identity::fixture::original_resolve(&program).unwrap();
    let paid = resolve(&program, None, |_| Ok::<(), Stop>(())).unwrap();
    assert_eq!(paid.0, ordinary.0);
    let saved: Vec<_> = agents(&program)
        .iter()
        .map(|declaration| saved_original(declaration, module_name(&program)))
        .collect();
    assert_eq!(paid.0.agents, saved);
    assert_eq!(paid.1.reused(), ordinary.1.reused());
}

/// Run on a ledger of `limit`; the identity callback is the real owner.
fn refuse_at(
    program: &Program,
    limit: usize,
    seen_expected: usize,
    requested: usize,
    remaining: usize,
    used_expected: usize,
) {
    let expected_report = crate::hir::resolve_program::agent_identity::fixture::refusal_report();
    let (((), observed), failed, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut owner = IdentityReservation::acquire().unwrap();
            let mut seen = 0;
            let failure = resolve(program, None, |bytes| {
                seen += 1;
                owner.reserve(bytes)
            })
            .err()
            .unwrap();
            assert!(matches!(failure, Failure::Identity(_)));
            assert_eq!(seen, seen_expected);
            assert_eq!(active_remaining(), Some(remaining));
            // Zero retry stays refused: the first receipt is sticky.
            assert!(!reserve_active_required(0));
            assert!(!reserve_active_required(1));
            assert!(owner.reserve(0).is_err());
            let report = owner.publish();
            assert_eq!(report.len(), 1);
            assert_eq!(report[0].code, "SPX-H006");
            assert_eq!(report, expected_report);
            drop(report);
        })
    });
    assert!(failed);
    assert_eq!(used, used_expected);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining), (requested, remaining));
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[test]
fn every_agent_request_refuses_one_short_with_exact_first_receipt_and_drop() {
    let program = agent_program();
    let c = census(&program);
    for k in c.agent_start..c.requests.len() {
        let used_before = BIG - c.before[k];
        refuse_at(
            &program,
            used_before + c.requests[k] - 1,
            k + 1,
            c.requests[k],
            c.requests[k] - 1,
            used_before,
        );
    }
}

#[test]
fn exactly_funded_agent_request_succeeds_and_the_next_one_refuses_with_zero_remaining() {
    let program = agent_program();
    let c = census(&program);
    for k in c.agent_start..c.requests.len().saturating_sub(1) {
        let used_before = BIG - c.before[k];
        refuse_at(
            &program,
            used_before + c.requests[k],
            k + 2,
            c.requests[k + 1],
            0,
            used_before + c.requests[k],
        );
    }
}

#[test]
fn exact_total_ledger_resolves_every_agent_identity() {
    let program = agent_program();
    let c = census(&program);
    let (ok, failed, used) = with_limit_usage(c.used, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        resolve(&program, None, |bytes| owner.reserve(bytes)).is_ok()
    });
    assert!(ok && !failed);
    assert_eq!(used, c.used);
}

#[test]
fn both_invariant_passes_pay_late_agent_identities_and_keep_complete_original_hir() {
    let program = crate::hir::resolve_program::agent_identity::fixture::invariant_agent_program();
    let requests: Vec<_> = agents(&program)
        .iter()
        .flat_map(agent_texts)
        .map(|text| backing(text.len()))
        .collect();
    let c = census(&program);
    assert_eq!(
        c.requests
            .windows(requests.len())
            .filter(|window| *window == requests.as_slice())
            .count(),
        2
    );
    let original =
        crate::hir::resolve_program::agent_identity::fixture::original_resolve(&program).unwrap();
    let (((), observed), failed, used) = with_limit_usage(c.used, || {
        measure(|| {
            let mut owner = IdentityReservation::acquire().unwrap();
            let current = resolve(&program, None, |bytes| owner.reserve(bytes)).unwrap();
            assert_eq!(current.0, original.0);
            assert_eq!(current.1.reused(), original.1.reused());
            drop(current);
        })
    });
    assert!(!failed);
    assert_eq!(used, c.used);
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
