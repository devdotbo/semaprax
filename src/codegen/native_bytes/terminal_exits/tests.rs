use super::*;
use crate::cleanup_plan::{
    BlockId, CleanupBlock, CleanupRegion, CleanupRegionId, CleanupResultSource, CleanupTerminator,
    ConditionalVariantEntry, ExitTargetId, FinalizeAction, LexicalControlPlan, StatusLane,
    StatusProducer, StatusSource, StatusSourceId,
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

const NONLEXICAL_GUARD: &str =
    "native nonlexical terminal cleanup reached the common epilogue with a live owner";
const LEXICAL_TRAP: &str = "native terminal cleanup lacks its exact selected exit";

/// The same owners without the loop: `run` has no lexical control.
fn nonlexical_variant_source() -> String {
    assert_eq!(VARIANT_OWNER_ORDER.matches(LEXICAL_LOOP).count(), 1);
    VARIANT_OWNER_ORDER.replace(LEXICAL_LOOP, "    let index = rounds;\n")
}

fn live_flag(exit: &ExitTarget, index: usize) -> String {
    format!("spx_bytes_live_{}", exit.finalize_in_order[index].guard_flag.0)
}

/// Some emitted finalizer line for `first` is immediately followed by `second`.
fn assert_adjacent_finalizers(generated: &str, first: &str, second: &str) {
    let lines = generated.lines().map(str::trim).collect::<Vec<_>>();
    let (first, second) = (format!("if ({first}) {{"), format!("if ({second}) {{"));
    assert!(
        lines
            .windows(2)
            .any(|pair| pair[0].starts_with(&first) && pair[1].starts_with(&second)),
        "no emitted route finalizes `{first}` immediately before `{second}`"
    );
}

/// Without lexical selection every exit is still authenticated by its plan
/// link: each failure by its unique declared status source, success by the
/// root normal scope end. Native admits both opposing exact vectors verbatim,
/// and the common epilogue only traps when a terminal slot is still live.
#[test]
fn checked_nonlexical_variant_owners_admit_exact_routes_with_actual_plan_vectors() {
    let (checked, program) = variant_owner_program(&nonlexical_variant_source());
    let run = run_function(&program);
    let plan = &run.cleanup_plan;
    assert!(plan.lexical_control.is_none());
    let (older, _) = body_let(run, 2);
    let (newer, _) = body_let(run, 10);
    let older = StorageId::Value(older.id.clone());
    let newer = StorageId::Value(newer.id.clone());
    let before = call_failure(plan, &body_let(run, 11).1.id);
    let after = call_failure(plan, &body_let(run, 13).1.id);
    assert_eq!(payload_order(before), [newer.clone(), older.clone()]);
    assert_eq!(payload_order(after), [older, newer]);
    assert_eq!(live_flag(before, 0), live_flag(after, 1));
    assert_eq!(live_flag(before, 1), live_flag(after, 0));
    for exit in [before, after] {
        let ExitContinuation::ReturnFailure { source } = &exit.continuation else {
            panic!("call exit is a failure");
        };
        assert!(std::ptr::eq(
            nonlexical_failure_exit(plan, source).expect("status source link is exact"),
            exit
        ));
    }
    let commit = nonlexical_success_exit(plan).expect("root success link is exact");
    assert_eq!(commit.id, plan.regions[0].normal_scope_end);
    assert!(matches!(
        commit.continuation,
        ExitContinuation::CommitResult { .. }
    ));

    let native = NativeBytesPlan::build(run)
        .expect("authenticated nonlexical links admit the opposing exact exits")
        .expect("run owns Bytes slots");
    assert!(native.exit_selected_terminal);
    assert!(native.nonlexical_terminal_routes());
    assert!(native.finalizers.is_empty(), "no shared order is invented");
    // Each route's exact vector reaches the router verbatim.
    let position = |code: &str, flag: &str| {
        code.find(&format!("if ({flag}) {{"))
            .unwrap_or_else(|| panic!("route finalizes `{flag}`"))
    };
    let before_code = native.exit_finalizers(before, true).unwrap();
    let after_code = native.exit_finalizers(after, true).unwrap();
    assert!(
        position(&before_code, &live_flag(before, 0))
            < position(&before_code, &live_flag(before, 1))
    );
    assert!(
        position(&after_code, &live_flag(after, 0)) < position(&after_code, &live_flag(after, 1))
    );
    // The common epilogue performs nothing and guards every terminal slot.
    let epilogue = native.epilogue();
    assert!(epilogue.starts_with("    if (spx_bytes_live_"), "{epilogue}");
    assert!(
        epilogue.ends_with(&format!(") spx_runtime_invariant_failure(\"{NONLEXICAL_GUARD}\");\n")),
        "{epilogue}"
    );
    assert_eq!(epilogue.lines().count(), 1);
    assert!(!epilogue.contains("drop"), "{epilogue}");
    for exit in [before, after, commit] {
        for action in &exit.finalize_in_order {
            let guarded = format!("spx_bytes_live_{}", action.guard_flag.0);
            assert!(
                epilogue
                    .split(['(', ')', ' ', '|'])
                    .any(|token| token == guarded),
                "{guarded}: {epilogue}"
            );
        }
    }

    let generated = crate::codegen::emit_c(&checked).expect("native emits the nonlexical owner");
    assert_eq!(generated.matches(NONLEXICAL_GUARD).count(), 1);
    assert_eq!(generated.matches(LEXICAL_TRAP).count(), 0);
    assert_adjacent_finalizers(&generated, &live_flag(before, 0), &live_flag(before, 1));
    assert_adjacent_finalizers(&generated, &live_flag(after, 0), &live_flag(after, 1));
}

fn assert_nonlexical_refusal(error: &Diagnostic, expected: &str) {
    assert_eq!(error.code, "SPX-B104", "{error:?}");
    for expected in [
        "contradictory finalizer precedence",
        "no lexical exit selection",
        "exact nonlexical routing refuses: ",
        expected,
    ] {
        assert!(error.message.contains(expected), "{expected}: {error:?}");
    }
}

/// Forged plan links on the actual checked nonlexical plan. Backend admission
/// re-authenticates every link itself rather than trusting earlier replay.
#[test]
fn hostile_forged_nonlexical_links_on_the_checked_plan_refuse() {
    let (_, program) = variant_owner_program(&nonlexical_variant_source());
    let run = run_function(&program);
    let before = call_failure(&run.cleanup_plan, &body_let(run, 11).1.id).clone();
    let commit = run.cleanup_plan.regions[0].normal_scope_end;
    let forge = |mutate: &dyn Fn(&mut CleanupPlan), expected: &str| {
        let mut forged = run.clone();
        mutate(&mut forged.cleanup_plan);
        assert_nonlexical_refusal(&NativeBytesPlan::build(&forged).unwrap_err(), expected);
    };
    let appended = |plan: &CleanupPlan, exit: &ExitTarget| {
        let mut extra = exit.clone();
        extra.id = ExitTargetId(u32::try_from(plan.exits.len()).unwrap());
        extra
    };
    forge(
        &|plan| plan.regions[0].normal_scope_end = before.id,
        "is not an ordinary success",
    );
    forge(
        &|plan| plan.regions[0].normal_scope_end = ExitTargetId(u32::MAX),
        "the root normal scope end names no exit",
    );
    forge(
        &|plan| {
            let extra = appended(&*plan, &plan.exits[commit.0 as usize]);
            plan.exits.push(extra);
        },
        "2 ordinary success exits exist, but only the root normal scope end may commit",
    );
    forge(
        &|plan| plan.exits[commit.0 as usize].leaves_regions.clear(),
        "does not leave exactly the root region",
    );
    forge(
        &|plan| {
            plan.blocks[before.from.0 as usize]
                .transitions
                .retain(|transition| !matches!(transition, CleanupTransition::SelectFailure { .. }))
        },
        "is not linked to its block's sole status selection",
    );
    forge(
        &|plan| {
            let extra = appended(&*plan, &before);
            plan.exits.push(extra);
        },
        "selects more than one failure exit",
    );
    forge(
        &|plan| {
            plan.entry_state.live_owned_parameters.push(CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: Vec::new(),
            })
        },
        "owned parameters are live at entry without a function-entry boundary certificate",
    );
}

/// Replay-shaped nonlexical plan: every exit terminates its own block, the
/// failure block selects its declared status source, and the root region's
/// normal scope end is the only commit.
fn nonlexical_plan(failed: &ExpressionId, result: &ExpressionId) -> CleanupPlan {
    let source = StatusSourceId {
        expression: failed.clone(),
        lane: StatusLane::OperationFailure,
    };
    let mut plan = plan(
        vec![
            exit(0, &[0, 1], failure(failed)),
            exit(1, &[1, 0], commit(result)),
        ],
        None,
    );
    plan.exits[1].leaves_regions = vec![CleanupRegionId(0)];
    plan.status_sources = vec![StatusSource {
        id: source.clone(),
        producer: StatusProducer::PropagatedCall {
            callee: DeclarationId::new("test.native_terminal_exits.probe"),
        },
    }];
    plan.blocks = vec![
        CleanupBlock {
            id: BlockId(0),
            region: CleanupRegionId(0),
            transitions: vec![CleanupTransition::SelectFailure { source }],
            terminator: CleanupTerminator::Exit(ExitTargetId(0)),
        },
        CleanupBlock {
            id: BlockId(1),
            region: CleanupRegionId(0),
            transitions: Vec::new(),
            terminator: CleanupTerminator::Exit(ExitTargetId(1)),
        },
    ];
    plan.regions = vec![CleanupRegion {
        id: CleanupRegionId(0),
        parent: None,
        slots: Vec::new(),
        normal_scope_end: ExitTargetId(1),
    }];
    plan
}

#[test]
fn authenticated_nonlexical_exits_keep_each_exact_contradictory_order() {
    let (function, failed, result) = function_ids();
    let plan = nonlexical_plan(&failed, &result);
    let order = terminal_order(&function, &plan, &flags()).unwrap();
    assert_eq!(order.mode, TerminalMode::Nonlexical);
    assert!(order.exit_selected);
    assert!(order.shared.is_empty());
    assert_eq!(order.slots, BTreeSet::from([slot("a"), slot("b")]));
    let ExitContinuation::ReturnFailure { source } = &plan.exits[0].continuation else {
        panic!("exit 0 is a failure");
    };
    assert!(std::ptr::eq(
        nonlexical_failure_exit(&plan, source).unwrap(),
        &plan.exits[0]
    ));
    assert!(std::ptr::eq(
        nonlexical_success_exit(&plan).unwrap(),
        &plan.exits[1]
    ));
    assert_eq!(plan.exits[0].finalize_in_order[0].guard_flag, LivenessFlagId(0));
    assert_eq!(plan.exits[1].finalize_in_order[0].guard_flag, LivenessFlagId(1));
}

#[test]
fn hostile_forged_synthetic_nonlexical_links_refuse() {
    let (function, failed, result) = function_ids();
    let refuse = |mutate: &dyn Fn(&mut CleanupPlan), expected: &str| {
        let mut plan = nonlexical_plan(&failed, &result);
        mutate(&mut plan);
        let error = terminal_order(&function, &plan, &flags()).unwrap_err();
        assert_nonlexical_refusal(&error, expected);
    };
    refuse(
        &|plan| plan.status_sources.clear(),
        "is not exactly one declared status source",
    );
    refuse(
        &|plan| {
            let selected = plan.blocks[0].transitions[0].clone();
            plan.blocks[0].transitions.push(selected);
        },
        "is not linked to its block's sole status selection",
    );
    refuse(
        &|plan| plan.blocks[0].terminator = CleanupTerminator::Exit(ExitTargetId(1)),
        "does not terminate its source block",
    );
    refuse(
        &|plan| plan.exits[0].id = ExitTargetId(7),
        "is not at its canonical index",
    );
    refuse(
        &|plan| {
            plan.regions.push(CleanupRegion {
                id: CleanupRegionId(1),
                parent: None,
                slots: Vec::new(),
                normal_scope_end: ExitTargetId(1),
            })
        },
        "the root cleanup region is not unique",
    );
    refuse(
        &|plan| plan.regions.clear(),
        "the root cleanup region is absent",
    );
    refuse(
        &|plan| {
            plan.entry_state
                .conditional_owned_parameters
                .push(ConditionalVariantEntry {
                    storage: StorageId::ProvisionalResult,
                    variant: DeclarationId::new("test.native_terminal_exits.Choice"),
                    cases: Vec::new(),
                })
        },
        "owned parameters are live at entry",
    );
    refuse(
        &|plan| {
            let mut outside = plan.exits[0].finalize_in_order[0].clone();
            outside.guard_flag = LivenessFlagId(9);
            plan.exits[0].finalize_in_order.push(outside);
        },
        "finalizes liveness flag 9 outside the physical Bytes plan",
    );
    // A plain loop has no iteration certificate, independent of its links.
    let plan = nonlexical_plan(&failed, &result);
    let error = terminal_order_with(&function, &plan, &flags(), || true).unwrap_err();
    assert_nonlexical_refusal(&error, "a plain `while` loop has no iteration execution-boundary certificate");
}

#[test]
fn nonlexical_common_epilogue_guards_live_terminal_slots_without_guessing() {
    let (_, program) = variant_owner_program(&nonlexical_variant_source());
    let mut plan = NativeBytesPlan::build(run_function(&program))
        .unwrap()
        .expect("run owns Bytes slots");
    assert!(plan.exit_selected_terminal);
    plan.nonlexical_terminal = Some(vec![slot("a"), slot("b")]);
    assert_eq!(
        plan.selected_epilogue(),
        format!("    if (flag_a || flag_b) spx_runtime_invariant_failure(\"{NONLEXICAL_GUARD}\");\n")
    );
    plan.nonlexical_terminal = Some(Vec::new());
    assert_eq!(plan.selected_epilogue(), "", "the empty state performs nothing");
    plan.nonlexical_terminal = None;
    assert_eq!(
        plan.selected_epilogue(),
        format!("    spx_runtime_invariant_failure(\"{LEXICAL_TRAP}\");\n")
    );
}

const NONLEXICAL_ENTRY_OWNER: &str = include_str!("nonlexical_entry_owner_order.spx");
const NONLEXICAL_LOOP: &str = include_str!("nonlexical_loop_order.spx");

fn checked_nonlexical_run(
    source: &str,
    run: &str,
) -> (crate::ast::Program, crate::hir::ResolvedProgram, DeclarationId) {
    let checked = crate::check(source, std::path::Path::new("nonlexical-refusal.spx"))
        .expect("nonlexical refusal source checks");
    let program = crate::hir::resolve(&checked).expect("nonlexical refusal source resolves");
    crate::hir::validate(&program).expect("independent replay admits the plan");
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == run)
        .expect("run is declared");
    assert!(function.cleanup_plan.lexical_control.is_none());
    let id = function.id.clone();
    (checked, program, id)
}

/// `label` is live at entry and the plan has no entry boundary certificate.
#[test]
fn checked_nonlexical_live_entry_owner_refuses() {
    let (checked, program, id) = checked_nonlexical_run(
        NONLEXICAL_ENTRY_OWNER,
        "test.native_terminal_nonlexical_entry.run",
    );
    let run = program.functions.iter().find(|f| f.id == id).unwrap();
    assert!(!run
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .is_empty());
    assert_nonlexical_refusal(
        &NativeBytesPlan::build(run).unwrap_err(),
        "owned parameters are live at entry without a function-entry boundary certificate",
    );
    assert_eq!(crate::codegen::emit_c(&checked).unwrap_err().code, "SPX-B104");
}

/// The plain loop has no iteration certificate: ordinary and metered native
/// emission both refuse before any artifact exists.
#[test]
fn checked_nonlexical_plain_loop_refuses_on_ordinary_and_metered_emission() {
    let (checked, program, id) =
        checked_nonlexical_run(NONLEXICAL_LOOP, "test.native_terminal_nonlexical_loop.run");
    let run = program.functions.iter().find(|f| f.id == id).unwrap();
    assert_nonlexical_refusal(
        &NativeBytesPlan::build(run).unwrap_err(),
        "a plain `while` loop has no iteration execution-boundary certificate",
    );
    assert_eq!(crate::codegen::emit_c(&checked).unwrap_err().code, "SPX-B104");
    let metering = crate::codegen::NativeSemanticMetering {
        fuel_limit: 1_000,
        functions: BTreeMap::from([(id, 0)]),
    };
    let error = crate::codegen::emit_hir_c_semantically_metered(&program, &metering).unwrap_err();
    assert_nonlexical_refusal(
        &error,
        "a plain `while` loop has no iteration execution-boundary certificate",
    );
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
