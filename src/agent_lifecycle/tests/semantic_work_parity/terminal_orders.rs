//! Actual native routes through opposing canonical terminal orders.
//!
//! `run` holds two sealed conditional variant owners, `older` (`body.s2`) and
//! `newer` (`body.s10`). Before the scalar `if` join they finalize newest-first;
//! the join rebuilds conditional roots by storage identity, so later exits
//! finalize `older` first. No single shared epilogue order honors both, and
//! lexical control selects every terminal exit at its source site instead.
//! Every route below is the metered native C11 artifact at O0 and O2: its
//! sticky status, fuel and copy-out match the interpreter, and its performed
//! finalizer trace equals the exact compiler-built vector(s) the route owns.
//! The common epilogue of `run` is `spx_runtime_invariant_failure`; any route
//! reaching it without completed cleanup aborts and the dispatch refuses.
use super::*;
use crate::cleanup_plan::{
    CleanupPlan, ContractPhase, ExecutionBoundaryKind, ExitContinuation, ExitTarget,
    FinalizeAction, StatusCase, StatusLane,
};
use crate::interpreter::retained_call::SemanticCleanupEvent;
use crate::runtime_status::{normalize_arithmetic, normalize_contract};

const ORDER_SOURCE: &str =
    include_str!("../../../codegen/native_bytes/terminal_exits/variant_owner_order.spx");
const ORDER_ENTRY: &str = "test.native_terminal_variant_order.run";
/// `depth, first, second, rounds, gate, cap`.
const OK: [i64; 6] = [0, 1, 1, 4, 1, 1_000];

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Returned,
    Arithmetic,
    Requires,
    Ensures,
    Fuel,
    Depth,
}

/// The exact compiler-built contour(s) a route performs, in execution order.
#[derive(Clone, Copy, Debug)]
enum Trace {
    Success,
    Ensures,
    Requires,
    Descend,
    BeforeJoin,
    AfterJoin,
    Iteration,
}

struct Route {
    name: &'static str,
    args: [i64; 6],
    limit: u64,
    /// Interpreter-owned semantic fuel: one unit per admitted frame entry and
    /// per entered loop body; exhaustion reports exactly the limit.
    fuel: u64,
    outcome: Outcome,
    trace: Trace,
}

const fn route(
    name: &'static str,
    args: [i64; 6],
    limit: u64,
    fuel: u64,
    outcome: Outcome,
    trace: Trace,
) -> Route {
    Route {
        name,
        args,
        limit,
        fuel,
        outcome,
        trace,
    }
}

const ROUTES: &[Route] = &[
    route("success", OK, 1_000, 7, Outcome::Returned, Trace::Success),
    // run, descend, two weigh frames and three loop bodies.
    route(
        "success-at-exact-limit",
        OK,
        7,
        7,
        Outcome::Returned,
        Trace::Success,
    ),
    // The entry charge precedes the precondition.
    route(
        "requires-false",
        [0, 1, 1, 4, 0, 1_000],
        1_000,
        1,
        Outcome::Requires,
        Trace::Requires,
    ),
    // run, descend and the failing weigh frame.
    route(
        "operation-failure-before-join",
        [0, 0, 1, 4, 1, 1_000],
        1_000,
        3,
        Outcome::Arithmetic,
        Trace::BeforeJoin,
    ),
    route(
        "operation-failure-after-join",
        [0, 1, 0, 4, 1, 1_000],
        1_000,
        4,
        Outcome::Arithmetic,
        Trace::AfterJoin,
    ),
    route(
        "ensures-false",
        [0, 1, 1, 4, 1, 0],
        1_000,
        7,
        Outcome::Ensures,
        Trace::Ensures,
    ),
    // run plus 255 descend frames; the 257th frame is refused uncharged.
    route(
        "call-depth-before-owners",
        [300, 1, 1, 4, 1, 1_000],
        100_000,
        256,
        Outcome::Depth,
        Trace::Descend,
    ),
    route(
        "fuel-at-callee-before-owners",
        OK,
        1,
        1,
        Outcome::Fuel,
        Trace::Descend,
    ),
    route(
        "fuel-at-callee-before-join",
        OK,
        2,
        2,
        Outcome::Fuel,
        Trace::BeforeJoin,
    ),
    route(
        "fuel-at-callee-after-join",
        OK,
        3,
        3,
        Outcome::Fuel,
        Trace::AfterJoin,
    ),
    route(
        "fuel-at-first-iteration",
        OK,
        4,
        4,
        Outcome::Fuel,
        Trace::Iteration,
    ),
    route(
        "fuel-at-last-iteration",
        OK,
        6,
        6,
        Outcome::Fuel,
        Trace::Iteration,
    ),
];

fn body_call(function: &hir::ResolvedFunction, index: usize) -> &hir::ExpressionId {
    let hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("run body is a block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &statements[index] else {
        panic!("run statement {index} is a let");
    };
    assert!(matches!(value.kind, hir::ResolvedExprKind::Call { .. }));
    &value.id
}

fn failure_exit<'p>(
    plan: &'p CleanupPlan,
    site: &hir::ExpressionId,
    lane: StatusLane,
) -> &'p ExitTarget {
    let mut exits = plan.exits.iter().filter(|exit| {
        plan.lexical_block_in_context(None, exit.from)
            && matches!(&exit.continuation, ExitContinuation::ReturnFailure { source }
                if source.expression == *site && source.lane == lane)
    });
    let exit = exits
        .next()
        .expect("status source has its exact ordinary exit");
    assert!(exits.next().is_none(), "status source selects one exit");
    exit
}

fn flags(actions: &[FinalizeAction]) -> Vec<u32> {
    actions.iter().map(|action| action.guard_flag.0).collect()
}

/// Exact performed vectors per route, read only from the checked plan.
fn traces(function: &hir::ResolvedFunction) -> impl Fn(Trace) -> Vec<u32> + '_ {
    let plan = &function.cleanup_plan;
    let control = plan.lexical_control.as_deref().expect("run is lexical");
    let mut success = control
        .function_success_exits
        .iter()
        .map(|id| &plan.exits[id.0 as usize])
        .filter(|exit| plan.lexical_block_in_context(None, exit.from));
    let commit = success.next().expect("one ordinary root commit");
    assert!(success.next().is_none());
    // Owners leaving a nested body scope settle before postconditions.
    let mut scopes = plan.exits.iter().filter(|exit| {
        matches!(exit.continuation, ExitContinuation::Continue(_))
            && plan.lexical_block_in_context(None, exit.from)
            && !exit.finalize_in_order.is_empty()
    });
    let scope = scopes
        .next()
        .map_or_else(Vec::new, |exit| flags(&exit.finalize_in_order));
    assert!(scopes.next().is_none(), "at most one owning scope end");
    let mut iterations = control
        .execution_boundaries
        .iter()
        .filter(|boundary| boundary.kind == ExecutionBoundaryKind::LoopIteration);
    let iteration = iterations
        .next()
        .expect("loop body charge has its boundary");
    assert!(iterations.next().is_none());
    let before = failure_exit(plan, body_call(function, 11), StatusLane::OperationFailure);
    let after = failure_exit(plan, body_call(function, 13), StatusLane::OperationFailure);
    let (before, after) = (
        flags(&before.finalize_in_order),
        flags(&after.finalize_in_order),
    );
    assert_eq!(before.len(), 2, "both conditional payload owners");
    assert_eq!(
        before,
        after.iter().rev().copied().collect::<Vec<_>>(),
        "the actual exits finalize the same owners in opposing canonical orders"
    );
    let requires = failure_exit(plan, &function.requires[0].id, StatusLane::ContractFalse);
    let ensures = failure_exit(plan, &function.ensures[0].id, StatusLane::ContractFalse);
    let descend = failure_exit(plan, body_call(function, 1), StatusLane::OperationFailure);
    move |trace| match trace {
        Trace::Success => [scope.clone(), flags(&commit.finalize_in_order)].concat(),
        Trace::Ensures => [scope.clone(), flags(&ensures.finalize_in_order)].concat(),
        Trace::Requires => flags(&requires.finalize_in_order),
        Trace::Descend => flags(&descend.finalize_in_order),
        Trace::BeforeJoin => before.clone(),
        Trace::AfterJoin => after.clone(),
        Trace::Iteration => flags(&iteration.finalize_in_order),
    }
}

fn assert_outcome(route: &Route, evaluation: &RetainedCallEvaluation) {
    let failure = RetainedCallOutcome::LanguageFailure;
    match route.outcome {
        Outcome::Returned => {
            let RetainedCallOutcome::Returned(RetainedValue::Record(summary)) = &evaluation.outcome
            else {
                panic!("{}: success did not publish its Summary", route.name);
            };
            assert_eq!(
                summary.record.as_str(),
                "test.native_terminal_variant_order.Summary"
            );
            let fields = summary
                .fields
                .iter()
                .map(|field| (field.field.as_str(), &field.value))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(fields.len(), 2, "{}", route.name);
            assert_eq!(
                fields.get("test.native_terminal_variant_order.Summary.tag"),
                Some(&&RetainedValue::Bytes(vec![83, 87]))
            );
            // reached 0 + before 100 + joined 1 + after 100 + index 3.
            assert_eq!(
                fields.get("test.native_terminal_variant_order.Summary.total"),
                Some(&&RetainedValue::I64(204))
            );
        }
        Outcome::Arithmetic => assert_eq!(
            evaluation.outcome,
            failure(normalize_arithmetic(StatusCase::DivisionByZero)),
            "{}",
            route.name
        ),
        Outcome::Requires => assert_eq!(
            evaluation.outcome,
            failure(normalize_contract(ContractPhase::Requires)),
            "{}",
            route.name
        ),
        Outcome::Ensures => assert_eq!(
            evaluation.outcome,
            failure(normalize_contract(ContractPhase::Ensures)),
            "{}",
            route.name
        ),
        Outcome::Fuel => assert_eq!(
            evaluation.outcome,
            RetainedCallOutcome::FuelExhausted,
            "{}",
            route.name
        ),
        Outcome::Depth => assert_eq!(
            evaluation.outcome,
            RetainedCallOutcome::CallDepthExceeded,
            "{}",
            route.name
        ),
    }
}

#[test]
fn opposing_terminal_orders_settle_exactly_on_every_native_route() {
    let checked = crate::check(ORDER_SOURCE, std::path::Path::new("variant-order.spx"))
        .expect("variant owner order source checks");
    let program = hir::resolve(&checked).expect("variant owner order source resolves");
    hir::validate(&program).expect("variant owner order source validates");
    let run = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == ORDER_ENTRY)
        .expect("run is declared");
    let expected_trace = traces(run);
    let generated = crate::codegen::emit_hir_c(&program).expect("native emits run");
    assert_eq!(
        generated
            .matches("native terminal cleanup lacks its exact selected exit")
            .count(),
        1,
        "run's common epilogue is the fail-closed physical guard"
    );
    let prepared = prepare_retained_call(&program, ORDER_ENTRY).expect("run stage prepares");
    let host = native_stage_host().expect("terminal order evidence requires held clang");
    for route in ROUTES {
        let args = route.args.map(RetainedValue::I64).to_vec();
        let expected = metered(
            authorization::StageBackend::Interpreter,
            &program,
            &prepared,
            &args,
            route.limit,
        );
        assert_outcome(route, &expected);
        assert_eq!(
            work(&expected).fuel_used,
            route.fuel,
            "{}: fuel",
            route.name
        );
        assert_eq!(
            work(&expected).fuel_limit,
            Some(route.limit),
            "{}: admitted limit",
            route.name
        );
        assert_eq!(
            work(&expected).exhausted,
            matches!(route.outcome, Outcome::Fuel),
            "{}: exhaustion",
            route.name
        );
        assert_eq!(
            work(&expected).finalizer_events,
            None,
            "{}: the interpreter performs no plan finalizer",
            route.name
        );
        let trace = expected_trace(route.trace)
            .into_iter()
            .map(|liveness_flag| SemanticCleanupEvent {
                function: run.id.clone(),
                liveness_flag,
            })
            .collect::<Vec<_>>();
        let mut settled = trace
            .iter()
            .map(|event| event.liveness_flag)
            .collect::<Vec<_>>();
        settled.sort_unstable();
        settled.dedup();
        assert_eq!(
            settled.len(),
            trace.len(),
            "{}: each owner settles once",
            route.name
        );
        for (leg, backend) in native_legs_on(&host) {
            // A trapped common epilogue aborts the probe and `metered` panics.
            let actual = metered(backend, &program, &prepared, &args, route.limit);
            assert_eq!(
                comparable(&actual),
                comparable(&expected),
                "{}: {leg} status, publication and fuel",
                route.name
            );
            assert_eq!(
                actual.steps_used, 0,
                "{}: {leg} reports no instruction count",
                route.name
            );
            assert_eq!(
                work(&actual).finalizer_events.as_deref(),
                Some(trace.as_slice()),
                "{}: {leg} exact canonical finalizer trace",
                route.name
            );
        }
    }
}

mod nonlexical;
