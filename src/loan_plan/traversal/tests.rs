use super::*;

const SOURCE: &str = r#"module app.traversal_proof;
@id("row") record Row { @id("row.left") left:i64, @id("row.right") right:i64, }
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{left:7,right:9});
 let mut total=0;
 for borrow row at index in rows { total=total+row.left+row.right+i64_from_usize(index); 0 }
 total
}"#;
fn resolved() -> ResolvedProgram {
    let source = crate::check(SOURCE, "record-traversal-proof.spx").unwrap();
    crate::hir::resolve(&source).unwrap()
}

#[test]
fn copy_only_rows_keep_complete_freeze_and_exact_loan_capacity() {
    let program = resolved();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(function.loan_plan.schema, LOAN_PLAN_SCHEMA_V2);
    let freeze = function
        .loan_plan
        .loans
        .iter()
        .find(|loan| loan.cause == LoanCause::TraversalFreeze)
        .unwrap();
    assert_eq!(freeze.start.phase, LoanPointPhase::Before);
    assert!(!freeze.end_edges.is_empty());
    assert_eq!(
        function.loan_plan.loans.capacity(),
        function.loan_plan.loans.len()
    );
    let legacy = crate::check("module app.legacy; @id(\"app.main\") fn main()->i64 { let bytes=bytes_zeroed(1usize); let view=bytes_as_slice(bytes); i64_from_usize(byte_len(view)) }", "legacy-loan-capacity.spx").unwrap();
    let legacy = crate::hir::resolve(&legacy).unwrap();
    for function in &legacy.functions {
        assert_eq!(function.loan_plan.schema, LOAN_PLAN_SCHEMA_V1);
        assert_eq!(
            function.loan_plan.loans.capacity(),
            function.loan_plan.loans.len()
        );
    }
}

#[test]
fn changed_snapshot_and_missing_freeze_are_rejected_independently() {
    let original = resolved();
    let mut forged = original.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    function
        .loan_plan
        .loans
        .retain(|loan| loan.cause != LoanCause::TraversalFreeze);
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    let mut forged = original;
    let function = forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("body")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[3] else {
        panic!("loop wrapper")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("loop")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("snapshot")
    };
    value.kind = ResolvedExprKind::Usize(1);
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
}

#[test]
fn copy_record_read_borrows_do_not_authorize_consuming_vector_calls() {
    let original = resolved();
    let function = original
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let mut pending = vec![&function.body];
    let mut reads = 0;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, args, .. } = &expression.kind {
            if matches!(
                callee.as_str(),
                crate::vec_ops::LEN_ID | crate::vec_ops::GET_ID
            ) {
                assert!(record_vec_borrow::argument(
                    &original, expression, 0, &args[0]
                ));
                let mut consuming = expression.clone();
                let ResolvedExprKind::Call { callee, .. } = &mut consuming.kind else {
                    unreachable!()
                };
                *callee = crate::hir::DeclarationId::new(crate::vec_ops::CLEAR_ID);
                assert!(!record_vec_borrow::argument(
                    &original, &consuming, 0, &args[0]
                ));
                reads += 1;
            }
        }
        push_children(expression, &mut pending);
    }
    assert_eq!(
        reads, 3,
        "one length snapshot and two exact Copy field reads"
    );
    crate::hir::validate(&original).unwrap();
}
