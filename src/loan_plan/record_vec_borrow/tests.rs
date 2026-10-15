use super::super::*;
use crate::hir::{DeclarationId, FunctionInstanceId};

fn fixture() -> ResolvedProgram {
    let source = r#"module app.clone_loan;
@id("row") record Row { @id("row.n") n:i64,@id("row.text") text:string,@id("row.data") data:Bytes, }
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{n:7,text:"leaf",data:bytes_zeroed(1usize)});
 let copied=vec_clone_at<Row>(rows,0usize);
 copied.n
}"#;
    let ast = crate::check(source, "owned-row-clone-loan.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
fn clone_call(function: &ResolvedFunction) -> &ResolvedExpr {
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("body")
    };
    let ResolvedStatement::Let { value, .. } = &statements[2] else {
        panic!("copy binding")
    };
    value
}

#[test]
fn owned_leaf_record_clone_borrows_exact_owner_and_missing_loan_refuses() {
    let program = fixture();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let call = clone_call(function);
    let ResolvedExprKind::Call { args, .. } = &call.kind else {
        panic!("clone call")
    };
    assert_eq!(args[0].ownership, OwnershipMode::Own);
    assert!(record_vec_borrow::argument(&program, call, 0, &args[0]));
    let loans: Vec<_> = function
        .loan_plan
        .loans
        .iter()
        .filter(|loan| {
            loan.site == call.id && loan.cause == LoanCause::BorrowedCall { argument: 0 }
        })
        .collect();
    assert_eq!(loans.len(), 1);
    let ResolvedExprKind::Place(place) = &args[0].kind else {
        panic!("named owner")
    };
    assert_eq!(&loans[0].origin, place);
    assert!(!loans[0].end_edges.is_empty());
    let mut forged = program.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    function.loan_plan.loans.retain(|loan| loan.site != call.id);
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
}

#[test]
fn owned_record_clone_borrow_classifier_rejects_consuming_and_forged_shapes() {
    let program = fixture();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let call = clone_call(function);
    let ResolvedExprKind::Call { args, .. } = &call.kind else {
        panic!("clone call")
    };
    assert!(!record_vec_borrow::argument(&program, call, 1, &args[0]));
    for attack in 0..7 {
        let mut forged = call.clone();
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } = &mut forged.kind
        else {
            unreachable!()
        };
        match attack {
            0 => *callee = DeclarationId::new(crate::vec_ops::CLEAR_ID),
            1 => {
                type_arguments.clear();
            }
            2 => {
                args.pop();
            }
            3 => args[1].ty = ResolvedType::I64,
            4 => {
                let ResolvedExprKind::Place(place) = &mut args[0].kind else {
                    unreachable!()
                };
                place
                    .projections
                    .push(PlaceProjection::Field(DeclarationId::new("row.data")));
            }
            5 => forged.ty = ResolvedType::Bytes,
            _ => {
                *instance = Some(FunctionInstanceId::derive(
                    &DeclarationId::new("forged"),
                    &[],
                ));
            }
        }
        let ResolvedExprKind::Call { args, .. } = &forged.kind else {
            unreachable!()
        };
        assert!(
            !record_vec_borrow::argument(&program, &forged, 0, &args[0]),
            "attack {attack}"
        );
    }
}
