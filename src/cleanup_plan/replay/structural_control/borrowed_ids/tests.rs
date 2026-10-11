//! Independent scratch-set equality and shared receiving resource witness.
use super::*;
#[test]
fn actual_identity_probes_match_ordinary_sets_and_share_exact_budget_through_drop() {
    let program = crate::hir::resolve(&crate::check(
        r#"module ids.borrowed; @id("main") fn main()->i64 requires true ensures result>=0 {1+1}"#,
        "borrowed-scratch-ids.spx",
    ).unwrap()).unwrap();
    let function = &program.functions[0];
    let a = ExpressionId::from_owned("a".into());
    let b = ExpressionId::from_owned("b".into());
    let mut first = function.cleanup_plan.status_sources[0].id.clone();
    first.expression = a.clone();
    let mut second = first.clone();
    second.expression = b.clone();
    let mut other_lane = first.clone();
    other_lane.lane = if first.lane == StatusLane::OperationFailure {
        StatusLane::ContractFalse
    } else {
        StatusLane::OperationFailure
    };
    // Exactly one table probe, then the literal lane/header/one-byte equality
    // work. This arithmetic is independent of the callback receipt.
    for (stored, sought, units) in [
        (Key::Expression(&a), Key::Expression(&b), 3),
        (Key::Status(&first), Key::Status(&second), 4),
        (Key::Status(&first), Key::Status(&other_lane), 2),
    ] {
        for allowance in [units, units - 1] {
            let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
            let mut ids = Ids::new(function, &mut budget).unwrap();
            assert!(ids.insert(function, stored, &mut budget).unwrap());
            budget.remaining = allowance;
            let found = ids.contains(function, sought, &mut budget);
            if allowance == units {
                assert!(!found.unwrap());
            } else {
                assert_eq!(found.unwrap_err().code, "SPX-H006");
            }
            assert_eq!(budget.remaining, 0);
        }
    }
    fn run(
        function: &ResolvedFunction,
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
                let mut ids = Ids::new(function, &mut budget).map_err(|_| ())?;
                for source in &function.cleanup_plan.status_sources {
                    assert!(ids
                        .insert(function, Key::Status(&source.id), &mut budget)
                        .map_err(|_| ())?);
                    assert!(!ids
                        .insert(function, Key::Status(&source.id), &mut budget)
                        .map_err(|_| ())?);
                }
                assert_eq!(ids.len(), function.cleanup_plan.status_sources.len());
                for source in &function.cleanup_plan.status_sources {
                    assert!(ids
                        .contains(function, Key::Status(&source.id), &mut budget)
                        .map_err(|_| ())?);
                }
                drop(ids);
            }
            let used = (
                work - budget.remaining,
                budget.physical.as_ref().unwrap().charged(),
            );
            drop(budget);
            Ok(used)
        })
    }
    let ordinary: BTreeSet<_> = function
        .cleanup_plan
        .status_sources
        .iter()
        .map(|source| &source.id)
        .collect();
    assert_eq!(ordinary.len(), function.cleanup_plan.status_sources.len());
    let (wide, allocation) = run(function, MAX_REPLAY_WORK_UNITS, 1 << 20);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
    for (limit_work, limit_bytes, success) in [
        (work, bytes, true),
        (work - 1, bytes, false),
        (work, bytes - 1, false),
    ] {
        let (actual, allocation) = run(function, limit_work, limit_bytes);
        assert_eq!(actual.is_ok(), success);
        assert!(!allocation.invalid && allocation.live == 0);
        if success {
            assert_eq!(actual.unwrap(), (work, bytes));
            assert!(allocation.allocated <= bytes);
        }
    }
}
