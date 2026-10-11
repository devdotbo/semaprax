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

fn canonical_probe(
    function: &ResolvedFunction,
    automaton: &Automaton,
    sink: usize,
    root: usize,
    limit: usize,
) -> (
    Result<usize, Diagnostic>,
    crate::test_allocation_observer::Metrics,
) {
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let mut canonical = Canonicalizer::new(automaton, sink);
        budget.reserve_array::<(usize, u64)>(function, 1, "test canonical root")?;
        let first = canonical
            .id(function, vec![(root, 1)], &mut budget)?
            .unwrap();
        budget.reserve_array::<(usize, u64)>(function, 1, "test repeated canonical root")?;
        let second = canonical
            .id(function, vec![(root, 1)], &mut budget)?
            .unwrap();
        assert_eq!(first, second);
        drop(canonical);
        Ok(budget.physical.as_ref().unwrap().charged() - before)
    })
}

#[test]
fn weighted_canonical_backing_and_overlap_fit_exact_and_one_short_limits() {
    let (_, function) =
        super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::GENERATED);
    let mut builder = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    builder.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut automaton = Automaton::default();
    let sink = automaton.node(&function, &mut builder).unwrap();
    let success = automaton
        .end(&function, &mut builder, sink, SkeletonTerminal::Success)
        .unwrap();
    let failure = automaton
        .end(&function, &mut builder, sink, SkeletonTerminal::Failure)
        .unwrap();
    let latch = automaton
        .end(&function, &mut builder, sink, SkeletonTerminal::LoopLatch)
        .unwrap();
    let root = automaton
        .add_plan(&function, success, failure, latch, &mut builder)
        .unwrap();
    let (result, observed) = canonical_probe(&function, &automaton, sink, root, 1 << 24);
    let charged = result.unwrap();
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    let baseline = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    let (exact, observed) = canonical_probe(&function, &automaton, sink, root, admission + charged);
    assert_eq!(exact.unwrap(), charged);
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    let (short, _) = canonical_probe(&function, &automaton, sink, root, admission + charged - 1);
    assert!(short.is_err(), "one-short canonical storage must refuse");
}

#[test]
fn duplicate_routes_change_identity_and_weight_overflow_is_no_proof() {
    let (_, function) =
        super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE);
    let mut budget = ReplayBudget::new();
    let mut automaton = Automaton::default();
    let sink = automaton.node(&function, &mut budget).unwrap();
    let success = automaton
        .end(&function, &mut budget, sink, SkeletonTerminal::Success)
        .unwrap();
    let duplicate = automaton.node(&function, &mut budget).unwrap();
    automaton
        .epsilon(&function, &mut budget, duplicate, success)
        .unwrap();
    automaton
        .epsilon(&function, &mut budget, duplicate, success)
        .unwrap();
    let mut canonical = Canonicalizer::new(&automaton, sink);
    let once = canonical
        .id(&function, vec![(success, 1)], &mut budget)
        .unwrap();
    let twice = canonical
        .id(&function, vec![(duplicate, 1)], &mut budget)
        .unwrap();
    assert_ne!(
        once, twice,
        "duplicate epsilon routes retain exact multiplicity"
    );
    assert_eq!(
        canonical
            .id(&function, vec![(duplicate, u64::MAX)], &mut budget)
            .unwrap(),
        None,
        "checked multiplicity overflow cannot authenticate equality"
    );
}
