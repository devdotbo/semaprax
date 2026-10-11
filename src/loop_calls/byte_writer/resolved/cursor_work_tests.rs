//! Work limits bind saved-cursor operations, not obsolete indexed prefixes.
use super::*;
fn function(width: usize) -> ResolvedFunction {
    let statements = (0..width)
        .map(|i| format!("let n{i}=0;"))
        .collect::<String>();
    let source = format!("module app; @id(\"app.main\") fn main()->i64 {{{statements}0}}");
    crate::hir::resolve(&crate::parse(&source, "helper-linear-work.spx").unwrap())
        .unwrap()
        .functions
        .remove(0)
}
fn run(function: &ResolvedFunction, limit: usize) -> Option<()> {
    let mut frames = [None; DEPTH];
    prove_with_frames(
        function,
        &|_, _| None,
        &|_, _, _| true,
        &|_, ty| crate::hir::is_scalar_resolved_type(ty),
        0,
        (&mut frames, limit),
    )
}
#[test]
fn helper_wide_census_and_constant_advances_have_exact_one_short_work() {
    for width in [0, 1, 64, 512] {
        let function = function(width);
        // Function body/terminal probes; one expression entry; each next
        // attempt costs one loop unit plus two conservative cursor units.
        let exact = 8 * width + 13;
        assert!(run(&function, exact).is_some());
        assert!(run(&function, exact - 1).is_none());
    }
    let function = function(0);
    let bytes = std::mem::size_of::<[Option<Frame<'_>>; DEPTH]>();
    let (exact, refused, charged) = crate::bounded_output::with_limit_usage(bytes, || {
        nonallocating_with(&function, |_, _| None, |_, _, _| true, |_, _| true, 0)
    });
    assert!(exact && !refused);
    assert_eq!(charged, bytes);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(bytes - 1, || {
        nonallocating_with(&function, |_, _| None, |_, _, _| true, |_, _| true, 0)
    });
    assert!(!short && refused);
}
#[test]
fn deep_helper_work_does_not_buy_depth_or_late_unknown_callee_permission() {
    let mut deep = function(0);
    let ResolvedExprKind::Block { tail, .. } = &mut deep.body.kind else {
        panic!()
    };
    for _ in 0..32 {
        let inner = tail.clone();
        tail.kind = ResolvedExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: inner,
        };
    }
    assert!(run(&deep, 13 + 7 * 32).is_some());
    assert!(run(&deep, 13 + 7 * 32 - 1).is_none());
    let ResolvedExprKind::Block { tail, .. } = &mut deep.body.kind else {
        panic!()
    };
    for _ in 32..DEPTH {
        let inner = tail.clone();
        tail.kind = ResolvedExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: inner,
        };
    }
    assert!(run(&deep, usize::MAX).is_none());
    let mut late = function(64);
    let ResolvedExprKind::Block { tail, .. } = &mut late.body.kind else {
        panic!()
    };
    tail.kind = ResolvedExprKind::Call {
        callee: crate::hir::DeclarationId::new("unknown"),
        instance: None,
        type_arguments: Vec::new(),
        args: Vec::new(),
    };
    assert!(run(&late, usize::MAX).is_none());
}
