//! Physical oracle for automaton construction only, including its destruction.
//! Canonicalizer storage remains outside this inactive construction owner.
use super::*;

fn probe(
    function: &ResolvedFunction,
    limit: usize,
) -> (
    Result<usize, Diagnostic>,
    crate::test_allocation_observer::Metrics,
) {
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let before = budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let mut automaton = Automaton::default();
        let sink = automaton.node(function, &mut budget)?;
        let success = automaton.end(function, &mut budget, sink, SkeletonTerminal::Success)?;
        let failure = automaton.end(function, &mut budget, sink, SkeletonTerminal::Failure)?;
        let latch = automaton.end(function, &mut budget, sink, SkeletonTerminal::LoopLatch)?;
        automaton.add_plan(function, success, failure, latch, &mut budget)?;
        drop(automaton);
        Ok(budget.physical.as_ref().unwrap().charged() - before)
    })
}

#[test]
fn lexical_automaton_backing_is_precharged_at_exact_and_one_short_limits() {
    let (_, function) =
        super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::GENERATED);
    let (result, observed) = probe(&function, 1 << 24);
    let charged = result.unwrap();
    assert!(!observed.invalid);
    assert!(observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    let baseline = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    let (exact, observed) = probe(&function, admission + charged);
    assert_eq!(exact.unwrap(), charged);
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    let (short, _) = probe(&function, admission + charged - 1);
    assert!(
        short.is_err(),
        "one-short construction must refuse before final allocation"
    );
}

#[test]
fn a_failed_attempt_keeps_spent_work_and_physical_refusal_sticky() {
    let (_, function) =
        super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE);
    let mut budget = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    budget.remaining = 3;
    let before = budget.physical.as_ref().unwrap().charged();
    let mut automaton = Automaton::default();
    assert!(automaton.node(&function, &mut budget).is_ok());
    assert!(automaton.node(&function, &mut budget).is_ok());
    assert!(automaton.node(&function, &mut budget).is_ok());
    assert!(automaton.node(&function, &mut budget).is_err());
    assert_eq!(budget.remaining, 0);
    assert!(budget.physical.as_ref().unwrap().charged() > before);
    assert!(budget.charge(&function, 1, "attempted fallback").is_err());
}
