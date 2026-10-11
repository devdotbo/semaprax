//! Lookup order is compared against the original indexed walk, not identities.
use super::*;

fn fixture(source: &str) -> hir::ResolvedFunction {
    crate::hir::resolve(&crate::check(source, "native-lookup.spx").unwrap())
        .unwrap()
        .functions
        .remove(0)
}
fn original<'a>(root: &'a ResolvedExpr, output: &mut Vec<&'a ResolvedExpr>) {
    output.push(root);
    let mut index = 0;
    while let Some(child) = hir::lexical_exit::child(root, index) {
        original(child, output);
        index += 1;
    }
}
fn assert_order(root: &ResolvedExpr) {
    let mut expected = Vec::new();
    original(root, &mut expected);
    let mut actual = Vec::new();
    walk(root, |item| {
        actual.push(item);
        Ok(())
    })
    .unwrap();
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(std::ptr::eq(actual, expected));
    }
}

#[test]
fn cursor_preserves_checked_nested_scope_and_lexical_operand_order() {
    let function = fixture(
        r#"module native.lookup;
@id("probe") fn probe(n:i64)->i64 {
 let mut i=0;
 while i<n {i=i+1;if i==2 {continue;}else{0} if i==3 {break;}else{0} 0}
 if i>0 {return i+1;}else{0}
 return 0;
}
@id("main") fn main()->i64 {probe(4)}
"#,
    );
    assert!(hir::lexical_exit::requires(&function).unwrap());
    assert_order(&function.body);
    let mut nodes = Vec::new();
    original(&function.body, &mut nodes);
    for node in nodes {
        assert!(std::ptr::eq(
            expression(&function.body, &node.id).unwrap(),
            node
        ));
    }
    let absent = ExpressionId::from_owned("absent.native.anchor".into());
    assert!(expression(&function.body, &absent).is_err());
    assert!(assignment(&function.body, &absent).is_err());
}

#[test]
fn wide_blocks_have_independent_linear_debits_and_retain_duplicate_refusal() {
    let function = fixture("module native.wide; @id(\"main\") fn main()->i64 {let n=1;0}");
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("block")
    };
    for width in [0usize, 1, 32, 1024, 8192] {
        let mut root = function.body.clone();
        root.kind = ResolvedExprKind::Block {
            statements: vec![statements[0].clone(); width],
            tail: tail.clone(),
        };
        // Two N-statement scans + N+2 inspections + 2N+3 child probes.
        let exact = 5 * width + 5;
        assert_eq!(walk_with_limit(&root, exact, |_| Ok(())).unwrap(), exact);
        assert!(walk_with_limit(&root, exact - 1, |_| Ok(())).is_err());
        if width <= 1024 {
            assert_order(&root);
        }
        if width > 1 {
            assert!(expression(&root, &statements[0].value().id).is_err());
        }
    }
    for width in [0usize, 1, 32, 1024] {
        let mut root = function.body.clone();
        root.kind = ResolvedExprKind::Match {
            scrutinee: tail.clone(),
            arms: (0..width)
                .map(|index| hir::ResolvedMatchArm {
                    pattern: hir::ResolvedMatchPattern::Wildcard,
                    guard: (index % 2 == 0).then(|| tail.clone()),
                    value: tail.as_ref().clone(),
                    span: tail.span,
                })
                .collect(),
            mode: hir::ResolvedMatchMode::Value,
        };
        let guards = width.div_ceil(2);
        // N arm visits, N+G+2 inspections and 2N+2G+3 child probes.
        let exact = 4 * width + 3 * guards + 5;
        assert_eq!(walk_with_limit(&root, exact, |_| Ok(())).unwrap(), exact);
        assert!(walk_with_limit(&root, exact - 1, |_| Ok(())).is_err());
        assert_order(&root);
    }
    let mut inspected = 0;
    assert!(walk_with_limit(&function.body, 0, |_| {
        inspected += 1;
        Ok(())
    })
    .is_err());
    assert_eq!(inspected, 0, "refuse before touching unpaid root");
}

#[test]
fn cursor_keeps_depth_boundary_and_assignment_ambiguity() {
    let function =
        fixture("module native.assignment; @id(\"main\") fn main()->i64 {let mut n=1;n=n+1;n}");
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Assign { binding, value, .. } = &statements[1] else {
        panic!("assignment")
    };
    let (actual_binding, actual_value) = assignment(&function.body, &value.id).unwrap();
    assert!(std::ptr::eq(actual_binding, binding));
    assert!(std::ptr::eq(actual_value, value));
    let mut duplicate = function.body.clone();
    duplicate.kind = ResolvedExprKind::Block {
        statements: vec![statements[1].clone(), statements[1].clone()],
        tail: tail.clone(),
    };
    assert!(assignment(&duplicate, &value.id).is_err());
    let mut nested = tail.as_ref().clone();
    for _ in 1..MAX_DEPTH {
        let mut next = function.body.clone();
        next.kind = ResolvedExprKind::Block {
            statements: Vec::new(),
            tail: Box::new(nested),
        };
        nested = next;
    }
    walk(&nested, |_| Ok(())).unwrap();
    let mut over = function.body.clone();
    over.kind = ResolvedExprKind::Block {
        statements: Vec::new(),
        tail: Box::new(nested),
    };
    assert!(walk(&over, |_| Ok(())).is_err());
}
