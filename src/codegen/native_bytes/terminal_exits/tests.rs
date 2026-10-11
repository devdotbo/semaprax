use super::*;
use crate::cleanup_plan::{
    BlockId, CleanupResultSource, ExitTargetId, FinalizeAction, LexicalControlPlan, StatusLane,
    StatusSourceId,
};

fn slot(name: &str) -> ByteSlot {
    ByteSlot {
        place: CleanupPlace {
            storage: StorageId::ProvisionalResult,
            projections: Vec::new(),
        },
        value: format!("value_{name}"),
        flag: format!("flag_{name}"),
        kind: OwnedLeafKind::Bytes,
    }
}

fn function_ids() -> (DeclarationId, ExpressionId, ExpressionId) {
    let source = crate::parse(
        "module test.native_terminal_exits; fn probe(value: i64) -> i64 { value } fn main() -> i64 { 0 }",
        std::path::Path::new("native-terminal-exits.spx"),
    )
    .unwrap();
    let resolved = crate::hir::resolve(&source).unwrap();
    let probe = resolved
        .functions
        .iter()
        .find(|function| function.name == "probe")
        .unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    (probe.id.clone(), probe.body.id.clone(), main.body.id.clone())
}

fn flags() -> BTreeMap<LivenessFlagId, ByteSlot> {
    BTreeMap::from([(LivenessFlagId(0), slot("a")), (LivenessFlagId(1), slot("b"))])
}

fn exit(id: u32, order: &[u32], continuation: ExitContinuation) -> ExitTarget {
    ExitTarget {
        id: ExitTargetId(id),
        from: BlockId(id),
        leaves_regions: Vec::new(),
        finalize_in_order: order
            .iter()
            .map(|flag| FinalizeAction {
                source: CleanupPlace {
                    storage: StorageId::ProvisionalResult,
                    projections: Vec::new(),
                },
                lifecycle_id: DeclarationId::new(crate::cleanup::BYTES_DROP_LIFECYCLE_ID),
                guard_flag: LivenessFlagId(*flag),
                active_case: None,
            })
            .collect(),
        continuation,
    }
}

fn failure(expression: &ExpressionId) -> ExitContinuation {
    ExitContinuation::ReturnFailure {
        source: StatusSourceId {
            expression: expression.clone(),
            lane: StatusLane::OperationFailure,
        },
    }
}

fn commit(expression: &ExpressionId) -> ExitContinuation {
    ExitContinuation::CommitResult {
        source: CleanupResultSource::Scalar {
            expression: expression.clone(),
        },
    }
}

fn lexical(success: &[u32]) -> Option<Box<LexicalControlPlan>> {
    Some(Box::new(LexicalControlPlan {
        schema: "semaprax.lexical-control-flow.v1",
        loops: Vec::new(),
        exits: Vec::new(),
        resumes: Vec::new(),
        abrupt_only_regions: Vec::new(),
        function_success_exits: success.iter().copied().map(ExitTargetId).collect(),
        execution_boundaries: Vec::new(),
    }))
}

fn plan(exits: Vec<ExitTarget>, control: Option<Box<LexicalControlPlan>>) -> CleanupPlan {
    let mut plan = CleanupPlan::unresolved();
    plan.exits = exits;
    plan.lexical_control = control;
    plan
}

#[test]
fn finalizer_order_accepts_reordered_subset_before_superset() {
    let a = slot("a");
    let b = slot("b");
    assert_eq!(
        canonical_finalizer_order(&[vec![a.clone()], vec![b.clone(), a.clone()]]).unwrap(),
        Some(vec![b, a])
    );
}

#[test]
fn finalizer_order_rejects_contradictory_exit_precedence() {
    let a = slot("a");
    let b = slot("b");
    assert_eq!(
        canonical_finalizer_order(&[vec![a.clone(), b.clone()], vec![b, a],]).unwrap(),
        None
    );
}

#[test]
fn consistent_exits_retain_one_shared_epilogue_order() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0], failure(&failed)),
            exit(1, &[1, 0], commit(&result)),
        ],
        None,
    );
    let order = terminal_order(&function, &plan, &flags()).unwrap();
    assert!(!order.exit_selected);
    assert_eq!(order.shared, [slot("b"), slot("a")]);
    assert_eq!(order.slots, BTreeSet::from([slot("a"), slot("b")]));
}

#[test]
fn nonterminal_exit_orders_never_join_the_terminal_union() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], ExitContinuation::Continue(crate::cleanup_plan::EdgeId(0))),
            exit(2, &[0, 1], commit(&result)),
        ],
        None,
    );
    let order = terminal_order(&function, &plan, &flags()).unwrap();
    assert!(!order.exit_selected);
    assert_eq!(order.shared, [slot("a"), slot("b")]);
}

#[test]
fn source_selected_lexical_exits_keep_each_exact_contradictory_order() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], commit(&result)),
        ],
        lexical(&[1]),
    );
    let order = terminal_order(&function, &plan, &flags()).unwrap();
    assert!(order.exit_selected);
    assert!(order.shared.is_empty());
    assert_eq!(order.slots, BTreeSet::from([slot("a"), slot("b")]));
    // The exact vectors themselves are untouched for the per-site router.
    assert_eq!(plan.exits[0].finalize_in_order[0].guard_flag, LivenessFlagId(0));
    assert_eq!(plan.exits[1].finalize_in_order[0].guard_flag, LivenessFlagId(1));
}

#[test]
fn contradictory_shared_epilogue_names_function_exits_and_slots() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], commit(&result)),
        ],
        None,
    );
    let error = terminal_order(&function, &plan, &flags()).unwrap_err();
    assert_eq!(error.code, "SPX-B104");
    assert!(error
        .message
        .starts_with("Bytes cleanup exits contain contradictory finalizer precedence"));
    for expected in [
        format!("function `{function}`"),
        format!("exit 0 (failure from `{failed}` OperationFailure) finalizes `flag_a`"),
        "before `flag_b`".to_owned(),
        "exit 1 (result commit) finalizes them in the opposite order".to_owned(),
        "no lexical exit selection".to_owned(),
    ] {
        assert!(error.message.contains(&expected), "{expected}: {error:?}");
    }
}

#[test]
fn hostile_ambiguous_failure_selection_still_refuses_contradiction() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], failure(&failed)),
            exit(2, &[], commit(&result)),
        ],
        lexical(&[2]),
    );
    let error = terminal_order(&function, &plan, &flags()).unwrap_err();
    assert!(error.message.contains("exit 0 (failure from"), "{error:?}");
    assert!(error
        .message
        .contains("is not exactly selected at its source site"));
}

/// Actual checked source: two conditional variant owners take opposing
/// canonical terminal orders (`variant_owner_order.spx`).
const VARIANT_OWNER_ORDER: &str = include_str!("variant_owner_order.spx");
/// The loop's `continue`/`break` are the source's only lexical exits.
const LEXICAL_LOOP: &str = "    let mut index = 0;\n    while index < rounds {\n        index = index + 1;\n        if index == 1 { continue; };\n        if index == 3 { break; };\n        0\n    }\n";

fn variant_owner_program(source: &str) -> (crate::ast::Program, crate::hir::ResolvedProgram) {
    let checked = crate::check(source, std::path::Path::new("variant-owner-order.spx"))
        .expect("variant owner order source checks");
    let program = crate::hir::resolve(&checked).expect("variant owner order source resolves");
    crate::hir::validate(&program).expect("independent replay admits the variant owner plan");
    (checked, program)
}

fn run_function(program: &crate::hir::ResolvedProgram) -> &crate::hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.native_terminal_variant_order.run")
        .expect("run is declared")
}

fn body_let(
    function: &crate::hir::ResolvedFunction,
    index: usize,
) -> (&crate::hir::ResolvedBinding, &crate::hir::ResolvedExpr) {
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("run body is a block");
    };
    let crate::hir::ResolvedStatement::Let { binding, value, .. } = &statements[index] else {
        panic!("run statement {index} is a let");
    };
    (binding, value)
}

/// The exact compiler-built failure exit selected by one callee call site.
fn call_failure<'p>(plan: &'p CleanupPlan, call: &ExpressionId) -> &'p ExitTarget {
    let mut exits = plan.exits.iter().filter(|exit| {
        matches!(&exit.continuation, ExitContinuation::ReturnFailure { source }
            if source.expression == *call && source.lane == StatusLane::OperationFailure)
    });
    let exit = exits.next().expect("call site has its exact failure exit");
    assert!(exits.next().is_none(), "call site selects one exit");
    exit
}

fn payload_order(exit: &ExitTarget) -> Vec<StorageId> {
    exit.finalize_in_order
        .iter()
        .map(|action| action.source.storage.clone())
        .collect()
}

/// `older` (`body.s2`) and `newer` (`body.s10`) are sealed conditional variants.
/// Before any join they finalize newest-first; the scalar `if` join rebuilds
/// conditional roots by storage identity, where `…body.s10` precedes
/// `…body.s2`, so later exits finalize `older` first. Both orders are canonical.
#[test]
fn checked_variant_owners_take_opposing_terminal_orders_and_lexical_sites_select_each() {
    let (checked, program) = variant_owner_program(VARIANT_OWNER_ORDER);
    let run = run_function(&program);
    let plan = &run.cleanup_plan;
    assert!(plan.lexical_control.is_some(), "loop exits make run lexical");
    let (older, _) = body_let(run, 2);
    let (newer, _) = body_let(run, 10);
    let older = StorageId::Value(older.id.clone());
    let newer = StorageId::Value(newer.id.clone());
    let before = call_failure(plan, &body_let(run, 11).1.id);
    let after = call_failure(plan, &body_let(run, 13).1.id);
    assert_eq!(payload_order(before), [newer.clone(), older.clone()]);
    assert_eq!(payload_order(after), [older, newer]);
    assert!(plan.lexical_block_in_context(None, before.from));
    assert!(plan.lexical_block_in_context(None, after.from));

    let native = NativeBytesPlan::build(run)
        .expect("lexical selection admits the opposing exact exits")
        .expect("run owns Bytes slots");
    assert!(native.exit_selected_terminal);
    assert!(native.finalizers.is_empty(), "no shared order is invented");
    assert_eq!(
        native.epilogue(),
        "    spx_runtime_invariant_failure(\"native terminal cleanup lacks its exact selected exit\");\n"
    );
    let generated = crate::codegen::emit_c(&checked).expect("native emits the lexical owner");
    assert_eq!(
        generated
            .matches("native terminal cleanup lacks its exact selected exit")
            .count(),
        1,
        "only run's unreachable common epilogue fails closed"
    );
}

/// The same owners without lexical selection keep one shared epilogue, which
/// cannot honor both exits: SPX-B104 names the function, both exits and slots.
#[test]
fn checked_nonlexical_variant_owner_conflict_still_refuses_with_exit_and_slot_detail() {
    assert_eq!(VARIANT_OWNER_ORDER.matches(LEXICAL_LOOP).count(), 1);
    let source = VARIANT_OWNER_ORDER.replace(LEXICAL_LOOP, "    let index = rounds;\n");
    let (checked, program) = variant_owner_program(&source);
    let run = run_function(&program);
    let plan = &run.cleanup_plan;
    assert!(plan.lexical_control.is_none());
    let before = call_failure(plan, &body_let(run, 11).1.id);
    let after = call_failure(plan, &body_let(run, 13).1.id);
    let flag = |exit: &ExitTarget, index: usize| {
        format!("spx_bytes_live_{}", exit.finalize_in_order[index].guard_flag.0)
    };
    assert_eq!(flag(before, 0), flag(after, 1));
    assert_eq!(flag(before, 1), flag(after, 0));
    let error = NativeBytesPlan::build(run).unwrap_err();
    assert_eq!(error.code, "SPX-B104");
    for expected in [
        format!(
            "contradictory finalizer precedence in function `{}`",
            run.id
        ),
        format!("{} finalizes `{}`", exit_label(before), flag(before, 0)),
        format!("before `{}`", flag(before, 1)),
        format!(
            "but {} finalizes them in the opposite order",
            exit_label(after)
        ),
        "no lexical exit selection".to_owned(),
    ] {
        assert!(error.message.contains(&expected), "{expected}: {error:?}");
    }
    assert_eq!(crate::codegen::emit_c(&checked).unwrap_err().code, "SPX-B104");
}

#[test]
fn hostile_uncertified_commit_still_refuses_contradiction() {
    let (function, failed, result) = function_ids();
    let certified: [&[u32]; 3] = [&[], &[0], &[1, 1]];
    for success in certified {
        let plan = plan(
            vec![
                exit(0, &[0, 1], failure(&failed)),
                exit(1, &[1, 0], commit(&result)),
            ],
            lexical(success),
        );
        let error = terminal_order(&function, &plan, &flags()).unwrap_err();
        assert!(
            error
                .message
                .contains("exit 1 (result commit) is not exactly selected"),
            "{success:?}: {error:?}"
        );
    }
}
