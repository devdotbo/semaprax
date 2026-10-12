//! Full old liveness/edge receivers and late semantic errors, not projected
//! success summaries. Allocation/Drop controls remain whole-constructor owners.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::with_limit_usage;

#[test]
fn paid_seed_live_and_edge_receivers_equal_old_full_rows_and_checked_work_frontiers() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let payer = CfgReservation::acquire().unwrap();
        let mut cfg_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        cfg_work.allocation = Some(&payer);
        let cfg = build_cfg(function, &mut cfg_work).unwrap();
        let starts = [0u16];
        let reachable = work::Reachability::build(&cfg, starts.into_iter(), &mut cfg_work).unwrap();
        let old_seeds: BTreeSet<_> = (0..cfg.points.len() as u16).collect();
        let mut seeds = cfg_directory::Set::new(&payer).unwrap();
        for seed in &old_seeds {
            cfg_directory::set_insert(&mut seeds, *seed, Some(&payer)).unwrap();
        }
        let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        old_work.allocation = Some(&payer);
        let expected =
            work::live_nodes(&cfg, LoanId(0), 0, &old_seeds, &reachable, &mut old_work).unwrap();
        let exact_work = old_work.used;
        for limit in [0, 1, 3, exact_work - 1, exact_work] {
            let mut old_work = WorkCounter::new(limit);
            old_work.allocation = Some(&payer);
            let old = work::live_nodes(&cfg, LoanId(0), 0, &old_seeds, &reachable, &mut old_work)
                .map(|nodes| nodes.into_iter().collect::<Vec<_>>());
            let mut current_work = WorkCounter::new(limit);
            current_work.allocation = Some(&payer);
            let current = live_nodes(&cfg, LoanId(0), 0, &seeds, &reachable, &mut current_work)
                .map(|nodes| nodes.into_vec(&payer).unwrap());
            assert_eq!(current, old);
            assert_eq!(current_work.used, old_work.used);
            assert_eq!(payer.first(), None);
        }
        let mut current_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        current_work.allocation = Some(&payer);
        let current =
            live_nodes(&cfg, LoanId(0), 0, &seeds, &reachable, &mut current_work).unwrap();
        let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        old_work.allocation = Some(&payer);
        let old_live = [expected];
        let current_live = [current];
        let expected = work::edge_liveness(&cfg, &old_live, &mut old_work).unwrap();
        let exact_work = old_work.used;
        assert!(exact_work > 0);
        for limit in [0, 1, exact_work - 1, exact_work] {
            let mut old_work = WorkCounter::new(limit);
            old_work.allocation = Some(&payer);
            let old = work::edge_liveness(&cfg, &old_live, &mut old_work);
            let mut current_work = WorkCounter::new(limit);
            current_work.allocation = Some(&payer);
            let current = edge_liveness(&cfg, &current_live, &mut current_work);
            assert_eq!(current, old);
            assert_eq!(current_work.used, old_work.used);
            if limit == exact_work {
                assert_eq!(current.unwrap(), expected);
            }
            assert_eq!(payer.first(), None);
        }
    });
    assert!(!refused);
}

#[test]
fn paid_full_plan_late_overlap_and_first_work_error_equal_saved_btree_receiver() {
    let (mut program, index) = boundary_tests::fixture_with_uses(3, true);
    let expected = saved_planner::build_plan(&program, &program.functions[index]).unwrap();
    assert_eq!(expected.loans.len(), 3);
    let owned = expected.loans[0].origin.root.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut program.functions[index].body.kind else {
        panic!("unchanged boundary block");
    };
    // Preserve the last two view uses, making the forged whole-owner transfer
    // conflict late in overlap validation after complete discovery/liveness.
    let ResolvedStatement::Let { value, .. } = &mut statements[4] else {
        panic!("first original use remains a let");
    };
    value.kind = ResolvedExprKind::Place(Place {
        root: owned,
        projections: Vec::new(),
    });
    value.ty = ResolvedType::Bytes;
    value.ownership = OwnershipMode::Own;
    let function = &program.functions[index];
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let payer = CfgReservation::acquire().unwrap();
        let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        old_work.allocation = Some(&payer);
        let expected = paid_plan_legacy::build(&program, function, &mut old_work).unwrap_err();
        assert_eq!(
            expected.message,
            "move, mutation, or transfer overlaps an active shared loan"
        );
        assert!(expected.span.is_some());
        assert!(expected.help.is_some());
        let exact_work = old_work.used;
        for limit in [0, 1, exact_work - 1, exact_work, MAX_LOAN_PLAN_WORK_V1] {
            let mut old_work = WorkCounter::new(limit);
            old_work.allocation = Some(&payer);
            let old = paid_plan_legacy::build(&program, function, &mut old_work).unwrap_err();
            let mut current_work = WorkCounter::new(limit);
            current_work.allocation = Some(&payer);
            let current = paid_plan::build(&program, function, &mut current_work).unwrap_err();
            assert_eq!(current, old);
            assert_eq!(current_work.used, old_work.used);
            if limit >= exact_work {
                assert_eq!(current, expected);
            }
            assert_eq!(payer.first(), None);
        }
        let old = funded_flat_legacy::build(&program, function, &payer);
        let current = funded_flat::build(&program, function, &payer);
        match (old, current) {
            (
                Err(storage::PlannerError::Semantic(old)),
                Err(storage::PlannerError::Semantic(current)),
            ) => {
                assert_eq!(current, old);
                assert_eq!(current, expected);
            }
            _ => panic!("full receiver lost its exact late semantic refusal"),
        }
        assert_eq!(payer.first(), None);
    });
    assert!(!refused);
}
