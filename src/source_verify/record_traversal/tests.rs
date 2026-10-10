use super::*;

fn body(text: &str) -> Expr {
    crate::parse(
        &format!("module app.scan; @id(\"app.scan\") fn scan()->i64{{{text}}}"),
        "scan.spx",
    )
    .unwrap()
    .functions[0]
        .body
        .clone()
}

#[test]
fn shadow_precedence_matches_reverse_child_frontier() {
    let expression = body("if true {let row=1; 0} else {let root=2; 0}");
    let span = body_binding_conflict(&expression, "root", "row", None)
        .unwrap()
        .unwrap();
    // The old LIFO scanner visited the else branch before the then branch.
    let ExprKind::Block { tail, .. } = &expression.kind else {
        panic!()
    };
    let ExprKind::If { else_branch, .. } = &tail.kind else {
        panic!()
    };
    let ExprKind::Block { statements, .. } = &else_branch.kind else {
        panic!()
    };
    let crate::ast::Statement::Let { name_span, .. } = &statements[0] else {
        panic!()
    };
    assert_eq!(span, *name_span);
}

#[test]
fn cursor_depth_and_work_are_exact_and_fail_closed() {
    let expression = body("0");
    // Two expression visits, three child probes and one repeated child lookup.
    // The missing Block child costs two preceding-slot work units.
    assert_eq!(scan(&expression, "root", "row", None, 2, 7), Ok(None));
    assert_eq!(scan(&expression, "root", "row", None, 2, 6), Err(()));
    assert_eq!(scan(&expression, "root", "row", None, 1, 7), Err(()));
    let siblings = body(&format!(
        "{}0",
        (0..32)
            .map(|n| format!("let local{n}=1; "))
            .collect::<String>()
    ));
    assert_eq!(
        scan(&siblings, "root", "row", None, 2, MAX_SCAN_WORK),
        Ok(None)
    );
    let wide = body("1+2+3+4+5+6+7+8");
    assert_eq!(scan(&wide, "root", "row", None, 2, MAX_SCAN_WORK), Err(()));
}

#[test]
fn complete_scratch_is_charged_and_one_short_is_sticky() {
    let expression = body("0");
    let bytes = std::mem::size_of::<[(&Expr, usize); MAX_SCAN_DEPTH]>();
    let (result, refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
        body_binding_conflict(&expression, "root", "row", None)
    });
    assert_eq!(result, Ok(None));
    assert!(!refused);
    assert_eq!(used, bytes);
    let (result, refused, used) = crate::bounded_output::with_limit_usage(bytes - 1, || {
        body_binding_conflict(&expression, "root", "row", None)
    });
    assert_eq!(result, Err(()));
    assert!(refused);
    assert_eq!(used, 0);
}
