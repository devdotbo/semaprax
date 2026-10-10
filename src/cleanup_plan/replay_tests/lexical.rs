//! Cleanup-plan v18 replay: lexical `return`, `break` and `continue` certificates.
//!
//! The parser does not yet spell lexical exits, so each fixture resolves an
//! ordinary bounded `while`, then installs the typed HIR a lexical source would
//! resolve to: a `LexicalWhile` with a guarded exit statement. Plans are built
//! for the authenticated execution identity and replayed independently.
use super::*;
use crate::ast::{BinaryOp, LexicalExitKind};
use crate::cleanup_plan::{build_plan_for_execution, ExitTargetId, CLEANUP_PLAN_SCHEMA_V18};
use crate::hir::{
    ControlTarget, ExpressionId, FunctionExecutionId, LoopLatchKind, OwnershipMode, Place,
    ResolvedBinding, ResolvedType, ValueId,
};

const LEXICAL_SOURCE: &str = r#"module test.lexical_replay;

@id("lex.count")
fn count(n: i64) -> i64 {
    let mut i = 0;
    while i < n {
        i = i + 1;
    }
    i
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[derive(Clone, Copy)]
enum Injection {
    Return,
    Break,
    Continue,
}

const OWNED_SOURCE: &str = r#"module test.lexical_owned;

@id("lex.token")
resource Token {
    @id("lex.token.drop")
    drop trivial;
}

@id("lex.hold")
fn hold(n: i64, token: own Token) -> Token {
    let mut i = 0;
    while i < n {
        i = i + 1;
    }
    token
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A resolved program whose `lex.count` loop is a lexical loop with one guarded
/// exit, and the function carrying its freshly built v18 plan.
fn fixture(injection: Injection) -> (ResolvedProgram, ResolvedFunction) {
    let parsed = parse(LEXICAL_SOURCE, Path::new("cleanup-replay-lexical.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let func = function(&program, "lex.count");
    install(program, func, injection)
}

/// The same guarded exit over an owned result: a returned `Token` moves into
/// provisional storage at the exit site before any contour cleanup.
fn owned_fixture() -> (ResolvedProgram, ResolvedFunction) {
    let parsed = parse(OWNED_SOURCE, Path::new("cleanup-replay-lexical-owned.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let func = function(&program, "lex.hold");
    install(program, func, Injection::Return)
}

fn install(
    program: ResolvedProgram,
    mut func: ResolvedFunction,
    injection: Injection,
) -> (ResolvedProgram, ResolvedFunction) {
    let execution = FunctionExecutionId::Monomorphic(func.id.clone());
    let ResolvedExprKind::Block { statements, tail } = &mut func.body.kind else {
        panic!("fixture body is a block")
    };
    // A returned operand repeats the function's own tail value.
    let operand_template = (**tail).clone();
    let ResolvedStatement::Let { binding, span, .. } = &statements[0] else {
        panic!("fixture starts with the counter binding")
    };
    let (binding, span) = (binding.clone(), *span);
    let ResolvedStatement::While {
        condition,
        mut body,
        ..
    } = statements.remove(1)
    else {
        panic!("fixture has a while statement")
    };

    let id = |path: &str| ExpressionId::new(&execution, path);
    let counter = |path: &str| ResolvedExpr {
        id: id(path),
        ty: ResolvedType::I64,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Place(Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        }),
        span,
    };
    let literal = |path: &str, value: i64| ResolvedExpr {
        id: id(path),
        ty: ResolvedType::I64,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Int(value),
        span,
    };
    let (kind, target, value) = match injection {
        Injection::Return => (
            LexicalExitKind::Return,
            ControlTarget::Function(execution.clone()),
            Some(Box::new(ResolvedExpr {
                id: id("lex.exit.operand"),
                ..operand_template
            })),
        ),
        Injection::Break | Injection::Continue => (
            if matches!(injection, Injection::Break) {
                LexicalExitKind::Break
            } else {
                LexicalExitKind::Continue
            },
            ControlTarget::Loop {
                owner: execution.clone(),
                site: condition.id.clone(),
            },
            None,
        ),
    };
    let exit = ResolvedExpr {
        id: id("lex.exit"),
        ty: ResolvedType::Never,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        },
        span,
    };
    let guard_condition = ResolvedExpr {
        id: id("lex.guard.condition"),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Binary {
            op: BinaryOp::Eq,
            left: Box::new(counter("lex.guard.left")),
            right: Box::new(literal("lex.guard.right", 3)),
        },
        span,
    };
    let guarded = ResolvedExpr {
        id: id("lex.guard"),
        ty: ResolvedType::I64,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::If {
            condition: Box::new(guard_condition),
            then_branch: Box::new(exit),
            else_branch: Box::new(literal("lex.guard.else", 0)),
        },
        span,
    };
    let guard = ResolvedStatement::Let {
        binding: ResolvedBinding {
            id: ValueId::local(&execution, "lex.guard.binding"),
            name: "guard".to_owned(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::I64,
            span,
        },
        mutable: false,
        value: guarded,
        span,
    };
    let ResolvedExprKind::Block {
        statements: loop_statements,
        ..
    } = &mut body.kind
    else {
        panic!("while body is a block")
    };
    loop_statements.insert(0, guard);
    statements.insert(
        1,
        ResolvedStatement::LexicalWhile {
            condition,
            body,
            latch: LoopLatchKind::Condition,
            span,
        },
    );
    func.cleanup_plan = build_plan_for_execution(&program, &func, &execution)
        .expect("lexical fixture plan builds for its authenticated execution");
    (program, func)
}

fn control(func: &mut ResolvedFunction) -> &mut crate::cleanup_plan::LexicalControlPlan {
    func.cleanup_plan
        .lexical_control
        .as_deref_mut()
        .expect("v18 fixture carries a lexical-control certificate")
}

fn assert_rejected(program: &ResolvedProgram, func: &ResolvedFunction, needle: &str) {
    let diagnostic = validate_structure(program, func).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(
        diagnostic.message.contains("failed independent replay"),
        "{diagnostic:?}"
    );
    assert!(diagnostic.message.contains(needle), "{diagnostic:?}");
}

#[test]
fn lexical_return_certificate_replays_with_per_return_success_domain() {
    let (program, func) = fixture(Injection::Return);
    assert_eq!(func.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V18);
    validate_structure(&program, &func).expect("canonical lexical return plan replays");
    let control = func.cleanup_plan.lexical_control.as_deref().unwrap();
    assert_eq!(control.loops.len(), 1);
    assert_eq!(control.exits.len(), 1);
    assert_eq!(control.resumes.len(), 1);
    // The ordinary completion and the one return each publish exactly once.
    assert_eq!(control.function_success_exits.len(), 2);
}

#[test]
fn lexical_break_and_continue_certificates_replay_without_a_back_edge() {
    for injection in [Injection::Break, Injection::Continue] {
        let (program, func) = fixture(injection);
        validate_structure(&program, &func).expect("canonical loop exit plan replays");
        let control = func.cleanup_plan.lexical_control.as_deref().unwrap();
        assert_eq!(control.exits.len(), 1);
        assert!(control.resumes.is_empty());
        assert_eq!(control.function_success_exits.len(), 1);
        // Latches are terminal exits: the proof CFG has no back edge.
        validate_reachable_acyclic_cfg(&func).expect("lexical proof CFG is acyclic");
    }
}

#[test]
fn forged_exit_target_is_rejected() {
    let (program, mut func) = fixture(Injection::Return);
    let site = func.body.id.clone();
    control(&mut func).exits[0].target = ControlTarget::Loop {
        owner: FunctionExecutionId::Monomorphic(func.id.clone()),
        site,
    };
    assert_rejected(&program, &func, "exit certificate");
}

#[test]
fn forged_exit_kind_is_rejected() {
    let (program, mut func) = fixture(Injection::Break);
    control(&mut func).exits[0].kind = LexicalExitKind::Continue;
    assert_rejected(&program, &func, "exit certificate");
}

#[test]
fn forged_loop_condition_and_latch_are_rejected() {
    let (program, mut func) = fixture(Injection::Break);
    let other = func.body.id.clone();
    control(&mut func).loops[0].condition = other;
    assert_independent_replay_rejects(&program, &func);

    let (program, mut func) = fixture(Injection::Break);
    let step = func.body.id.clone();
    control(&mut func).loops[0].latch = LoopLatchKind::SourceForStep {
        kind: crate::hir::SourceForKind::Copy,
        step,
    };
    assert_rejected(&program, &func, "loop certificate");
}

#[test]
fn forged_contour_resume_inventory_is_rejected() {
    let (program, mut func) = fixture(Injection::Return);
    let entry = func.cleanup_plan.entry;
    control(&mut func).resumes[0].blocks.push(entry);
    assert_rejected(&program, &func, "resume certificate");

    let (program, mut func) = fixture(Injection::Return);
    control(&mut func).resumes[0].blocks.clear();
    assert_rejected(&program, &func, "resume");
}

#[test]
fn forged_success_exit_order_is_rejected() {
    let (program, mut func) = fixture(Injection::Return);
    control(&mut func).function_success_exits.reverse();
    assert_rejected(&program, &func, "success exits");

    let (program, mut func) = fixture(Injection::Return);
    control(&mut func).function_success_exits.pop();
    assert_rejected(&program, &func, "success exits");
}

#[test]
fn abrupt_iteration_sentinel_is_rejected_for_a_completing_loop() {
    let (program, mut func) = fixture(Injection::Break);
    control(&mut func).loops[0].iteration_end = ExitTargetId(u32::MAX);
    assert_rejected(&program, &func, "iteration");
}

#[test]
fn provisional_region_end_requires_the_abrupt_only_certificate() {
    let (program, mut func) = fixture(Injection::Return);
    let region = func
        .cleanup_plan
        .regions
        .iter_mut()
        .skip(1)
        .find(|region| region.normal_scope_end != ExitTargetId(u32::MAX))
        .expect("fixture has a region with a real normal scope end");
    region.normal_scope_end = ExitTargetId(u32::MAX);
    assert_rejected(&program, &func, "abrupt-only");

    let (program, mut func) = fixture(Injection::Return);
    control(&mut func)
        .abrupt_only_regions
        .push(CleanupRegionId(1));
    assert_rejected(&program, &func, "abrupt-only");
}

#[test]
fn forged_return_result_source_is_rejected() {
    let (program, mut func) = fixture(Injection::Return);
    let body = func.body.id.clone();
    let source = func
        .cleanup_plan
        .exits
        .iter_mut()
        .find_map(|exit| match &mut exit.continuation {
            ExitContinuation::FunctionReturn { source, .. } => Some(source),
            _ => None,
        })
        .expect("fixture has a function return exit");
    *source = CleanupResultSource::Scalar { expression: body };
    assert_rejected(&program, &func, "function return");
}

#[test]
fn return_domain_cannot_publish_another_exits_result() {
    let (program, mut func) = fixture(Injection::Return);
    let body = func.body.id.clone();
    let commit = func
        .cleanup_plan
        .exits
        .iter_mut()
        .rev()
        .find_map(|exit| match &mut exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source),
            _ => None,
        })
        .expect("the return domain ends in a result commit");
    *commit = CleanupResultSource::Scalar { expression: body };
    assert_independent_replay_rejects(&program, &func);
}

#[test]
fn continue_resume_into_the_plan_entry_is_a_rejected_proof_cycle() {
    let (program, mut func) = fixture(Injection::Continue);
    let entry = func.cleanup_plan.entry;
    control(&mut func).exits[0].resume = Some(entry);
    assert_independent_replay_rejects(&program, &func);
}

#[test]
fn legacy_function_cannot_carry_lexical_control_or_the_v18_schema() {
    let (program, lexical) = fixture(Injection::Break);
    let mut legacy = function(&program, "lex.count");
    legacy.cleanup_plan.lexical_control = lexical.cleanup_plan.lexical_control.clone();
    assert_rejected(&program, &legacy, "lexical-control certificate");

    let mut legacy = function(&program, "lex.count");
    legacy.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V18;
    assert_rejected(&program, &legacy, "schema");
}

#[test]
fn owned_return_stages_its_result_at_the_exit_site_and_replays() {
    let (program, func) = owned_fixture();
    validate_structure(&program, &func).expect("owned lexical return plan replays");
    let control = func.cleanup_plan.lexical_control.as_deref().unwrap();
    let site = control.exits[0].site.clone();
    assert!(func.cleanup_plan.exits.iter().any(|exit| matches!(
        &exit.continuation,
        ExitContinuation::FunctionReturn {
            source: CleanupResultSource::Owned { .. },
            ..
        }
    )));
    // The owned result moves into provisional storage at the exit site itself,
    // before the exit's contour cleanup runs.
    assert!(func
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| matches!(
            transition,
            CleanupTransition::Transfer { at, destination, .. }
                if at == &site && destination.storage == StorageId::ProvisionalResult
        )));
}

#[test]
fn owned_return_cannot_drop_its_staging_transfer_or_downgrade_to_scalar() {
    let (program, mut func) = owned_fixture();
    let site = func.cleanup_plan.lexical_control.as_deref().unwrap().exits[0]
        .site
        .clone();
    for block in &mut func.cleanup_plan.blocks {
        block.transitions.retain(|transition| {
            !matches!(transition, CleanupTransition::Transfer { at, .. } if at == &site)
        });
    }
    assert_independent_replay_rejects(&program, &func);

    let (program, mut func) = owned_fixture();
    let body = func.body.id.clone();
    let source = func
        .cleanup_plan
        .exits
        .iter_mut()
        .find_map(|exit| match &mut exit.continuation {
            ExitContinuation::FunctionReturn { source, .. } => Some(source),
            _ => None,
        })
        .expect("fixture has a function return exit");
    *source = CleanupResultSource::Scalar { expression: body };
    assert_independent_replay_rejects(&program, &func);
}
