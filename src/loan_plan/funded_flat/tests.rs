use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

#[test]
fn funded_flat_receiver_keeps_complete_saved_plan_and_current_member_boundary() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let old = saved_planner::build_plan(&program, function).unwrap();
    let (result, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        build(&program, function, &owner)
    });
    assert!(!refused);
    let actual = match result {
        Ok(plan) => plan,
        Err(_) => panic!("intended funded flat planner positive refused"),
    };
    assert_eq!(actual, old);
    assert!(!actual.loans.is_empty());
    let foreign = function.clone();
    let (result, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        build(&program, &foreign, &owner)
    });
    assert!(!refused);
    match result {
        Err(PlannerError::Semantic(error)) =>
            assert_eq!(error.message, "funded loan planner requires the exact current function"),
        _ => panic!("foreign member was not rejected before planner storage"),
    }
}

#[test]
fn empty_semantic_diagnostic_is_not_a_physical_tag_or_paid_report() {
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        match classify::<()>(Err(storage::marker()), &owner) {
            Err(PlannerError::Semantic(error)) => {
                assert_eq!(error.code, "SPX-H006");
                assert!(error.message.is_empty());
            }
            _ => panic!("semantic empty diagnostic acquired physical meaning"),
        }
        assert_eq!(owner.first(), None);
    });
    assert!(!refused);
    let setup = std::mem::size_of::<CfgReservation>() + 46;
    // The independent literal is checked against the dedicated message length
    // by the owning report module; this is not an identity-message allowance.
    let ((), refused, _) = with_limit_usage(setup, || {
        let owner = CfgReservation::acquire().unwrap();
        let first = owner.reserve(1).unwrap_err();
        match classify::<()>(Err(storage::marker()), &owner) {
            Err(PlannerError::Physical(reason)) => assert_eq!(reason, first),
            _ => panic!("actual typed physical failure was rendered semantically"),
        }
        assert_eq!(owner.reserve(0), Err(first));
        drop(owner.diagnostic());
    });
    assert!(refused);
}

#[test]
fn actual_flat_adjacency_and_fixed_queue_arrays_have_exact_short_drop_payment() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let cfg = build_cfg(function, &mut WorkCounter::new(MAX_LOAN_PLAN_WORK_V1)).unwrap();
    let starts = [0_u16, 1, 2];
    let mut legacy_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    let legacy = work::Reachability::build(&cfg, starts.into_iter(), &mut legacy_work).unwrap();
    // Exact actual allocations: two offsets, one predecessor entry array,
    // one mask word per point, two u16 links and one membership byte.
    let arrays = 2 * (cfg.points.len() + 1) * 2 + cfg.edges.len() * 2
        + cfg.points.len() * (8 + 2 + 2 + 1);
    let setup = std::mem::size_of::<CfgReservation>() + 46;
    let run = |limit| with_limit_usage_refusal(limit, || measure(|| {
        let owner = CfgReservation::acquire().unwrap();
        let result = (|| {
            let adjacency = adjacency::Adjacency::build_with_storage(
                cfg.points.len(), &cfg.edges, Some(&owner))?;
            let mut counted = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            counted.allocation = Some(&owner);
            let reachable = work::Reachability::build(&cfg, starts.into_iter(), &mut counted)?;
            assert_eq!(counted.used, legacy_work.used);
            for loan in 0..3 {
                for node in 0..cfg.points.len() {
                    assert_eq!(reachable.contains(LoanId(loan), node as u16),
                        legacy.contains(LoanId(loan), node as u16));
                }
            }
            drop(reachable);
            drop(adjacency);
            Ok::<_, Diagnostic>(())
        })();
        match classify(result, &owner) {
            Ok(()) => { drop(owner); true }
            Err(PlannerError::Physical(reason)) => {
                assert_eq!(owner.reserve(0), Err(reason));
                assert_eq!(owner.reserve(1), Err(reason));
                drop(owner.diagnostic());
                false
            }
            Err(PlannerError::Semantic(_)) => panic!("unexpected semantic failure"),
        }
    }));
    let ((success, observed), refused, used, first) = run(setup + arrays);
    assert!(success);
    assert!(!refused);
    assert_eq!(used, setup + arrays);
    assert_eq!(first, None);
    assert_eq!(observed.allocations, 8); // text and seven actual arrays
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocated, 46 + arrays);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((success, observed), refused, _, first) = run(setup + arrays - 1);
    assert!(!success);
    assert!(refused);
    assert_eq!(first.unwrap().requested, cfg.points.len()); // last bool array
    assert_eq!(observed.allocations, 7);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}
