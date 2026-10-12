//! Original full planner collection, exact source/instance and payer controls.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::hir::{DeclarationId, FunctionExecutionId, ResolvedExprKind};
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"module initial.loan.plans;
@id("packet") record Packet {
 @id("payload") payload:Bytes,
 @id("sibling") sibling:Bytes,
}
@id("first") fn first(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.payload);byte_len(view)+byte_len(view)
}
@id("second") fn second(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.sibling);byte_len(view)
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(0)}
"#;
fn fixture() -> ResolvedProgram {
    let ast = crate::check(SOURCE, "initial-loan-plans.spx").unwrap();
    let mut program = crate::hir::resolve(&ast).unwrap();
    assert_eq!(program.function_instances.len(), 1);
    assert!(program
        .functions
        .iter()
        .any(|f| !f.loan_plan.loans.is_empty()));
    for function in &mut program.functions {
        function.loan_plan = LoanPlan::unresolved();
    }
    for instance in &mut program.function_instances {
        instance.function.loan_plan = LoanPlan::unresolved();
    }
    program
}

// Saved complete original attachment route: authored plans are collected and
// installed before instance planning; each group publishes only when complete.
fn legacy_attach(resolved: &mut ResolvedProgram) -> Result<(), Diagnostic> {
    let loan_plans = staged!(
        HirLoanPlans,
        resolved
            .functions
            .iter()
            .map(|function| crate::loan_plan::build_plan(resolved, function))
            .collect::<Result<Vec<_>, _>>()
    )?;
    for (function, loan_plan) in resolved.functions.iter_mut().zip(loan_plans) {
        function.loan_plan = loan_plan;
    }
    let instance_loan_plans = staged!(
        HirInstanceLoanPlans,
        resolved
            .function_instances
            .iter()
            .map(|instance| crate::loan_plan::build_plan(resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()
    )?;
    for (instance, loan_plan) in resolved
        .function_instances
        .iter_mut()
        .zip(instance_loan_plans)
    {
        instance.function.loan_plan = loan_plan;
    }
    Ok(())
}

#[test]
fn complete_initial_attachment_matches_original_authored_and_instance_plans() {
    let original = fixture();
    let mut expected = original.clone();
    legacy_attach(&mut expected).unwrap();
    let mut ordinary = original.clone();
    attach(&mut ordinary).unwrap();
    assert_eq!(ordinary, expected);
    let mut paid = original;
    let (result, refused, used) = with_limit_usage(usize::MAX, || attach(&mut paid));
    result.unwrap();
    assert!(!refused && used > 0);
    assert_eq!(paid, expected);
    // Independent canonical reconstruction is still required after attachment.
    crate::hir::validate(&paid).unwrap();
}

fn observed(limit: usize) -> (bool, bool, usize, crate::test_allocation_observer::Metrics) {
    let mut program = fixture();
    let ((success, metrics), refused, used, _) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let result = attach(&mut program);
            let success = result.is_ok();
            if let Err(error) = &result {
                assert_eq!(error.code, "SPX-H006");
                let remaining = crate::bounded_output::active_remaining();
                let retry = attach(&mut program);
                assert_eq!(retry.unwrap_err().code, "SPX-H006");
                assert_eq!(remaining, crate::bounded_output::active_remaining());
            }
            drop(result);
            // Observe every plan retained by this constructor through its Drop.
            // The preexisting HIR is outside this planner-only allocation scope;
            // the unchanged subtree owner still measures full resolve→HIR Drop.
            for function in &mut program.functions {
                drop(std::mem::replace(
                    &mut function.loan_plan,
                    LoanPlan::unresolved(),
                ));
            }
            for instance in &mut program.function_instances {
                drop(std::mem::replace(
                    &mut instance.function.loan_plan,
                    LoanPlan::unresolved(),
                ));
            }
            success
        })
    });
    (success, refused, used, metrics)
}

#[test]
fn complete_initial_planner_backing_is_paid_through_drop_exact_short_and_sticky() {
    let (success, refused, required, metrics) = observed(usize::MAX);
    assert!(success && !refused);
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.freed, metrics.allocated);
    assert!(
        metrics.allocated <= required,
        "initial planner requested {:?}, ledger debited {required}",
        metrics
    );
    let (success, refused, exact, _) = observed(required);
    assert!(success && !refused);
    assert_eq!(exact, required);
    for limit in [required - 1, 0] {
        let (success, refused, used, metrics) = observed(limit);
        assert!(!success && refused);
        assert!(used <= limit);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
}

#[test]
fn first_complete_plan_precedes_exact_staging_refusal_without_partial_attachment() {
    let mut program = fixture();
    let before = program.clone();
    let ((), refused, first_boundary) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let plan = match crate::loan_plan::build_plan_with_reservation(
            &program,
            &program.functions[0],
            &owner,
        ) {
            Ok(plan) => plan,
            Err(_) => panic!("first ordinary member must plan"),
        };
        owner.array::<LoanPlan>(program.functions.len()).unwrap();
        drop(plan);
    });
    assert!(!refused);
    let (result, refused, _) = with_limit_usage(first_boundary - 1, || attach(&mut program));
    assert!(refused);
    let error = result.unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "canonical loan CFG storage reservation refused"
    );
    assert_eq!(program, before);
}

fn hostile_return(function: &mut ResolvedFunction) {
    function.body.kind = ResolvedExprKind::LexicalExit {
        kind: crate::ast::LexicalExitKind::Return,
        target: crate::hir::lexical_exit::ControlTarget::Function(
            FunctionExecutionId::Monomorphic(DeclarationId::new("foreign.owner")),
        ),
        value: None,
    };
}
fn same_error(actual: &Diagnostic, expected: &Diagnostic) {
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.path, expected.path);
    assert_eq!(actual.span, expected.span);
    assert_eq!(actual.help, expected.help);
}
#[test]
fn authored_late_semantic_error_keeps_original_diagnostic_and_group_unpublished() {
    let mut program = fixture();
    hostile_return(program.functions.last_mut().unwrap());
    let before = program.clone();
    let mut old = before.clone();
    let expected = legacy_attach(&mut old).unwrap_err();
    assert_eq!(old, before);
    let (actual, refused, _) = with_limit_usage(usize::MAX, || attach(&mut program));
    assert!(!refused);
    same_error(&actual.unwrap_err(), &expected);
    assert_eq!(program, before);
}
#[test]
fn instance_late_semantic_error_preserves_completed_authored_attachment_only() {
    let mut program = fixture();
    hostile_return(&mut program.function_instances[0].function);
    let mut old = program.clone();
    let expected = legacy_attach(&mut old).unwrap_err();
    assert!(old.functions.iter().any(|f| !f.loan_plan.loans.is_empty()));
    let (actual, refused, _) = with_limit_usage(usize::MAX, || attach(&mut program));
    assert!(!refused);
    same_error(&actual.unwrap_err(), &expected);
    assert_eq!(program, old);
}
