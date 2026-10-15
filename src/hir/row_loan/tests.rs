//! Row Loan v1 HIR owning definitions: exact descriptor binding, whole-Vec
//! call loan, cache transport without authority, and fail-closed forgeries
//! including helper-proof laundering by retargeting to an escaping helper.
use super::*;

const SOURCE: &str = r#"module app.loan;
@id("app.row") record Row { @id("app.row.n") n:i64, @id("app.row.text") text:string, }
@id("app.weight") fn weight(row:borrow Row)->i64 {row.n+1}
@id("app.keep") fn keep(row:borrow Row)->i64 {row.n}
@id("app.escape") fn escape(row:borrow Row)->i64 {keep(row)}
@id("app.view") fn view(row:borrow Row)->i64 {str_len_bytes(string_as_str(row.text))}
@id("app.main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{n:7,text:"x"});
 let mut seen=0;
 for borrow row in rows { seen=seen+weight(row); 0 }
 seen
}"#;

fn resolved() -> ResolvedProgram {
    let program = crate::check(SOURCE, "row-loan-hir.spx").unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    resolved
}

/// The unique call carrying a row loan in `expression`, if any.
fn loan_call(expression: &mut ResolvedExpr) -> Option<&mut ResolvedExpr> {
    let carries = matches!(&expression.kind, ResolvedExprKind::Call { args, .. }
        if args.iter().any(|argument| matches!(argument.kind, ResolvedExprKind::RowLoan { .. })));
    if carries {
        return Some(expression);
    }
    match &mut expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements.iter_mut() {
                let found = match statement {
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::CopyDiscard { value, .. }
                    | ResolvedStatement::Assign { value, .. } => loan_call(value),
                    ResolvedStatement::Unsafe { body, .. } => loan_call(body),
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => match loan_call(condition) {
                        Some(found) => Some(found),
                        None => loan_call(body),
                    },
                };
                if let Some(found) = found {
                    return Some(found);
                }
            }
            loan_call(tail)
        }
        ResolvedExprKind::Binary { left, right, .. } => match loan_call(left) {
            Some(found) => Some(found),
            None => loan_call(right),
        },
        ResolvedExprKind::Call { args, .. } => args.iter_mut().find_map(loan_call),
        _ => None,
    }
}

fn main_loan_call(program: &mut ResolvedProgram) -> &mut ResolvedExpr {
    let main = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    loan_call(&mut main.body).expect("authored row loan call")
}

fn parameter(program: &ResolvedProgram, function: &str) -> ValueId {
    program
        .functions
        .iter()
        .find(|item| item.id.as_str() == function)
        .unwrap()
        .params[0]
        .id
        .clone()
}

#[test]
fn row_loan_binds_its_exact_call_slot_parameter_and_call_long_vec_loan() {
    let mut program = resolved();
    let weight = parameter(&program, "app.weight");
    let lent = lent_parameters(&program);
    assert_eq!(lent.len(), 1);
    assert_eq!(
        lent.get(&DeclarationId::new("app.weight")),
        Some(&BTreeSet::from([0usize]))
    );
    let call = main_loan_call(&mut program);
    let call_id = call.id.clone();
    let ResolvedExprKind::Call { callee, args, .. } = &call.kind else {
        unreachable!()
    };
    assert_eq!(callee.as_str(), "app.weight");
    let ResolvedExprKind::RowLoan {
        loan,
        args: children,
    } = &args[0].kind
    else {
        panic!("row argument is not a row loan")
    };
    assert_eq!(loan.version, RowLoanVersion::V1);
    assert_eq!(loan.callee.as_str(), "app.weight");
    assert_eq!(loan.parameter, weight);
    assert_eq!(args[0].ownership, OwnershipMode::Borrow);
    assert_eq!(args[0].ty, loan.element);
    let ResolvedExprKind::Place(owner) = &children[0].kind else {
        panic!("Vec child")
    };
    let owner = owner.root.clone();
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    // Call-only: live from before argument evaluation, expired right after the
    // call returns, so before the loop latch, continue, break or return.
    let plan = &main.loan_plan;
    let call_loan = plan
        .loans
        .iter()
        .find(|item| {
            item.site == call_id
                && item.origin.root == owner
                && item.origin.projections.is_empty()
                && item.cause == crate::loan_plan::LoanCause::BorrowedCall { argument: 0 }
        })
        .expect("exact whole-Vec call loan");
    assert_eq!(call_loan.start.expression, call_id);
    assert_eq!(
        call_loan.start.phase,
        crate::loan_plan::LoanPointPhase::Before
    );
    let after = plan
        .endpoints
        .iter()
        .position(|endpoint| {
            endpoint.point.expression == call_id
                && endpoint.point.phase == crate::loan_plan::LoanPointPhase::After
        })
        .expect("current call After endpoint");
    assert!(plan.endpoints[after].live_before.contains(&call_loan.id));
    assert!(!plan.endpoints[after].live_after.contains(&call_loan.id));
    // `ends` contains destination points, not the last live point. Every
    // termination must leave this exact Call.After; no latch can extend it.
    assert!(!call_loan.end_edges.is_empty());
    for edge in &call_loan.end_edges {
        let edge = &plan.edges[usize::from(*edge)];
        assert_eq!(usize::from(edge.from), after);
        assert!(!edge.live.contains(&call_loan.id));
        assert!(call_loan
            .ends
            .contains(&plan.endpoints[usize::from(edge.to)].point));
    }
    assert_eq!(call_loan.ends.len(), call_loan.end_edges.len());
    assert!(main
        .loan_plan
        .loans
        .iter()
        .any(|item| item.cause == crate::loan_plan::LoanCause::TraversalFreeze));
}

#[test]
fn row_loan_cache_transport_is_exact_and_carries_no_authority() {
    let program = resolved();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&restored).unwrap();
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
    assert_eq!(restored, program);
}

#[test]
fn forged_row_loans_and_proof_laundering_fail_closed_directly_and_after_cache_decode() {
    let original = resolved();
    let keep = parameter(&original, "app.keep");
    let escape = parameter(&original, "app.escape");
    let view = parameter(&original, "app.view");
    let retarget = |callee: &str, parameter: &ValueId| {
        let callee = DeclarationId::new(callee);
        let parameter = parameter.clone();
        move |call: &mut ResolvedExpr| {
            let ResolvedExprKind::Call {
                callee: called,
                args,
                ..
            } = &mut call.kind
            else {
                unreachable!()
            };
            *called = callee.clone();
            let ResolvedExprKind::RowLoan { loan, .. } = &mut args[0].kind else {
                unreachable!()
            };
            loan.callee = callee.clone();
            loan.parameter = parameter.clone();
        }
    };
    type Forgery = Box<dyn Fn(&mut ResolvedExpr)>;
    let descriptor = |forge: fn(&mut RowLoan, &mut Vec<ResolvedExpr>)| -> Forgery {
        Box::new(move |call: &mut ResolvedExpr| {
            let ResolvedExprKind::Call { args, .. } = &mut call.kind else {
                unreachable!()
            };
            let ResolvedExprKind::RowLoan { loan, args } = &mut args[0].kind else {
                unreachable!()
            };
            forge(loan, args);
        })
    };
    let proof_laundering: Vec<Forgery> = vec![
        // Exact signature, escaping body: a name or signature is no proof.
        Box::new(retarget("app.escape", &escape)),
        Box::new(retarget("app.view", &view)),
    ];
    for forge in &proof_laundering {
        let mut forged = original.clone();
        forge(main_loan_call(&mut forged));
        let error = crate::hir::validate(&forged).unwrap_err();
        assert_eq!(error.code, "SPX-T284", "{error:?}");
        assert!(error.message.contains("may only read Copy scalar fields"));
    }
    let structural: Vec<Forgery> = vec![
        // Descriptor callee/parameter disagree with the call slot.
        descriptor(|loan, _| loan.callee = DeclarationId::new("app.keep")),
        Box::new(move |call: &mut ResolvedExpr| {
            let ResolvedExprKind::Call { args, .. } = &mut call.kind else {
                unreachable!()
            };
            let ResolvedExprKind::RowLoan { loan, .. } = &mut args[0].kind else {
                unreachable!()
            };
            loan.parameter = keep.clone();
        }),
        descriptor(|loan, _| loan.callee = DeclarationId::new(UNBOUND_CALLEE)),
        // Traversal region, bound and children must be the exact freeze facts.
        descriptor(|loan, args| loan.traversal = args[0].id.clone()),
        descriptor(|loan, args| {
            let ResolvedExprKind::Place(index) = &args[1].kind else {
                unreachable!()
            };
            loan.bound = index.root.clone();
        }),
        descriptor(|_, args| args.swap(0, 1)),
        descriptor(|_, args| {
            args.pop();
        }),
        descriptor(|loan, _| loan.element = ResolvedType::I64),
    ];
    for forge in &structural {
        let mut forged = original.clone();
        forge(main_loan_call(&mut forged));
        assert!(crate::hir::validate(&forged).is_err());
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert!(crate::hir::validate(&restored).is_err());
    }
}

#[test]
fn lent_helper_effect_and_owned_or_non_scalar_result_forgery_is_independently_refused() {
    let original = resolved();
    super::validate_program(&original).unwrap();
    let returns = [
        ResolvedType::String,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::Unit,
        ResolvedType::Never,
        crate::vec_ops::resolved_vec(ResolvedType::I64),
    ];
    for ty in returns {
        let mut forged = original.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.weight")
            .unwrap();
        function.return_type = ty;
        let error = super::validate_program(&forged).unwrap_err();
        assert_eq!(error.code, "SPX-T284");
        assert_eq!(
            error.message,
            "row loan helper must be effect-free and return one Value Copy scalar"
        );
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert_eq!(
            super::validate_program(&restored).unwrap_err().code,
            "SPX-T284"
        );
    }
    for own in [false, true] {
        let mut forged = original.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.weight")
            .unwrap();
        if own {
            function.body.ownership = OwnershipMode::Own;
        } else {
            function.effects.push("stdout".to_owned());
        }
        let error = super::validate_program(&forged).unwrap_err();
        assert_eq!(error.code, "SPX-T284");
        assert_eq!(
            error.message,
            "row loan helper must be effect-free and return one Value Copy scalar"
        );
        assert!(crate::hir::validate(&forged).is_err());
    }
}
