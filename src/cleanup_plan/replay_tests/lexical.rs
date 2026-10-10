//! Cleanup-plan v18 replay: lexical `return`, `break` and `continue` certificates,
//! per-exit generated-advancement fragments, owning wrapper breaks, execution
//! boundaries and terminal postfix `?` residual paths.
//!
//! Every fixture is authored source resolved by the ordinary pipeline; the plan
//! under test is the one attached at resolution. Hostile cases mutate that plan
//! (or its certificates) and must be refused by independent replay.
use super::*;
use crate::ast::LexicalExitKind;
use crate::cleanup_plan::{ExecutionBoundaryKind, ExitTargetId, CLEANUP_PLAN_SCHEMA_V18};
use crate::hir::ControlTarget;

const SCALAR: &str = r#"module test.lexical_scalar;

@id("lex.count")
fn count(n: i64) -> i64 {
    let mut i = 0;
    let mut total = 0;
    while i < n {
        i = i + 1;
        if i == 2 { continue; };
        if i == 3 { continue; };
        if i == 5 { break; };
        total = total + i;
        0
    }
    if total == 99 { return 1; };
    return total;
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const COPY_FOR: &str = r#"module test.lexical_copy_for;

@id("lex.copy")
fn copy(limit: i64) -> i64 {
    let mut building = vec_with_capacity<i64>(4usize);
    building = vec_push<i64>(building, 1);
    building = vec_push<i64>(building, 2);
    building = vec_push<i64>(building, 3);
    let values = building;
    let mut total = 0;
    for item in values {
        if item == 2 { continue; };
        if item == 3 { continue; };
        if item == limit { break; };
        total = total + item;
        0
    }
    return total;
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const OWNING_FOR: &str = r#"module test.lexical_owning_for;

@id("lex.sum")
fn sum(values: own Vec<Bytes>, stop: usize) -> i64 {
    let mut total = 0;
    for own item in vec_into_iter<Bytes>(values) {
        let length = byte_len(bytes_as_slice(item));
        if length == 2usize { continue; };
        if length == stop { break; };
        total = total + 1;
        0
    }
    return total;
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const OWNED_RESULT: &str = r#"module test.lexical_owned_result;

@id("lex.token")
resource Token {
    @id("lex.token.drop")
    drop trivial;
}

@id("lex.hold")
fn hold(n: i64, token: own Token) -> Token
    ensures string_len("ok") == 2
{
    let mut i = 0;
    while i < n {
        i = i + 1;
        if i == 3 { return token; };
        0
    }
    return token;
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const TERMINAL_TRY: &str = r#"module test.lexical_try;

@id("lex.try")
fn forward(value: Result<i64, bool>, n: i64) -> Result<bool, bool>
    ensures true
{
    let mut i = 0;
    while i < n {
        i = i + 1;
        if i == 2 { return Result<bool, bool>::Ok { value: false }; };
        0
    }
    let number = value?;
    return Result<bool, bool>::Ok { value: true };
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn fixture(source: &str, id: &str) -> (ResolvedProgram, ResolvedFunction) {
    // Source admission precedes HIR/proof construction: guessed syntax or a
    // closed source profile must fail here, not acquire authority through HIR.
    let parsed = crate::check(source, Path::new("cleanup-replay-lexical.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let func = function(&program, id);
    assert_eq!(func.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V18);
    validate_structure(&program, &func).expect("canonical lexical plan replays");
    (program, func)
}

fn control(func: &mut ResolvedFunction) -> &mut crate::cleanup_plan::LexicalControlPlan {
    func.cleanup_plan
        .lexical_control
        .as_deref_mut()
        .expect("v18 fixture carries a lexical-control certificate")
}

fn certificate(func: &ResolvedFunction) -> &crate::cleanup_plan::LexicalControlPlan {
    func.cleanup_plan.lexical_control.as_deref().unwrap()
}

fn assert_rejected(program: &ResolvedProgram, func: &ResolvedFunction) {
    let diagnostic = validate_structure(program, func).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(
        diagnostic.message.contains("failed independent replay"),
        "{diagnostic:?}"
    );
}

fn kinds(func: &ResolvedFunction, kind: LexicalExitKind) -> Vec<usize> {
    certificate(func)
        .exits
        .iter()
        .enumerate()
        .filter(|(_, exit)| exit.kind == kind)
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn scalar_loop_with_two_continues_break_and_returns_replays_independently() {
    let (_, func) = fixture(SCALAR, "lex.count");
    let control = certificate(&func);
    assert_eq!(kinds(&func, LexicalExitKind::Continue).len(), 2);
    assert_eq!(kinds(&func, LexicalExitKind::Break).len(), 1);
    assert_eq!(kinds(&func, LexicalExitKind::Return).len(), 2);
    // An ordinary `while` latch needs no generated advancement fragment.
    assert!(control.loops.iter().all(|lp| lp.latch_entry.is_none()));
    // The body ends in a return, so only the two returns publish a success.
    assert_eq!(control.function_success_exits.len(), 2);
    // Function entry plus one iteration boundary for the one loop.
    assert_eq!(control.execution_boundaries.len(), 2);
    assert_eq!(
        control.execution_boundaries[0].kind,
        ExecutionBoundaryKind::FunctionEntry
    );
    assert_eq!(
        control.execution_boundaries[1].kind,
        ExecutionBoundaryKind::LoopIteration
    );
}

#[test]
fn each_copy_for_continue_owns_one_separate_advancement_domain() {
    let (_, func) = fixture(COPY_FOR, "lex.copy");
    let control = certificate(&func);
    let continues = kinds(&func, LexicalExitKind::Continue);
    assert_eq!(continues.len(), 2);
    let entries = continues
        .iter()
        .map(|index| control.exits[*index].resume.expect("generated fragment"))
        .collect::<BTreeSet<_>>();
    assert_eq!(entries.len(), 2, "continue exits never share a fragment");
    assert!(control.loops.iter().all(|lp| lp.latch_entry.is_none()));
    for entry in entries {
        assert!(control.resumes.iter().any(|resume| resume.entry == entry));
    }
}

#[test]
fn owning_for_continue_retains_remainder_and_break_completes_its_wrapper() {
    let (_, func) = fixture(OWNING_FOR, "lex.sum");
    let control = certificate(&func);
    let continue_entry = control.exits[kinds(&func, LexicalExitKind::Continue)[0]]
        .resume
        .expect("owning continue advances through a fragment");
    let break_certificate = &control.exits[kinds(&func, LexicalExitKind::Break)[0]];
    let break_entry = break_certificate
        .resume
        .expect("owning break completes its wrapper in a fragment");
    assert_ne!(continue_entry, break_entry);
    let ExitContinuation::LoopBreak { edge, .. } =
        &func.cleanup_plan.exits[break_certificate.exit.0 as usize].continuation
    else {
        panic!("break exit is a loop break")
    };
    // The break enters its wrapper fragment, never the loop's `after` block.
    assert_eq!(func.cleanup_plan.edges[edge.0 as usize].to, break_entry);
    assert_ne!(break_entry, control.loops[0].after);
}

#[test]
fn owned_result_returns_run_exact_per_return_postconditions() {
    let (_, func) = fixture(OWNED_RESULT, "lex.hold");
    let control = certificate(&func);
    let returns = kinds(&func, LexicalExitKind::Return);
    assert_eq!(returns.len(), 2);
    let entries = returns
        .iter()
        .map(|index| control.exits[*index].resume.unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(entries.len(), 2);
    assert_eq!(control.function_success_exits.len(), 2);
    assert!(func.cleanup_plan.exits.iter().any(|exit| matches!(
        &exit.continuation,
        ExitContinuation::FunctionReturn {
            source: CleanupResultSource::Owned { .. },
            ..
        }
    )));
}

#[test]
fn lexical_return_and_terminal_try_residual_share_one_ordinary_epilogue() {
    let (_, func) = fixture(TERMINAL_TRY, "lex.try");
    let control = certificate(&func);
    assert_eq!(kinds(&func, LexicalExitKind::Return).len(), 2);
    // The residual path and the normal path publish through one ordinary exit.
    assert_eq!(
        control.function_success_exits.len(),
        1 + kinds(&func, LexicalExitKind::Return).len()
    );
}

#[test]
fn forged_exit_target_kind_and_site_are_rejected() {
    let (program, mut func) = fixture(SCALAR, "lex.count");
    let mut swapped = func.clone();
    let continues = kinds(&func, LexicalExitKind::Continue);
    let first = control(&mut swapped).exits[continues[0]].target.clone();
    control(&mut swapped).exits[continues[0]].target = ControlTarget::Function(
        crate::hir::FunctionExecutionId::Monomorphic(func.id.clone()),
    );
    assert_ne!(first, control(&mut swapped).exits[continues[0]].target);
    assert_rejected(&program, &swapped);

    control(&mut func).exits[continues[0]].kind = LexicalExitKind::Break;
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(SCALAR, "lex.count");
    let other = func.body.id.clone();
    control(&mut func).exits[0].site = other;
    assert_rejected(&program, &func);
}

#[test]
fn shared_or_forged_continue_resume_is_rejected() {
    let (program, mut func) = fixture(COPY_FOR, "lex.copy");
    let continues = kinds(&func, LexicalExitKind::Continue);
    let first = control(&mut func).exits[continues[0]].resume;
    control(&mut func).exits[continues[1]].resume = first;
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(COPY_FOR, "lex.copy");
    let continues = kinds(&func, LexicalExitKind::Continue);
    let entry = func.cleanup_plan.entry;
    control(&mut func).exits[continues[0]].resume = Some(entry);
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(COPY_FOR, "lex.copy");
    control(&mut func).loops[0].latch_entry = control(&mut func).resumes[0].blocks.first().copied();
    assert_rejected(&program, &func);
}

#[test]
fn forged_resume_inventory_and_wrapper_tail_are_rejected() {
    let (program, mut func) = fixture(OWNING_FOR, "lex.sum");
    let entry = control(&mut func).exits[kinds(&func, LexicalExitKind::Break)[0]]
        .resume
        .unwrap();
    let resume = control(&mut func)
        .resumes
        .iter_mut()
        .find(|resume| resume.entry == entry)
        .unwrap();
    // Dropping the wrapper's final (join) block leaves the fragment unfinished.
    resume.blocks.pop();
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(OWNING_FOR, "lex.sum");
    let entry = func.cleanup_plan.entry;
    control(&mut func).resumes[0].blocks.push(entry);
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(OWNING_FOR, "lex.sum");
    let break_index = kinds(&func, LexicalExitKind::Break)[0];
    control(&mut func).exits[break_index].resume = None;
    assert_rejected(&program, &func);
}

#[test]
fn owning_break_cannot_join_the_loop_continuation_or_skip_the_exact_join() {
    let (program, mut func) = fixture(OWNING_FOR, "lex.sum");
    let after = control(&mut func).loops[0].after;
    let exit = control(&mut func).exits[kinds(&func, LexicalExitKind::Break)[0]].exit;
    let ExitContinuation::LoopBreak { edge, .. } = func.cleanup_plan.exits[exit.0 as usize]
        .continuation
        .clone()
    else {
        panic!("break exit is a loop break")
    };
    func.cleanup_plan.edges[edge.0 as usize].to = after;
    assert_rejected(&program, &func);
}

#[test]
fn forged_or_missing_execution_boundaries_are_rejected() {
    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    control(&mut func).execution_boundaries.pop();
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    let duplicate = certificate(&func).execution_boundaries[1].clone();
    control(&mut func).execution_boundaries.push(duplicate);
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    control(&mut func).execution_boundaries[1].kind = ExecutionBoundaryKind::FunctionEntry;
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    let other = func.body.id.clone();
    control(&mut func).execution_boundaries[1].site = other;
    assert_rejected(&program, &func);

    // An out-of-domain boundary: the loop-iteration boundary moved into a resume.
    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    let resume = certificate(&func).resumes[0].entry;
    control(&mut func).execution_boundaries[1].block = resume;
    assert_rejected(&program, &func);
}

#[test]
fn forged_boundary_finalizer_contour_and_order_are_rejected() {
    // The owned parameter is live at function entry: its finalizer is mandatory.
    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    assert!(!certificate(&func).execution_boundaries[0]
        .finalize_in_order
        .is_empty());
    control(&mut func).execution_boundaries[0]
        .finalize_in_order
        .clear();
    assert_rejected(&program, &func);

    // A repeated finalizer would release the same leaf twice at a guard failure.
    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    let first = certificate(&func).execution_boundaries[0].finalize_in_order[0].clone();
    control(&mut func).execution_boundaries[0]
        .finalize_in_order
        .push(first);
    assert_rejected(&program, &func);

    // A truncated region chain hides outer scopes from the guard contour.
    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
    control(&mut func).execution_boundaries[1]
        .leaves_regions
        .pop();
    assert_rejected(&program, &func);
}

#[test]
fn forged_return_result_source_and_domain_commit_are_rejected() {
    let (program, mut func) = fixture(OWNED_RESULT, "lex.hold");
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
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(SCALAR, "lex.count");
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
        .expect("a return domain ends in a result commit");
    *commit = CleanupResultSource::Scalar { expression: body };
    assert_rejected(&program, &func);
}

#[test]
fn abrupt_iteration_sentinel_and_provisional_regions_are_independently_proven() {
    let (program, mut func) = fixture(SCALAR, "lex.count");
    control(&mut func).loops[0].iteration_end = ExitTargetId(u32::MAX);
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(SCALAR, "lex.count");
    let region = func
        .cleanup_plan
        .regions
        .iter_mut()
        .skip(1)
        .find(|region| region.normal_scope_end != ExitTargetId(u32::MAX))
        .expect("fixture has a region with a real normal scope end");
    region.normal_scope_end = ExitTargetId(u32::MAX);
    assert_rejected(&program, &func);

    let (program, mut func) = fixture(SCALAR, "lex.count");
    control(&mut func)
        .abrupt_only_regions
        .push(CleanupRegionId(1));
    assert_rejected(&program, &func);
}

#[test]
fn success_exit_order_and_legacy_substitution_are_rejected() {
    let (program, mut func) = fixture(SCALAR, "lex.count");
    control(&mut func).function_success_exits.reverse();
    assert_rejected(&program, &func);

    let (program, lexical) = fixture(SCALAR, "lex.count");
    let mut legacy = function(&program, "app.main");
    legacy.cleanup_plan.lexical_control = lexical.cleanup_plan.lexical_control.clone();
    assert_rejected(&program, &legacy);

    let mut legacy = function(&program, "app.main");
    legacy.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V18;
    assert_rejected(&program, &legacy);
}
