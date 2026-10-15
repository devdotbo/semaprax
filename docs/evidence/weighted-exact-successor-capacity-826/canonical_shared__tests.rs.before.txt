//! Independent full-old algebra and actual allocation/drop controls.
use super::*;

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

fn automaton(function: &ResolvedFunction) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::new();
    let mut automaton = Automaton::default();
    let sink = automaton.node(function, &mut budget).unwrap();
    let mut roots = Vec::new();
    for terminal in [
        SkeletonTerminal::Success,
        SkeletonTerminal::Failure,
        SkeletonTerminal::LoopLatch,
    ] {
        roots.push(
            automaton
                .end(function, &mut budget, sink, terminal)
                .unwrap(),
        );
    }
    let duplicate = automaton.node(function, &mut budget).unwrap();
    for _ in 0..2 {
        automaton
            .epsilon(function, &mut budget, duplicate, roots[0])
            .unwrap();
    }
    roots.push(duplicate);
    for targets in [[roots[0], roots[1]], [roots[1], roots[0]]] {
        let root = automaton.node(function, &mut budget).unwrap();
        for target in targets {
            automaton
                .epsilon(function, &mut budget, root, target)
                .unwrap();
        }
        roots.push(root);
    }
    let cycle = automaton.node(function, &mut budget).unwrap();
    automaton
        .movement(
            function,
            &mut budget,
            cycle,
            Label::End(SkeletonTerminal::Success),
            cycle,
        )
        .unwrap();
    roots.push(cycle);
    (automaton, sink, roots)
}

#[test]
fn full_old_receiver_preserves_ids_signatures_work_duplicates_overflow_and_cycle() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let mut old = canonical_before::Receiver::new(&automaton, sink);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut old_budget = ReplayBudget::new();
    let mut budget = ReplayBudget::new();
    let mut ids = Vec::new();
    for root in &roots[..6] {
        let expected = old
            .id(&function, vec![(*root, 1)], &mut old_budget)
            .unwrap();
        let actual = current
            .id(&function, vec![(*root, 1)], &mut budget)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(budget.remaining, old_budget.remaining);
        assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
        assert_eq!(&current.ids, old.signatures());
        assert_eq!(current.states.len(), old.states().len());
        for (state, id) in old.states() {
            assert_eq!(current.states.get(&Rc::new(state.clone())), Some(id));
        }
        ids.push(actual);
    }
    assert_ne!(ids[0], ids[3]);
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_eq!(ids[4], ids[5]);
    let expected = old
        .id(&function, vec![(roots[3], u64::MAX)], &mut old_budget)
        .unwrap();
    assert_eq!(
        current
            .id(&function, vec![(roots[3], u64::MAX)], &mut budget)
            .unwrap(),
        expected
    );
    assert_eq!(expected, None);
    assert_eq!(budget.remaining, old_budget.remaining);
    let expected = old
        .id(&function, vec![(roots[6], 1)], &mut old_budget)
        .unwrap_err();
    let actual = current
        .id(&function, vec![(roots[6], 1)], &mut budget)
        .unwrap_err();
    assert_eq!(actual, expected);
    assert_eq!(budget.remaining, old_budget.remaining);
}

fn probe(
    function: &ResolvedFunction,
    automaton: &Automaton,
    sink: usize,
    roots: &[usize],
    limit: usize,
) -> (Result<usize, ()>, crate::test_allocation_observer::Metrics) {
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let mut current = Canonicalizer::new(automaton, sink);
        let result = (|| {
            for root in roots {
                budget.reserve_array::<(usize, u64)>(function, 1, "shared test root")?;
                current.id(function, vec![(*root, 1)], &mut budget)?;
            }
            Ok::<_, Diagnostic>(budget.physical.as_ref().unwrap().charged() - before)
        })();
        drop(current);
        // Drop refusal storage inside observation as well as every memo/frame.
        result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        })
    })
}

#[test]
fn shared_full_backing_exact_short_refusal_and_error_drop_are_physical() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let sequence = [roots[4], roots[5], roots[3], roots[4]];
    let (result, observed) = probe(&function, &automaton, sink, &sequence, 1 << 24);
    let charged = result.unwrap();
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    let baseline = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    let (exact, observed) = probe(&function, &automaton, sink, &sequence, admission + charged);
    assert_eq!(exact.unwrap(), charged);
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    for limit in [admission, admission + charged - 1] {
        let (result, observed) = probe(&function, &automaton, sink, &sequence, limit);
        assert!(result.is_err());
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    let (cycle, observed) = probe(&function, &automaton, sink, &roots[6..], 1 << 24);
    assert!(cycle.is_err());
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[test]
fn immutable_shares_preserve_copy_work_and_zero_requests_cannot_clear_refusal() {
    let function = fixture();
    let mut budget = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    budget
        .reserve_array::<(usize, u64)>(&function, 2, "share test backing")
        .unwrap();
    let state = own_weighted(&function, &mut budget, vec![(0, 1), (1, 2)]).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    let work = budget.remaining;
    let shared = share_weighted(&function, &mut budget, &state).unwrap();
    assert!(Rc::ptr_eq(&state, &shared));
    assert_eq!(budget.remaining, work - state.len() - 1);
    assert_eq!(budget.physical.as_ref().unwrap().charged(), before);
    budget.physical.as_mut().unwrap().reject();
    assert!(share_weighted(&function, &mut budget, &state).is_err());
    assert!(budget.reserve_bytes(&function, 0, "refused zero").is_err());
    assert_eq!(budget.physical.as_ref().unwrap().charged(), before);
}

#[test]
fn exact_short_and_zero_work_match_the_complete_old_receiver() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let mut full = ReplayBudget::new();
    Canonicalizer::new(&automaton, sink)
        .id(&function, vec![(roots[4], 1)], &mut full)
        .unwrap();
    let spent = MAX_REPLAY_WORK_UNITS - full.remaining;
    for limit in [spent, spent - 1, 0] {
        let mut old_budget = ReplayBudget::new();
        old_budget.remaining = limit;
        let mut budget = ReplayBudget::new();
        budget.remaining = limit;
        let old = canonical_before::Receiver::new(&automaton, sink).id(
            &function,
            vec![(roots[4], 1)],
            &mut old_budget,
        );
        let current =
            Canonicalizer::new(&automaton, sink).id(&function, vec![(roots[4], 1)], &mut budget);
        assert_eq!(current, old);
        assert_eq!(budget.remaining, old_budget.remaining);
        if limit == spent {
            assert!(current.unwrap().is_some());
            assert_eq!(budget.remaining, 0);
        } else {
            assert!(current.is_err());
            assert!(budget.charge(&function, 0, "sticky work refusal").is_err());
        }
    }
}

#[test]
fn overflow_returns_no_proof_and_drops_all_paid_backing() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let mut budget = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    let (charged, observed) = crate::test_allocation_observer::measure(|| {
        let mut current = Canonicalizer::new(&automaton, sink);
        budget
            .reserve_array::<(usize, u64)>(&function, 1, "overflow test root")
            .unwrap();
        assert_eq!(
            current
                .id(&function, vec![(roots[3], u64::MAX)], &mut budget)
                .unwrap(),
            None
        );
        drop(current);
        budget.physical.as_ref().unwrap().charged() - before
    });
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
