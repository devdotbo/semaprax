//! Whole-planner oracle for the flat adjacency, and its allocation owner.
//!
//! The independent oracle is `saved_planner`: the original planner relocated
//! verbatim (nested `Vec<Vec<u16>>` adjacency, original move-to-front queue,
//! original `Vec`-returning child list, original materialization), over the
//! common public `LoanPlan`, `Loan`, endpoint and edge types. The authored
//! boundary and renewal sources are checked; padded detached functions and
//! synthetic cap boundaries are structural planner fixtures, not evidence of
//! complete source/HIR admission. Intended positives are unwrapped, so a
//! refusal can never masquerade as coverage.

use std::path::Path;

use crate::test_allocation_observer::{measure, Metrics};

use super::boundary_tests as fixtures;
use super::*;

const WORK_ERROR: &str = "loan analysis exceeds 1,000,000 checked work";

enum Outcome {
    Plan {
        min_loans: usize,
    },
    /// The structural refusal both planners must select, with its message.
    Refused(&'static str),
    /// Plans without a limit but exceeds the production 1,000,000 limit.
    OverProductionWork,
}

struct Source {
    label: String,
    program: ResolvedProgram,
    function: ResolvedFunction,
    outcome: Outcome,
    /// The function owns a root, so the CFG planner runs on it.
    candidate: bool,
    /// Small enough for the observer's fixed live-allocation registry.
    observed: bool,
}

fn source(
    label: &str,
    program: &ResolvedProgram,
    function: ResolvedFunction,
    outcome: Outcome,
    observed: bool,
) -> Source {
    Source {
        label: label.to_string(),
        program: program.clone(),
        function,
        outcome,
        candidate: true,
        observed,
    }
}

/// `heavy` adds the cap, point-count and checked-work boundary sources, which
/// are too large for the observer's registry and are not allocation-observed.
fn sources(heavy: bool) -> Vec<Source> {
    let mut out = Vec::new();

    let (program, index) = fixtures::fixture_with_uses(3, true);
    let base = program.functions[index].clone();
    let mut padded = base.clone();
    fixtures::add_live_padding(&mut padded, 3, "oracle.live", 2, 2, 2);
    fixtures::add_padding(&mut padded, "oracle.disconnected", 1, 1, 1);
    out.push(source(
        "boundary base",
        &program,
        base,
        Outcome::Plan { min_loans: 3 },
        true,
    ));
    out.push(source(
        "boundary padded",
        &program,
        padded,
        Outcome::Plan { min_loans: 3 },
        true,
    ));

    let (program, index) = fixtures::fixture_with_uses(32, true);
    out.push(source(
        "32 views with last uses",
        &program,
        program.functions[index].clone(),
        Outcome::Plan { min_loans: 32 },
        true,
    ));

    if heavy {
        push_heavy_boundaries(&mut out);
    }

    let text = include_str!("../../tests/language/while_loops/record_borrow_renewal.spx");
    let ast = crate::check(text, Path::new("oracle-renewal.spx")).unwrap();
    let renewal = crate::hir::resolve(&ast).unwrap();
    for function in &renewal.functions {
        let candidate = has_own_root_candidate(&renewal, function).unwrap();
        out.push(Source {
            label: format!("renewal {}", function.id.as_str()),
            program: renewal.clone(),
            function: function.clone(),
            outcome: Outcome::Plan { min_loans: 0 },
            candidate,
            observed: true,
        });
    }
    out
}

fn push_heavy_boundaries(out: &mut Vec<Source>) {
    // 128 own-root views plus their 128 last-use calls are exactly 256 loans;
    // one additional view is the first overflow; retain its source refusal too.
    let (program, index) = fixtures::fixture_with_uses(fixtures::WORK_ROOT_LOANS, true);
    out.push(source(
        "256-loan boundary",
        &program,
        program.functions[index].clone(),
        Outcome::Plan {
            min_loans: MAX_LOANS_PER_FUNCTION_V1,
        },
        false,
    ));
    let (program, _index, overflow) = fixtures::first_loan_overflow();
    out.push(source(
        "257-loan overflow",
        &program,
        overflow,
        Outcome::Refused("function exceeds 256 shared loans"),
        false,
    ));

    let (program, index) = fixtures::fixture(1);
    let mut points = program.functions[index].clone();
    let (base_points, _) = fixtures::cfg_counts(&points);
    let leaves = (MAX_LOAN_ENDPOINTS_V1 - base_points) / 2;
    fixtures::add_padding(&mut points, "point.boundary", leaves, 0, 0);
    out.push(source(
        "4,096 program points",
        &program,
        points.clone(),
        Outcome::Plan { min_loans: 1 },
        false,
    ));
    fixtures::add_padding(&mut points, "point.overflow", 1, 0, 0);
    out.push(source(
        "4,098 program points",
        &program,
        points,
        Outcome::Refused("function exceeds 4,096 loan program points"),
        false,
    ));

    let (program, _index, exact) = fixtures::exact_million_work_boundary();
    let mut over = exact.clone();
    fixtures::add_live_padding(
        &mut over,
        fixtures::WORK_ROOT_LOANS,
        "work.overflow",
        1,
        0,
        0,
    );
    out.push(source(
        "exactly 1,000,000 checked work",
        &program,
        exact,
        Outcome::Plan {
            min_loans: MAX_LOANS_PER_FUNCTION_V1,
        },
        false,
    ));
    out.push(source(
        "1,000,001 checked work",
        &program,
        over,
        Outcome::OverProductionWork,
        false,
    ));
}

type Counted = (Result<LoanPlan, Diagnostic>, usize);

fn assert_outcome(label: &str, result: &Result<LoanPlan, Diagnostic>, outcome: &Outcome) {
    match outcome {
        Outcome::Plan { min_loans } => {
            let plan = result
                .as_ref()
                .unwrap_or_else(|error| panic!("{label}: intended positive refused: {error:?}"));
            assert!(plan.loans.len() >= *min_loans, "{label}");
        }
        Outcome::Refused(message) => {
            let error = result.as_ref().expect_err(label);
            assert_eq!(error.code, "SPX-H006", "{label}");
            assert_eq!(error.message, *message, "{label}");
        }
        Outcome::OverProductionWork => {
            let error = result.as_ref().expect_err(label);
            assert_eq!(error.code, "SPX-H006", "{label}");
            assert_eq!(error.message, WORK_ERROR, "{label}");
            assert!(
                error.help.is_some(),
                "{label}: work refusal names the function"
            );
        }
    }
}

fn counted_pair(source: &Source, limit: usize) -> (Counted, Counted) {
    (
        build_cfg_plan_with_work_limit(&source.program, &source.function, limit),
        saved_planner::build_cfg_plan_with_work_limit(&source.program, &source.function, limit),
    )
}

#[test]
fn flat_planner_matches_the_saved_planner_result_and_work_at_every_limit() {
    let mut ladders = 0;
    let mut renewal_loans = 0;
    for source in sources(true) {
        let label = source.label.as_str();
        let new_public = build_plan(&source.program, &source.function);
        let old_public = saved_planner::build_plan(&source.program, &source.function);
        assert_eq!(new_public, old_public, "{label}: public build_plan");
        if label.starts_with("renewal ") {
            let plan = new_public.as_ref().unwrap();
            renewal_loans += plan.loans.len();
        }
        // The production limit applies to the public entry point only.
        assert_outcome(label, &new_public, &source.outcome);
        if !source.candidate {
            // No own root: the CFG planner never runs, so no loan exists.
            assert!(new_public.unwrap().loans.is_empty(), "{label}");
            continue;
        }

        let (new, old) = counted_pair(&source, usize::MAX);
        assert_eq!(new, old, "{label}: unlimited");
        let used = new.1;
        assert!(used > 0, "{label}");
        match &source.outcome {
            Outcome::Plan { .. } => {
                new.0
                    .as_ref()
                    .unwrap_or_else(|error| panic!("{label}: {error:?}"));
            }
            Outcome::OverProductionWork => {
                new.0
                    .as_ref()
                    .unwrap_or_else(|error| panic!("{label}: {error:?}"));
                assert!(used > MAX_LOAN_PLAN_WORK_V1, "{label}");
            }
            Outcome::Refused(message) => {
                assert_eq!(new.0.as_ref().unwrap_err().message, *message, "{label}");
            }
        }

        // Exact limit reproduces the unlimited result and ledger.
        let (exact_new, exact_old) = counted_pair(&source, used);
        assert_eq!(exact_new, exact_old, "{label}: exact limit");
        assert_eq!(exact_new, new, "{label}: exact limit is the unlimited run");

        // One short fails at the last charged unit, with the same diagnostic.
        let (short_new, short_old) = counted_pair(&source, used - 1);
        assert_eq!(short_new, short_old, "{label}: one short");
        if !matches!(source.outcome, Outcome::Refused(_)) {
            assert_eq!(
                short_new.0.as_ref().unwrap_err().message,
                WORK_ERROR,
                "{label}"
            );
            assert_eq!(short_new.1, used, "{label}: one-short ledger");
        }

        // A mid-run limit and zero fail identically; zero stops at unit one.
        let (half_new, half_old) = counted_pair(&source, used / 2);
        assert_eq!(half_new, half_old, "{label}: half limit");
        let (zero_new, zero_old) = counted_pair(&source, 0);
        assert_eq!(zero_new, zero_old, "{label}: zero limit");
        assert_eq!(
            zero_new.0.as_ref().unwrap_err().message,
            WORK_ERROR,
            "{label}"
        );
        assert_eq!(zero_new.1, 1, "{label}: zero-limit ledger");
        ladders += 1;
    }
    assert!(
        ladders >= 9,
        "boundary, padded, cap, points, work and renewal ladders"
    );
    assert!(renewal_loans > 0, "the renewal source must plan real loans");
}

type Planner = fn(&ResolvedProgram, &ResolvedFunction) -> Result<LoanPlan, Diagnostic>;
type Census = (usize, usize, usize);

fn census(plan: &LoanPlan) -> Census {
    (plan.loans.len(), plan.endpoints.len(), plan.edges.len())
}

/// Constructor through Drop: the plan is built, counted, and dropped inside the
/// observation. The census is a fixed tuple, so the observed region performs no
/// bookkeeping allocation of its own.
fn observed_planning(planner: Planner, source: &Source) -> (Census, Metrics) {
    measure(|| {
        let plan = planner(&source.program, &source.function).expect("observed positive plans");
        let counts = census(&plan);
        drop(plan);
        counts
    })
}

#[test]
fn whole_planning_through_drop_requests_fewer_allocations_and_bytes_than_the_saved_planner() {
    let mut owners = 0;
    for source in sources(false).into_iter().filter(|source| source.observed) {
        let label = source.label.as_str();
        // Exact semantic plans and the census are taken outside observation.
        let old_plan = saved_planner::build_plan(&source.program, &source.function).unwrap();
        let new_plan = build_plan(&source.program, &source.function).unwrap();
        assert_eq!(new_plan, old_plan, "{label}");
        let expected = census(&new_plan);
        drop((old_plan, new_plan));
        if expected.0 == 0 {
            // An empty plan builds no CFG tables; there is nothing to compare.
            continue;
        }

        let (old_census, old) = observed_planning(saved_planner::build_plan, &source);
        let (new_census, new) = observed_planning(build_plan, &source);
        assert_eq!(old_census, expected, "{label}");
        assert_eq!(new_census, expected, "{label}");
        for (side, metrics) in [("saved", old), ("flat", new)] {
            assert!(!metrics.invalid, "{label}: {side} observation invalid");
            assert_eq!(metrics.live, 0, "{label}: {side} retained backing");
            assert_eq!(
                metrics.freed, metrics.allocated,
                "{label}: {side} not fully freed"
            );
        }
        // Genuine reductions, compared exactly: no discount or tolerance.
        assert!(
            new.allocations < old.allocations,
            "{label}: {new:?} vs {old:?}"
        );
        assert!(new.allocated < old.allocated, "{label}: {new:?} vs {old:?}");
        // The observation is deterministic, so repeating it is exact.
        assert_eq!(
            observed_planning(build_plan, &source),
            (new_census, new),
            "{label}"
        );
        owners += 1;
    }
    assert!(
        owners >= 3,
        "boundary base, padded and 32-view owners must be measured"
    );
}

#[test]
fn saved_planner_refuses_new_discard_in_body_and_contracts_before_shared_helpers() {
    let (program, index) = boundary_tests::fixture(1);
    let original = &program.functions[index];
    for contract in [false, true] {
        // This is a feature-exclusion forgery, not an admitted HIR program.
        let mut function = original.clone();
        let value = function.body.clone();
        let mut block = function.body.clone();
        let ResolvedExprKind::Block { statements, .. } = &mut block.kind else {
            panic!("boundary block")
        };
        statements.push(ResolvedStatement::CopyDiscard {
            span: value.span,
            value,
        });
        if contract {
            function.requires.push(block);
        } else {
            function.body = block;
        }
        let expected = "saved c8a loan planner does not admit CopyDiscard";
        assert_eq!(
            saved_planner::build_plan(&program, &function)
                .unwrap_err()
                .message,
            expected
        );
        let (result, used) = saved_planner::build_cfg_plan_with_work_limit(&program, &function, 0);
        assert_eq!(result.unwrap_err().message, expected);
        assert_eq!(used, 0);
    }
}
