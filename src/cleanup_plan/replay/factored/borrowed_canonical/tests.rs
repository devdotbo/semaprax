//! Original key-order/full multiset oracle plus cumulative actual backing.
use super::*;
use dictionary::Key;
#[test]
fn ordinary_order_and_full_duplicate_multisets_match_with_one_budget_through_drop() {
    let program = crate::hir::resolve(
        &crate::check(
            "module canonical.borrowed; @id(\"main\") fn main()->i64 {0}",
            "borrowed-canonical.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = &program.functions[0];
    let observation = |id: &str, value| {
        Label::Observation(Rc::new(SkeletonObservation::Boolean {
            expression: ExpressionId::from_owned(id.into()),
            value,
        }))
    };
    let labels = [
        observation("a", true),
        observation("a", false),
        observation("b", true),
        observation("é", true),
        Label::Observation(Rc::new(SkeletonObservation::VariantCase {
            scrutinee: ExpressionId::from_owned("a".into()),
            case: DeclarationId::new("some"),
            matches: true,
        })),
        Label::Observation(Rc::new(SkeletonObservation::Status {
            source: StatusSourceId {
                expression: ExpressionId::from_owned("a".into()),
                lane: StatusLane::OperationFailure,
            },
            success: true,
        })),
        Label::Observation(Rc::new(SkeletonObservation::StageCopyResult(
            StagedCopyResultSource::Body {
                expression: function.body.id.clone(),
                instance: ResolvedType::I64,
            },
        ))),
        Label::Observation(Rc::new(SkeletonObservation::CallCommit {
            call: function.body.id.clone(),
            arguments: Vec::new(),
        })),
        Label::Observation(Rc::new(SkeletonObservation::ArmSelected {
            scrutinee: function.body.id.clone(),
            arm: 0,
            selected: true,
        })),
        Label::Observation(Rc::new(SkeletonObservation::ArmSelected {
            scrutinee: function.body.id.clone(),
            arm: 1,
            selected: true,
        })),
        Label::Observation(Rc::new(SkeletonObservation::StageCopyResult(
            StagedCopyResultSource::Body {
                expression: function.body.id.clone(),
                instance: ResolvedType::Bool,
            },
        ))),
        observation("", true),
        Label::End(SkeletonTerminal::Success),
        Label::End(SkeletonTerminal::Failure),
    ];
    for a in &labels {
        for b in &labels {
            let mut budget = ReplayBudget::new();
            assert_eq!(a.compare(b, function, &mut budget).unwrap(), a.cmp(b));
        }
    }
    // Kind, observation kind, first differing byte; equal ID additionally
    // visits length and polarity. Independent literal short-circuit census.
    for (a, b, units) in [
        (observation("a", true), observation("b", false), 3),
        (observation("a", true), observation("a", false), 5),
    ] {
        for allowance in [units, units - 1] {
            let mut budget = ReplayBudget::new();
            budget.remaining = allowance;
            let result = a.compare(&b, function, &mut budget);
            assert_eq!(result.is_ok(), allowance == units);
            assert_eq!(budget.remaining, 0);
        }
    }
    // One DAG pointer with different continuations is two distinct identities.
    // The saved BTree route remains an independent numeric tuple oracle.
    let mut memo = dag_memo::Memo::Borrowed(dictionary::Dictionary::new());
    let mut old_memo = BTreeMap::new();
    let mut memo_budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    for (key, end) in [((1, 1), 10), ((1, 2), 20), ((2, 1), 30), ((1, 1), 11)] {
        old_memo.insert(key, end);
        dag_memo::insert(function, &mut memo, key, end, &mut memo_budget).unwrap();
    }
    for key in [(1, 1), (1, 2), (2, 1), (2, 2), (0, 1)] {
        assert_eq!(
            dag_memo::get(function, &memo, &key, &mut memo_budget).unwrap(),
            old_memo.get(&key).copied()
        );
    }

    // Two identical alternatives must remain two traces. A shared suffix
    // and a different terminal exercise weighted closure and label identity.
    let mut automaton = Automaton::default();
    let mut build = ReplayBudget::new();
    for _ in 0..7 {
        automaton.node(function, &mut build).unwrap();
    }
    automaton
        .movement(
            function,
            &mut build,
            1,
            Label::End(SkeletonTerminal::Success),
            0,
        )
        .unwrap();
    automaton
        .movement(
            function,
            &mut build,
            6,
            Label::End(SkeletonTerminal::Failure),
            0,
        )
        .unwrap();
    for root in [2, 3] {
        for _ in 0..2 {
            automaton
                .movement(function, &mut build, root, labels[0].clone(), 1)
                .unwrap();
        }
    }
    automaton
        .movement(function, &mut build, 4, labels[0].clone(), 1)
        .unwrap();
    automaton
        .movement(function, &mut build, 5, labels[0].clone(), 6)
        .unwrap();
    fn enumerate(automaton: &Automaton, node: usize, sink: usize) -> Vec<Vec<Label>> {
        if node == sink {
            return vec![Vec::new()];
        }
        let mut result = Vec::new();
        for child in &automaton.nodes[node].epsilon {
            result.extend(enumerate(automaton, *child, sink));
        }
        for (label, child) in &automaton.nodes[node].moves {
            for mut suffix in enumerate(automaton, *child, sink) {
                suffix.insert(0, label.clone());
                result.push(suffix);
            }
        }
        result.sort();
        result
    }
    let multisets: Vec<_> = (2..6).map(|root| enumerate(&automaton, root, 0)).collect();
    assert_eq!(multisets[0].len(), 2);
    assert_eq!(multisets[2].len(), 1);
    let mut old = Canonicalizer::new(&automaton, 0);
    let mut budget = ReplayBudget::new();
    let old_ids: Vec<_> = (2..6)
        .map(|root| {
            old.id(function, vec![(root, 1)], &mut budget)
                .unwrap()
                .unwrap()
        })
        .collect();
    for a in 0..4 {
        for b in 0..4 {
            assert_eq!(old_ids[a] == old_ids[b], multisets[a] == multisets[b]);
        }
    }
    let mut overflow_budget = ReplayBudget::new();
    let mut overflow =
        BorrowedCanonicalizer::new(function, &automaton, 0, &mut overflow_budget).unwrap();
    assert!(overflow
        .id(function, vec![(2, u64::MAX)], &mut overflow_budget)
        .unwrap()
        .is_none());
    assert!(overflow_budget.remaining < MAX_REPLAY_WORK_UNITS);

    fn run(
        function: &ResolvedFunction,
        automaton: &Automaton,
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), ()>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let mut budget = ReplayBudget::with_physical(function, bytes).map_err(|_| ())?;
            budget.remaining = work;
            for _ in 0..2 {
                let mut canonical = BorrowedCanonicalizer::new(function, automaton, 0, &mut budget)
                    .map_err(|_| ())?;
                let mut ids = [0; 4];
                for root in 2..6 {
                    budget
                        .reserve_array::<(usize, u64)>(function, 1, "test borrowed weighted root")
                        .map_err(|_| ())?;
                    ids[root - 2] = canonical
                        .id(function, vec![(root, 1)], &mut budget)
                        .map_err(|_| ())?
                        .ok_or(())?;
                }
                assert_eq!(ids[0], ids[1]);
                assert_ne!(ids[0], ids[2]);
                assert_ne!(ids[0], ids[3]);
                drop(canonical);
            }
            let used = (
                work - budget.remaining,
                budget.physical.as_ref().unwrap().charged(),
            );
            drop(budget);
            Ok(used)
        })
    }
    let (wide, metrics) = run(function, &automaton, MAX_REPLAY_WORK_UNITS, 1 << 24);
    let (work, bytes) = wide.unwrap();
    assert!(!metrics.invalid && metrics.allocated <= bytes && metrics.highwater <= bytes);
    assert_eq!(metrics.live, 0);
    let (exact, metrics) = run(function, &automaton, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    for (w, b) in [(work - 1, bytes), (work, bytes - 1)] {
        let (result, metrics) = run(function, &automaton, w, b);
        assert!(result.is_err());
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
    }
}
