//! Complete prior receiver, identity/order and actual paid storage controls.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

fn old_union(rows: &[Vec<LoanId>], owner: &CfgReservation) -> Vec<LoanId> {
    let mut values = cfg_directory::Set::new(owner).unwrap();
    for row in rows {
        for id in row {
            cfg_directory::set_insert(&mut values, *id, Some(owner)).unwrap();
        }
    }
    values.into_vec(owner).unwrap()
}
#[test]
fn ordered_endpoint_union_keeps_full_id_domain_duplicates_and_malformed_old_behavior() {
    let mut cases = vec![
        vec![],
        vec![vec![]],
        vec![vec![LoanId(0)]],
        vec![
            vec![LoanId(0), LoanId(0), LoanId(u16::MAX)],
            vec![],
            vec![LoanId(0), LoanId(1), LoanId(u16::MAX)],
        ],
        vec![
            vec![LoanId(9), LoanId(1), LoanId(9)],
            vec![LoanId(2), LoanId(0)],
        ],
    ];
    cases.push(
        (0..257)
            .map(|i| vec![LoanId(i), LoanId(300), LoanId(u16::MAX)])
            .collect(),
    );
    for rows in cases {
        let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let expected = old_union(&rows, &owner);
            let current = ordered_union::merge(rows.iter().map(Vec::as_slice), &owner).unwrap();
            assert_eq!(current, expected);
            assert_eq!(current.capacity(), current.len());
            assert!(current.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(owner.first(), None);
        });
        assert!(!refused);
        assert_eq!(first, None);
    }
}
// Reconstruct both materializer inputs from the independent complete old plan,
// retaining every actual site/place/parent/edge rather than a count projection.
fn run_materializer(
    function: &ResolvedFunction,
    plan: &LoanPlan,
    owner: &CfgReservation,
    old: bool,
    missing_end: bool,
    missing_edge_rows: bool,
) -> (Result<LoanPlan, Diagnostic>, usize) {
    let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    work.allocation = Some(owner);
    let cfg = build_cfg(function, &mut work).unwrap();
    let drafts: Vec<CfgDraft<()>> = plan
        .loans
        .iter()
        .map(|loan| CfgDraft {
            site: loan.site.clone(),
            origin: loan.origin.clone(),
            origin_span: function.body.span,
            parent_root: None,
            binding: None,
            start: *cfg.nodes.get(&loan.start).unwrap(),
            seeds: (),
            cause: loan.cause.clone(),
        })
        .collect();
    let parents = plan.loans.iter().map(|loan| loan.parent).collect();
    // The public planner may return empty_v1 before building any CFG. A
    // materializer input still needs one liveness row per actual CFG edge.
    // Keep its original malformed zero-row input as an explicit panic control.
    let live = if missing_edge_rows {
        Vec::new()
    } else if plan.edges.is_empty() {
        assert!(plan.loans.is_empty());
        (0..cfg.edges.len()).map(|_| Vec::new()).collect()
    } else {
        assert_eq!(plan.edges.len(), cfg.edges.len());
        assert!(plan
            .edges
            .iter()
            .zip(&cfg.edges)
            .all(|(edge, pair)| (edge.from, edge.to) == *pair));
        plan.edges.iter().map(|edge| edge.live.to_vec()).collect()
    };
    let ends = plan
        .loans
        .iter()
        .map(|loan| {
            if missing_end {
                Vec::new()
            } else {
                loan.end_edges.clone()
            }
        })
        .collect();
    let used = work.used;
    let result = if old {
        before::materialize(cfg, drafts, parents, live, ends, owner)
    } else {
        materialize(cfg, drafts, parents, live, ends, owner)
    };
    assert_eq!(
        work.used, used,
        "materialization neither grants nor consumes logical work"
    );
    (result, used)
}
#[test]
fn ordered_endpoint_union_complete_old_materializer_plan_and_first_error_are_exact() {
    for count in [0, 1, 3] {
        let (program, index) = boundary_tests::fixture_with_uses(count, true);
        let function = &program.functions[index];
        let plan = saved_planner::build_plan(&program, function).unwrap();
        let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let (old, old_work) = run_materializer(function, &plan, &owner, true, false, false);
            let (current, current_work) =
                run_materializer(function, &plan, &owner, false, false, false);
            assert_eq!(current_work, old_work);
            assert_eq!(current, old);
            // Empty plans can have no CFG rows in the public early return;
            // both complete materializers still produce identical full rows.
            if !plan.loans.is_empty() {
                assert_eq!(current.unwrap(), plan);
                let (old, _) = run_materializer(function, &plan, &owner, true, true, false);
                let (current, _) = run_materializer(function, &plan, &owner, false, true, false);
                assert_eq!(current, old);
                assert_eq!(
                    current.unwrap_err().message,
                    "loan has no bounded CFG termination edge"
                );
            }
            if count == 0 {
                let inherited = |saved| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_materializer(function, &plan, &owner, saved, false, true)
                    }))
                    .expect_err(
                        "the full old/new internal receiver both reject missing CFG rows by panic",
                    )
                };
                let old = inherited(true);
                let current = inherited(false);
                let message = |value: &(dyn std::any::Any + Send)| -> String {
                    if let Some(value) = value.downcast_ref::<String>() {
                        value.clone()
                    } else if let Some(value) = value.downcast_ref::<&'static str>() {
                        (*value).to_owned()
                    } else {
                        panic!("expected inherited index panic payload")
                    }
                };
                assert_eq!(message(&*current), message(&*old));
                assert!(message(&*current).contains("index out of bounds"));
            } else {
                // A missing termination must select the original stable full
                // diagnostic before even the malformed edge rows are read.
                let (old, _) = run_materializer(function, &plan, &owner, true, true, true);
                let (current, _) = run_materializer(function, &plan, &owner, false, true, true);
                assert_eq!(current, old);
                assert_eq!(
                    current.unwrap_err(),
                    error("loan has no bounded CFG termination edge")
                );
            }
            assert_eq!(owner.first(), None);
        });
        assert!(!refused);
        assert_eq!(first, None);
    }
}
fn physical(rows: &[Vec<LoanId>], limit: usize) -> (bool, usize) {
    let ((success, metrics), refused, debit, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let owner = match CfgReservation::acquire() {
                Ok(owner) => owner,
                Err(_) => {
                    assert!(!reserve_active_required(0));
                    return false;
                }
            };
            let result = ordered_union::merge(rows.iter().map(Vec::as_slice), &owner);
            let success = result.is_ok();
            if let Ok(values) = &result {
                assert_eq!(values.capacity(), values.len());
            } else {
                let reason = owner.first().unwrap();
                let at_stop = current_metrics().allocated;
                assert_eq!(owner.reserve(0), Err(reason));
                // Refused retry must authenticate before reading even the iterator.
                let retry = ordered_union::merge(
                    std::iter::from_fn(|| -> Option<&[LoanId]> {
                        panic!("selected refusal must precede input reads")
                    }),
                    &owner,
                );
                assert!(retry.is_err());
                assert_eq!(owner.first(), Some(reason));
                drop(retry);
                assert_eq!(current_metrics().allocated, at_stop);
            }
            drop(result);
            drop(owner);
            success
        })
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= debit);
    assert_eq!(success, !refused);
    assert_eq!(first.is_none(), success);
    (success, debit)
}
#[test]
fn ordered_endpoint_union_exact_short_zero_sticky_and_full_drop_are_physically_paid() {
    for rows in [
        vec![],
        vec![vec![]],
        vec![vec![LoanId(7)]],
        (0..129)
            .map(|i| vec![LoanId(i), LoanId(400), LoanId(u16::MAX)])
            .collect(),
        vec![vec![LoanId(9), LoanId(1), LoanId(9)]],
    ] {
        let (ok, exact) = physical(&rows, usize::MAX);
        assert!(ok);
        for limit in [0, exact - 1, exact, exact + 1] {
            assert_eq!(physical(&rows, limit).0, limit >= exact);
        }
    }
}
