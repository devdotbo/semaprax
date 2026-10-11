//! Scalar Vec sort inside a lexical-control function. The canonical plan
//! declares no status lane for sort, so native lowering must not request a
//! context-selected failure exit for it, while every declared failure keeps
//! its exact context-selected exit authority.
use semaprax::cleanup_plan::{
    CleanupTransition, ExitContinuation, StatusLane, StatusProducer, StatusSource, StatusSourceId,
};
use semaprax::hir::{DeclarationId, ExpressionId, ResolvedFunction, ResolvedProgram};
use semaprax::{codegen, hir, interpreter, parse, verify, wasm};
use std::process::Command;

const SOURCE: &str = r#"module sort.lexical;
@id("sort.lexical.rank")
fn rank(limit: i64) -> i64
{
    let mut values = vec_with_capacity<i64>(4usize);
    values = vec_push<i64>(values, 9);
    values = vec_push<i64>(values, -3);
    values = vec_push<i64>(values, 4);
    values = vec_push<i64>(values, 5);
    let ordered = vec_sort<i64>(values);
    let mut index = 0usize;
    while index < 4usize {
        let value = vec_get<i64>(ordered, index);
        if value >= limit { return value; }
        index = index + 1usize;
        0
    }
    0
}
@id("sort.lexical.main")
fn main() -> i64
{
    if rank(4) == 4 && rank(7) == 9 && rank(10) == 0 && rank(11) == 0 { 42 } else { 1 }
}
"#;
const INVARIANT: &str =
    "spx_runtime_invariant_failure(\"plan-infallible Vec operation returned failure\");";

fn resolved() -> ResolvedProgram {
    let ast = parse(SOURCE, "sort-lexical.spx").unwrap();
    assert!(verify::verify(&ast).is_empty());
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    program
}

fn rank(program: &mut ResolvedProgram) -> &mut ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "sort.lexical.rank")
        .unwrap()
}

/// The only committed call without any declared status is the sort.
fn sort_occurrence(function: &ResolvedFunction) -> ExpressionId {
    let plan = &function.cleanup_plan;
    let mut calls = plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::CallCommit { call, .. } => Some(call.clone()),
            _ => None,
        })
        .filter(|call| {
            !plan
                .status_sources
                .iter()
                .any(|source| &source.id.expression == call)
        })
        .collect::<Vec<_>>();
    calls.sort();
    calls.dedup();
    assert_eq!(calls.len(), 1, "{calls:?}");
    calls.pop().unwrap()
}

fn operation_failures(function: &ResolvedFunction) -> Vec<usize> {
    function
        .cleanup_plan
        .exits
        .iter()
        .enumerate()
        .filter(|(_, exit)| {
            matches!(&exit.continuation, ExitContinuation::ReturnFailure { source }
                if source.lane == StatusLane::OperationFailure)
        })
        .map(|(index, _)| index)
        .collect()
}

fn refuse(program: &ResolvedProgram, case: &str) {
    let expected = hir::validate(program).expect_err(case);
    assert_eq!(expected.code, "SPX-H006", "{case}: {expected:?}");
    let error = codegen::emit_hir_c(program).expect_err(case);
    assert_eq!(
        (
            error.code,
            error.severity,
            error.message,
            error.path,
            error.span,
            error.help
        ),
        (
            expected.code,
            expected.severity,
            expected.message,
            expected.path,
            expected.span,
            expected.help
        ),
        "{case}: native must retain the independent replay diagnostic"
    );
}

#[test]
fn lexical_scalar_sort_has_no_failure_route_and_agrees_across_engines() {
    let program = resolved();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "sort.lexical.rank")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    let sort = sort_occurrence(function);
    for exit in &function.cleanup_plan.exits {
        if let ExitContinuation::ReturnFailure { source } = &exit.continuation {
            assert_ne!(source.expression, sort);
        }
    }
    assert!(!operation_failures(function).is_empty());

    let c = codegen::emit_hir_c(&program).unwrap();
    assert_eq!(c, codegen::emit_hir_c(&resolved()).unwrap(), "stable");
    assert_eq!(c.matches(INVARIANT).count(), 1);
    assert!(c.contains("spx_vec_sort(spx_ctx"));

    let root =
        std::env::temp_dir().join(format!("semaprax-vec-sort-lexical-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("sort.spx");
    std::fs::write(&path, SOURCE).unwrap();
    let result = interpreter::interpret(
        &path,
        "sort.lexical.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(
        result.envelope.contains("\"value\":\"42\""),
        "{}",
        result.envelope
    );
    let source = root.join("sort.c");
    std::fs::write(&source, &c).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("sort{optimization}"));
        let output = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for _ in 0..2 {
            let result = Command::new(&binary).output().unwrap();
            assert!(result.status.success(), "{result:?}");
            assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
            assert!(result.stderr.is_empty(), "{result:?}");
        }
    }
    let ast = parse(SOURCE, "sort-lexical.spx").unwrap();
    let module = root.join("sort.wasm");
    let bytes = wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    std::fs::write(&module, bytes).unwrap();
    let output = Command::new("node")
        .arg("-e")
        .arg(include_str!("../vec_sort_host.js"))
        .arg(&module)
        .output()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn lexical_failure_exits_refuse_declared_missing_wrong_and_foreign_context() {
    let base = resolved();

    // A forged status for the plan-infallible sort is never routed.
    let mut declared = base.clone();
    let function = rank(&mut declared);
    let sort = sort_occurrence(function);
    function.cleanup_plan.status_sources.push(StatusSource {
        id: StatusSourceId {
            expression: sort,
            lane: StatusLane::OperationFailure,
        },
        producer: StatusProducer::PropagatedCall {
            callee: DeclarationId::new("core.vec.sort"),
        },
    });
    refuse(&declared, "declared sort status");

    let failures = operation_failures(rank(&mut base.clone()));
    for &index in &failures {
        // Missing: the declared source keeps no exit of its own lane.
        let mut missing = base.clone();
        let exit = &mut rank(&mut missing).cleanup_plan.exits[index];
        let ExitContinuation::ReturnFailure { source } = &mut exit.continuation else {
            unreachable!()
        };
        source.lane = StatusLane::ContractFalse;
        refuse(&missing, "missing failure exit");

        // Wrong context: the exit moves into a finite resume proof fragment.
        let mut wrong = base.clone();
        let plan = &mut rank(&mut wrong).cleanup_plan;
        let resume = plan.lexical_control.as_ref().unwrap().resumes[0].entry;
        plan.exits[index].from = resume;
        refuse(&wrong, "wrong-context failure exit");

        // Foreign: the exit names a status owned by another function.
        let mut foreign = base.clone();
        let other = foreign
            .functions
            .iter()
            .find(|function| function.id.as_str() == "sort.lexical.main")
            .unwrap()
            .cleanup_plan
            .status_sources
            .iter()
            .find(|source| source.id.lane == StatusLane::OperationFailure)
            .unwrap()
            .id
            .clone();
        let exit = &mut rank(&mut foreign).cleanup_plan.exits[index];
        exit.continuation = ExitContinuation::ReturnFailure { source: other };
        refuse(&foreign, "foreign failure exit");
    }
    // The untouched authority still emits after every hostile copy.
    assert!(codegen::emit_hir_c(&base).is_ok());
}
