//! Actual native routes through opposing orders without lexical control.
//!
//! The same checked owners as the lexical witness, with the loop (the source's
//! only lexical exits) replaced by a scalar binding: `run` has no lexical
//! control and no loop. Native admits it only by exact nonlexical routing:
//! every failure by its unique declared status source, success by the root
//! normal scope end. The common epilogue of `run` traps if any terminal slot is
//! live and otherwise performs nothing; `enter` drives run's own uncertified
//! entry fuel refusal through that empty state. Every route is the metered
//! native C11 artifact at O0 and O2 and compares status, publication, copy-out
//! and absolute semantic fuel with the interpreter; its performed finalizer
//! trace equals the exact compiler-built vector(s) the route owns.
use super::*;
use crate::cleanup_plan::ExitTargetId;

/// The loop's `continue`/`break` are the source's only lexical exits.
const LEXICAL_LOOP: &str = "    let mut index = 0;\n    while index < rounds {\n        index = index + 1;\n        if index == 1 { continue; }\n        if index == 3 { break; }\n        0\n    }\n";
const ENTER: &str = "test.native_terminal_variant_order.enter";
/// Drives run's own frame entry from one ordinary caller frame.
const ENTER_SOURCE: &str = "@id(\"test.native_terminal_variant_order.enter\")\nfn enter(depth: i64, first: i64, second: i64, rounds: i64, gate: i64, cap: i64) -> Summary {\n    run(depth, first, second, rounds, gate, cap)\n}\n\n@id(\"app.main\")";

fn source() -> String {
    assert_eq!(ORDER_SOURCE.matches(LEXICAL_LOOP).count(), 1);
    assert_eq!(ORDER_SOURCE.matches("@id(\"app.main\")").count(), 1);
    ORDER_SOURCE
        .replace(LEXICAL_LOOP, "    let index = rounds;\n")
        .replace("@id(\"app.main\")", ENTER_SOURCE)
}

#[derive(Clone, Copy, Debug)]
enum Entry {
    Run,
    Enter,
    /// `enter` is charged; run's own uncertified entry charge is refused
    /// before any owner exists, so run performs no contour at all.
    RunEntryRefused,
}

struct NonlexicalRoute {
    route: Route,
    entry: Entry,
}

const fn at(entry: Entry, route: Route) -> NonlexicalRoute {
    NonlexicalRoute { route, entry }
}

/// `depth, first, second, rounds, gate, cap`.
const ROUTES: &[NonlexicalRoute] = &[
    // run, descend and two weigh frames; no loop body is charged.
    at(
        Entry::Run,
        route("success", OK, 1_000, 4, Outcome::Returned, Trace::Success),
    ),
    at(
        Entry::Run,
        route(
            "success-at-exact-limit",
            OK,
            4,
            4,
            Outcome::Returned,
            Trace::Success,
        ),
    ),
    // The entry charge precedes the precondition.
    at(
        Entry::Run,
        route(
            "requires-false",
            [0, 1, 1, 4, 0, 1_000],
            1_000,
            1,
            Outcome::Requires,
            Trace::Requires,
        ),
    ),
    at(
        Entry::Run,
        route(
            "operation-failure-before-join",
            [0, 0, 1, 4, 1, 1_000],
            1_000,
            3,
            Outcome::Arithmetic,
            Trace::BeforeJoin,
        ),
    ),
    at(
        Entry::Run,
        route(
            "operation-failure-after-join",
            [0, 1, 0, 4, 1, 1_000],
            1_000,
            4,
            Outcome::Arithmetic,
            Trace::AfterJoin,
        ),
    ),
    at(
        Entry::Run,
        route(
            "ensures-false",
            [0, 1, 1, 4, 1, 0],
            1_000,
            4,
            Outcome::Ensures,
            Trace::Ensures,
        ),
    ),
    // run plus 255 descend frames; the 257th frame is refused uncharged.
    at(
        Entry::Run,
        route(
            "call-depth-before-owners",
            [300, 1, 1, 4, 1, 1_000],
            100_000,
            256,
            Outcome::Depth,
            Trace::Descend,
        ),
    ),
    at(
        Entry::Run,
        route(
            "fuel-at-callee-before-owners",
            OK,
            1,
            1,
            Outcome::Fuel,
            Trace::Descend,
        ),
    ),
    at(
        Entry::Run,
        route(
            "fuel-at-callee-before-join",
            OK,
            2,
            2,
            Outcome::Fuel,
            Trace::BeforeJoin,
        ),
    ),
    at(
        Entry::Run,
        route(
            "fuel-at-callee-after-join",
            OK,
            3,
            3,
            Outcome::Fuel,
            Trace::AfterJoin,
        ),
    ),
    // The trace label is never read: run reaches no precondition or owner.
    at(
        Entry::RunEntryRefused,
        route(
            "fuel-at-run-entry",
            OK,
            1,
            1,
            Outcome::Fuel,
            Trace::Requires,
        ),
    ),
    at(
        Entry::Enter,
        route(
            "success-through-enter",
            OK,
            1_000,
            5,
            Outcome::Returned,
            Trace::Success,
        ),
    ),
    // A fresh invocation after exhaustion settles exactly again.
    at(
        Entry::Run,
        route(
            "reentry-success-after-exhaustion",
            OK,
            1_000,
            4,
            Outcome::Returned,
            Trace::Success,
        ),
    ),
];

/// Exact performed vectors per route, read only from the checked plan. The
/// root normal scope end is read structurally, independent of the router.
fn nonlexical_traces(
    function: &hir::ResolvedFunction,
) -> impl Fn(&NonlexicalRoute) -> Vec<u32> + '_ {
    let plan = &function.cleanup_plan;
    assert!(plan.lexical_control.is_none(), "run is nonlexical");
    let root = &plan.regions[0];
    assert!(root.parent.is_none());
    let commit = &plan.exits[root.normal_scope_end.0 as usize];
    assert_eq!(commit.id, root.normal_scope_end);
    assert!(matches!(
        commit.continuation,
        ExitContinuation::CommitResult { .. }
    ));
    assert_eq!(
        plan.exits
            .iter()
            .filter(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
            .map(|exit| exit.id)
            .collect::<Vec<ExitTargetId>>(),
        [commit.id],
        "the root normal scope end is the only ordinary commit"
    );
    let mut scopes = plan.exits.iter().filter(|exit| {
        matches!(exit.continuation, ExitContinuation::Continue(_))
            && !exit.finalize_in_order.is_empty()
    });
    let scope = scopes
        .next()
        .map_or_else(Vec::new, |exit| flags(&exit.finalize_in_order));
    assert!(scopes.next().is_none(), "at most one owning scope end");
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
    move |item| {
        if let Entry::RunEntryRefused = item.entry {
            return Vec::new();
        }
        match item.route.trace {
            Trace::Success => [scope.clone(), flags(&commit.finalize_in_order)].concat(),
            Trace::Ensures => [scope.clone(), flags(&ensures.finalize_in_order)].concat(),
            Trace::Requires => flags(&requires.finalize_in_order),
            Trace::Descend => flags(&descend.finalize_in_order),
            Trace::BeforeJoin => before.clone(),
            Trace::AfterJoin => after.clone(),
            Trace::Iteration => panic!("{}: run has no loop", item.route.name),
        }
    }
}

fn assert_nonlexical_outcome(route: &Route, evaluation: &RetainedCallEvaluation) {
    if !matches!(route.outcome, Outcome::Returned) {
        assert_outcome(route, evaluation);
        return;
    }
    let RetainedCallOutcome::Returned(RetainedValue::Record(summary)) = &evaluation.outcome else {
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
    // reached 0 + before 100 + joined 1 + after 100 + index (rounds) 4.
    assert_eq!(
        fields.get("test.native_terminal_variant_order.Summary.total"),
        Some(&&RetainedValue::I64(205))
    );
}

#[test]
fn nonlexical_opposing_terminal_orders_settle_exactly_on_every_native_route() {
    let source = source();
    let checked = crate::check(
        &source,
        std::path::Path::new("variant-order-nonlexical.spx"),
    )
    .expect("nonlexical variant owner order source checks");
    let program = hir::resolve(&checked).expect("nonlexical variant owner order source resolves");
    hir::validate(&program).expect("nonlexical variant owner order source validates");
    let run = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == ORDER_ENTRY)
        .expect("run is declared");
    let expected_trace = nonlexical_traces(run);
    let generated = crate::codegen::emit_hir_c(&program).expect("native emits nonlexical run");
    assert_eq!(
        generated
            .matches(
                "native nonlexical terminal cleanup reached the common epilogue with a live owner"
            )
            .count(),
        1,
        "run's common epilogue is the live-slot physical guard"
    );
    assert_eq!(
        generated
            .matches("native terminal cleanup lacks its exact selected exit")
            .count(),
        0,
        "no lexical exit-selected function exists"
    );
    let run_prepared = prepare_retained_call(&program, ORDER_ENTRY).expect("run stage prepares");
    let enter_prepared = prepare_retained_call(&program, ENTER).expect("enter stage prepares");
    let host = native_stage_host().expect("terminal order evidence requires held clang");
    for item in ROUTES {
        let route = &item.route;
        let prepared = match item.entry {
            Entry::Run => &run_prepared,
            Entry::Enter | Entry::RunEntryRefused => &enter_prepared,
        };
        let args = route.args.map(RetainedValue::I64).to_vec();
        let expected = metered(
            authorization::StageBackend::Interpreter,
            &program,
            prepared,
            &args,
            route.limit,
        );
        assert_nonlexical_outcome(route, &expected);
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
        let trace = expected_trace(item)
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
            let actual = metered(backend, &program, prepared, &args, route.limit);
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
            // enter owns no finalizer, so every performed event is run's.
            assert_eq!(
                work(&actual).finalizer_events.as_deref(),
                Some(trace.as_slice()),
                "{}: {leg} exact canonical finalizer trace",
                route.name
            );
        }
    }
}
