use super::*;
const SOURCE: &str = r#"module test.scalar_stack;
@id("scalar.label") fn label(key:i64,flag:bool,divisor:i64)->string {
 match key {
  0 if (match flag { true=>10/divisor>0, _=>false, }) =>
   match key { 0=>match flag {true=>"yes",_=>"no",}, _=>"other", },
  _=>"fallback",
 }
}
@id("scalar.main") fn main()->i64 {0}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "scalar-stack.spx").unwrap()).unwrap()
}
fn function(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "scalar.label")
        .unwrap()
}
#[test]
fn nested_scalar_guards_and_string_values_preserve_decisions_failure_and_owned_transfers() {
    let program = program();
    let function = function(&program);
    crate::hir::validate(&program).unwrap();
    let paths = hir_skeleton_paths(&program, function, &mut ReplayBudget::new()).unwrap();
    assert!(paths
        .iter()
        .any(|path| path.terminal == SkeletonTerminal::Failure));
    assert!(paths
        .iter()
        .any(|path| path.terminal == SkeletonTerminal::Success));
    for selected in [true, false] {
        assert!(paths.iter().any(|path|path.observations.iter().any(|observation|
            matches!(observation.as_ref(),SkeletonObservation::ArmSelected{selected:value,..} if *value==selected))));
    }
    for path in &paths {
        if let Some(failure) = path.observations.iter().position(|observation| {
            matches!(
                observation.as_ref(),
                SkeletonObservation::Status { success: false, .. }
            )
        }) {
            assert_eq!(path.terminal, SkeletonTerminal::Failure);
            assert!(
                !path
                    .observations
                    .iter()
                    .skip(failure + 1)
                    .any(|observation| matches!(
                        observation.as_ref(),
                        SkeletonObservation::Initialize { .. }
                            | SkeletonObservation::Transfer { .. }
                    )),
                "a failed guard cannot initialize an arm result or publish it"
            );
        }
    }
    assert!(paths.iter().any(|path|path.terminal==SkeletonTerminal::Success && path.observations.iter().any(|observation|
        matches!(observation.as_ref(),SkeletonObservation::Transfer{destination,..} if destination.storage==StorageId::ProvisionalResult))));
    let canonical = crate::format::canonical(&crate::parse(SOURCE, "scalar-stack.spx").unwrap());
    assert_eq!(
        crate::format::canonical(&crate::parse(&canonical, "scalar-stack.spx").unwrap()),
        canonical
    );
}
#[test]
fn nested_scalar_continuations_keep_exact_and_one_short_work_debits() {
    let program = program();
    let function = function(&program);
    let mut measured = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let expected = expression_skeleton(
        &program,
        function,
        &function.body,
        &mut SkeletonWork::new(&program, function, &mut measured),
    )
    .unwrap();
    let required = MAX_REPLAY_WORK_UNITS - measured.skeleton_remaining;
    assert!(required > 0 && required < MAX_REPLAY_WORK_UNITS);
    let mut exact = ReplayBudget::with_skeleton_limit(required);
    let actual = expression_skeleton(
        &program,
        function,
        &function.body,
        &mut SkeletonWork::new(&program, function, &mut exact),
    )
    .unwrap();
    assert_eq!(actual.len(), expected.len());
    assert_eq!(exact.skeleton_remaining, 0);
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_eq!(actual.observations, expected.observations);
        assert_eq!(actual.owned_source, expected.owned_source);
        assert_eq!(
            (actual.failed, actual.residual),
            (expected.failed, expected.residual)
        );
    }
    let mut short = ReplayBudget::with_skeleton_limit(required - 1);
    let error = expression_skeleton(
        &program,
        function,
        &function.body,
        &mut SkeletonWork::new(&program, function, &mut short),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(short.skeleton_remaining, 0);
}
#[test]
fn late_nested_scalar_value_catch_all_poison_is_not_skipped() {
    let program = program();
    let mut function = function(&program).clone();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("body block")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("outer scalar match")
    };
    let guard = arms[0].guard.as_ref().unwrap().clone();
    let ResolvedExprKind::Match { arms, .. } = &mut arms[0].value.kind else {
        panic!("nested value match")
    };
    arms.last_mut().unwrap().guard = Some(guard);
    let error = expression_skeleton(
        &program,
        &function,
        &function.body,
        &mut SkeletonWork::new(&program, &function, &mut ReplayBudget::new()),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("guard-free catch-all"));
}

#[test]
fn heap_continuation_backing_and_path_storage_refuse_one_short_before_growth() {
    // The ordinary context/phase indexes are constructed before this isolated
    // constructor probe. Only the typed evaluator's actual backing and path
    // constructors are under this ledger; this is not a whole-replay/RSS claim.
    fn probe(
        program: &ResolvedProgram,
        bytes: usize,
    ) -> (Result<Vec<ExprSkeletonPath>, Diagnostic>, usize, bool) {
        let function = function(program);
        let mut budget = ReplayBudget::new();
        let mut work = SkeletonWork::new(program, function, &mut budget);
        work.budget.physical = Some(Box::new(PhysicalReservation::new(bytes)));
        let result = expression_skeleton(program, function, &function.body, &mut work);
        let physical = work.budget.physical.as_mut().unwrap();
        let charged = physical.charged();
        let sticky = physical.reserve(0).is_err();
        (result, charged, sticky)
    }
    let program = program();
    let (measured, required, sticky) = probe(&program, usize::MAX);
    assert!(measured.is_ok() && required > 0 && !sticky);
    let (exact, charged, sticky) = probe(&program, required);
    assert!(exact.is_ok() && !sticky);
    assert_eq!(charged, required);
    let (short, charged, sticky) = probe(&program, required - 1);
    assert_eq!(short.err().unwrap().code, "SPX-H006");
    assert!(charged < required && sticky);
}
