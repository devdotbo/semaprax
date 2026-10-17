use super::*;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module retained.lookup; @id(\"lookup.main\") fn main()->i64 {0}",
            "retained-lookup.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn arena<'hir>(function: &'hir ResolvedFunction, keys: &[ExpressionId]) -> Arena<'hir> {
    let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
    let mut capacity =
        capacity::FactCapacity::with_payer(function, capacity::Payer::Selected(&mut physical))
            .unwrap();
    let mut arena = Arena::new(&mut capacity).unwrap();
    let mut budget = ReplayBudget::unobserved();
    for key in keys {
        arena.prepare_growth_work(function, &mut budget).unwrap();
        arena.reserve_next(&mut capacity).unwrap();
        arena
            .prepare_insert_work(function, key, &mut budget)
            .unwrap();
        assert!(arena.insert(key.clone(), None, &mut capacity).unwrap());
    }
    assert!(arena.balanced());
    arena
}
fn acquire<'a, 'hir, 'b>(
    arena: &'a Arena<'hir>,
    function: &'a ResolvedFunction,
    budget: &'b mut ReplayBudget,
) -> Result<Session<'a, 'hir, 'b>, Diagnostic> {
    budget.reserve_bytes(
        function,
        Session::OLD_SCRATCH + Session::acquisition_bytes(),
        "retained expression-fact owner acquisition",
    )?;
    budget.charge(
        function,
        12,
        "retained expression-fact owner initialization",
    )?;
    Ok(Session::new_prepaid(arena, function, budget))
}

#[test]
fn alternating_queries_preserve_complete_borrowed_result_and_old_work_on_all_shapes() {
    let program = program();
    let function = &program.functions[0];
    let mut keys = vec!["", "\0", "a", "a\0", "aa", "λ", "識別", "🙂"]
        .into_iter()
        .map(|s| ExpressionId::from_owned(s.into()))
        .collect::<Vec<_>>();
    for n in 0..67 {
        keys.push(ExpressionId::from_owned(format!("shared.λ\0.{n:03}")));
    }
    for count in [0, 1, 2, 15, 16, keys.len()] {
        for reverse in [false, true] {
            let mut order = keys[..count].to_vec();
            if reverse {
                order.reverse();
            }
            let arena = arena(function, &order);
            let queries = keys
                .iter()
                .chain(keys.iter().rev())
                .map(|key| ExpressionId::from_owned(key.as_str().to_owned()))
                .collect::<Vec<_>>();
            let mut budget = ReplayBudget::unobserved();
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            for query in &queries {
                let mut old_budget = ReplayBudget::unobserved();
                let before = session.budget.remaining;
                let old = prefix::retained_before::find_receiving(
                    &arena,
                    function,
                    query,
                    &mut old_budget,
                )
                .unwrap();
                let current = session.fact(query).unwrap();
                assert_eq!(current.is_some(), old.is_some());
                if let (Some(current), Some(old)) = (current, old) {
                    assert!(
                        std::ptr::eq(current, old),
                        "complete payload and present noncall are unchanged"
                    );
                }
                assert_eq!(
                    before - session.budget.remaining,
                    MAX_REPLAY_WORK_UNITS - old_budget.remaining
                );
            }
        }
    }
}

#[test]
fn every_old_query_work_cut_keeps_all_six_diagnostic_fields() {
    let program = program();
    let function = &program.functions[0];
    let keys = (0..33)
        .map(|n| ExpressionId::from_owned(format!("prefix.{n:03}")))
        .collect::<Vec<_>>();
    let arena = arena(function, &keys);
    for query in [&keys[0], &keys[17], &keys[32]] {
        let mut complete = ReplayBudget::unobserved();
        prefix::retained_before::find_receiving(&arena, function, query, &mut complete).unwrap();
        for work in 0..=MAX_REPLAY_WORK_UNITS - complete.remaining {
            let mut old_budget = ReplayBudget::unobserved();
            old_budget.remaining = work;
            let old =
                prefix::retained_before::find_receiving(&arena, function, query, &mut old_budget);
            let mut budget = ReplayBudget::unobserved();
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            session.budget.remaining = work;
            let current = session.fact(query);
            assert_eq!(
                current.as_ref().err(),
                old.as_ref().err(),
                "all six fields at work {work}"
            );
            assert_eq!(current.is_ok(), old.is_ok());
            assert_eq!(session.budget.remaining, old_budget.remaining);
        }
    }
}

#[test]
fn exact_short_first_and_later_query_payments_are_sticky_without_allocation_or_refund() {
    let program = program();
    let function = &program.functions[0];
    let arena = arena(function, &[function.body.id.clone()]);
    let observe = |bytes, queries| {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        let result = (|| {
            let mut session = acquire(&arena, function, &mut budget)?;
            for _ in 0..queries {
                session.fact(&function.body.id)?;
            }
            Ok::<_, Diagnostic>(())
        })();
        let paid = budget.physical.as_ref().unwrap().charged();
        if result.is_err() {
            let remaining = budget.remaining;
            assert!(budget.reserve_bytes(function, 0, "sticky retry").is_err());
            assert_eq!(remaining, budget.remaining);
            assert_eq!(paid, budget.physical.as_ref().unwrap().charged());
        }
        (result.is_ok(), paid)
    };
    for count in [0, 1, 2, 7] {
        let (ok, paid) = observe(usize::MAX, count);
        assert!(ok);
        assert_eq!(observe(paid, count), (true, paid));
        assert!(!observe(paid - 1, count).0);
    }
    let mut budget = ReplayBudget::unobserved();
    let (_, metrics) = measure(|| {
        let mut session = acquire(&arena, function, &mut budget).unwrap();
        for _ in 0..10 {
            session.fact(&function.body.id).unwrap();
        }
    });
    assert!(!metrics.invalid);
    assert_eq!(
        (metrics.allocated, metrics.reallocations, metrics.live),
        (0, 0, 0)
    );
}

#[test]
fn unwind_releases_the_exclusive_budget_loan_without_any_scratch_backing() {
    let program = program();
    let function = &program.functions[0];
    let arena = arena(function, &[function.body.id.clone()]);
    let mut budget = ReplayBudget::unobserved();
    let (_, metrics) = measure(|| {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            session.fact(&function.body.id).unwrap();
            std::panic::resume_unwind(Box::new(()));
        }));
        assert!(result.is_err());
        drop(result);
        budget
            .reserve_bytes(function, 0, "after unwound owner")
            .unwrap();
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

#[test]
fn zero_budget_and_reserved_floor_refuse_without_later_retry_authority() {
    let program = program();
    let function = &program.functions[0];
    let arena = arena(function, &[function.body.id.clone()]);
    assert_eq!(
        ReplayBudget::with_physical(function, 0).err().unwrap().code,
        "SPX-H006"
    );
    crate::bounded_output::with_limit_usage(1 << 20, || {
        let mut budget = ReplayBudget::new();
        budget.prepare_builder(function).unwrap();
        let remaining = crate::bounded_output::active_remaining().unwrap();
        assert!(crate::bounded_output::set_active_floor(remaining));
        let error = acquire(&arena, function, &mut budget)
            .err()
            .expect("reserved floor refuses owner acquisition");
        assert_eq!(error.code, "SPX-H006");
        crate::bounded_output::clear_active_floor();
        assert!(budget
            .reserve_bytes(function, 0, "retry after floor restoration")
            .is_err());
    });
}
