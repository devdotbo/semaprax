//! Row-loan traversal coverage grants no scalar Rust ABI authority.
use super::*;

#[test]
fn row_loan_arguments_and_descriptor_are_censused_and_disposed_but_scalar_abi_refuses() {
    let source = crate::parse(
        r#"module interop.closed; @id("helper") fn helper(value:i64)->i64 {value} @id("main") fn main()->i64 {7}"#,
        Path::new("row-loan-closed.spx"),
    ).unwrap();
    let mut program = hir::resolve(&source).unwrap();
    let helper = &program.functions[0];
    let mut row = helper.body.clone();
    // Deliberately scalar-looking forged descriptor: neither its children nor
    // its callee field may confer profile authority or become a real call site.
    row.kind = ResolvedExprKind::RowLoan {
        loan: Box::new(hir::RowLoan {
            version: hir::RowLoanVersion::V1,
            element: ResolvedType::I64,
            traversal: row.id.clone(),
            bound: helper.params[0].id.clone(),
            callee: helper.id.clone(),
            parameter: helper.params[0].id.clone(),
        }),
        args: vec![helper.body.clone(), program.functions[1].body.clone()],
    };
    let ResolvedExprKind::RowLoan { loan, args } = &row.kind else {
        panic!("row fixture")
    };
    let mut cursor = 0;
    for (index, arg) in args.iter().enumerate() {
        let (actual, child) = resolved_expression_child(&row, &mut cursor).unwrap();
        assert_eq!(actual, index);
        assert!(std::ptr::eq(child, arg));
    }
    assert!(resolved_expression_child(&row, &mut cursor).is_none());
    let expected = std::mem::size_of::<ResolvedExpr>()
        + row.id.owned_allocation_bytes().unwrap()
        + std::mem::size_of::<hir::RowLoan>()
        + loan.traversal.owned_allocation_bytes().unwrap()
        + loan.bound.as_str().len()
        + loan.callee.as_str().len()
        + loan.parameter.as_str().len()
        + args.capacity() * std::mem::size_of::<ResolvedExpr>()
        + args
            .iter()
            .map(|arg| capacity::hir_expr_owned_capacity(arg).unwrap())
            .sum::<usize>();
    assert_eq!(capacity::hir_expr_owned_capacity(&row).unwrap(), expected);
    assert_eq!(
        expression_call_site_census(&row)
            .err()
            .expect("row loan is refused")
            .code,
        "SPX-B107"
    );
    let mut calls = BTreeSet::new();
    let mut imports = BTreeSet::new();
    assert_eq!(
        visit_calls(&row, &mut calls, &mut imports, 0, 0)
            .unwrap_err()
            .code,
        "SPX-B107"
    );
    assert!(calls.is_empty() && imports.is_empty());
    assert_eq!(
        fingerprint_expression_types_scratch(&row, 1)
            .unwrap_err()
            .code,
        "SPX-B107"
    );
    assert_eq!(
        hash_expr(&mut Sha256::new(), &row, 0).unwrap_err().code,
        "SPX-B107"
    );
    let mut count = 0;
    let mut lines = artifacts::CExpressionLineArena::new();
    assert_eq!(
        artifacts::c_expr(&row, &[], &mut count, &mut lines)
            .unwrap_err()
            .code,
        "SPX-B107"
    );
    assert_eq!(count, 0);
    assert_eq!(lines.as_str().unwrap(), "");
    program.functions[0].body = row;
    assert_eq!(
        phase_a::validate_selected_scalar_closure(&[&program.functions[0]])
            .unwrap_err()
            .code,
        "SPX-B107"
    );
    assert_eq!(
        phase_a::validate_native_unit_discard_bindings(&[&program.functions[0]])
            .unwrap_err()
            .code,
        "SPX-B107"
    );
    // The structural budget receiver still traverses both children. The
    // ordinary scalar profile refuses above rather than shortening this walk.
    capacity::validate_native_rust_expression_budget(&program).unwrap();
    RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
    let capacity = MAX_SEMANTIC_EXPRESSION_DEPTH + 1;
    drop(ResolvedProgramOwner::new(
        program,
        Vec::with_capacity(capacity),
        capacity,
    ));
    assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 1);
}
