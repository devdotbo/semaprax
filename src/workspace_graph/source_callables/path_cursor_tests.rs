use super::{child, visit_ast_expression_sites, Path};
use crate::ast::{
    BinaryOp, Expr, ExprKind as K, FieldInitializer, LexicalExitKind, MatchArm, MatchMode,
    MatchPattern, Span, Statement as S, Type, UnaryOp,
};
use std::fmt::Write;
fn expr(kind: K) -> Expr {
    Expr {
        kind,
        span: Span::default(),
    }
}
fn call() -> Expr {
    expr(K::Call {
        name: "observe".into(),
        type_arguments: vec![],
        args: vec![expr(K::Int(1))],
    })
}
fn field() -> FieldInitializer {
    FieldInitializer {
        name: "n".into(),
        name_span: Span::default(),
        value: call(),
        span: Span::default(),
    }
}
fn compared(root: &Expr, path: &str) {
    let mut old = Vec::new();
    let mut new = Vec::new();
    super::super::old_walk::visit_ast_expression_sites(root, path, &mut |e, p| {
        old.push((e as *const Expr, p.to_owned()));
        Ok(())
    })
    .unwrap();
    visit_ast_expression_sites(root, path, &mut |e, p| {
        new.push((e as *const Expr, p.to_owned()));
        Ok(())
    })
    .unwrap();
    assert_eq!(new, old);
}
#[test]
fn reusable_paths_preserve_every_expression_and_statement_child_domain() {
    let z = Span::default();
    let fields = || vec![field(), field()];
    let cases = vec![
        K::VecConstructor {
            element: Type::I64,
            count: 1,
            value: Box::new(call()),
        },
        K::Closure {
            params: vec![],
            return_type: Type::I64,
            body: Box::new(call()),
            owning: false,
            retained: false,
            mutable: false,
        },
        K::Call {
            name: "outer".into(),
            type_arguments: vec![],
            args: vec![call(), call()],
        },
        K::LexicalExit {
            kind: LexicalExitKind::Return,
            value: Some(Box::new(call())),
        },
        K::LexicalExit {
            kind: LexicalExitKind::Break,
            value: None,
        },
        K::LexicalExit {
            kind: LexicalExitKind::Continue,
            value: None,
        },
        K::Unary {
            op: UnaryOp::Neg,
            value: Box::new(call()),
        },
        K::Binary {
            op: BinaryOp::Add,
            left: Box::new(call()),
            right: Box::new(call()),
        },
        K::If {
            condition: Box::new(call()),
            then_branch: Box::new(call()),
            else_branch: Box::new(call()),
        },
        K::ConstructRecord {
            type_name: "R".into(),
            type_span: z,
            type_arguments: vec![],
            fields: fields(),
        },
        K::ConstructVariant {
            type_name: "V".into(),
            type_span: z,
            type_arguments: vec![],
            case_name: "C".into(),
            case_span: z,
            fields: fields(),
        },
        K::Match {
            mode: MatchMode::Value,
            scrutinee: Box::new(call()),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Wildcard { span: z },
                    guard: Some(Box::new(call())),
                    value: call(),
                    span: z,
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard { span: z },
                    guard: None,
                    value: call(),
                    span: z,
                },
            ],
        },
        K::Try {
            operand: Box::new(call()),
        },
        K::Yield {
            request: Box::new(call()),
        },
        K::UpdateRecord {
            base: Box::new(call()),
            fields: fields(),
        },
        K::Project {
            base: Box::new(call()),
            field: "n".into(),
            field_span: z,
        },
        // These legacy domains intentionally have no visited method children.
        K::MethodCall {
            receiver: Box::new(call()),
            method: "m".into(),
            method_span: z,
            type_arguments: vec![],
            args: vec![call()],
        },
        K::SuperMethod {
            method: "m".into(),
            method_span: z,
            args: vec![call()],
        },
        K::Int(1),
        K::Int32(1),
        K::Char(65),
        K::Uint8(1),
        K::Usize(1),
        K::Float32(0),
        K::Float64(0),
        K::Bool(true),
        K::ArrayU8(vec![1]),
        K::RepeatArrayU8 { value: 1, count: 1 },
        K::String("é:😀".into()),
        K::BorrowedUtf8Literal("é:😀".into()),
        K::Var("n".into()),
    ];
    for kind in cases {
        compared(&expr(kind), "é:😀:body");
    }
    let statements = vec![
        S::Let {
            syntax: Default::default(),
            name: "n".into(),
            name_span: z,
            mutable: false,
            declared: None,
            value: call(),
            span: z,
        },
        S::Assign {
            name: "n".into(),
            name_span: z,
            field: None,
            value: call(),
            span: z,
        },
        S::Unsafe {
            audit: "test".into(),
            audit_span: z,
            body: Box::new(call()),
            span: z,
        },
        S::While {
            condition: Box::new(call()),
            body: Box::new(call()),
            span: z,
        },
        S::For {
            item: "item".into(),
            item_span: z,
            values: Box::new(call()),
            body: Box::new(call()),
            span: z,
        },
        S::ForBorrow {
            item: "item".into(),
            item_span: z,
            index: None,
            values: Box::new(call()),
            body: Box::new(call()),
            span: z,
        },
        S::ForOwn {
            item: "item".into(),
            item_span: z,
            values: Box::new(call()),
            body: Box::new(call()),
            span: z,
        },
    ];
    compared(
        &expr(K::Block {
            statements,
            tail: Box::new(call()),
        }),
        "requires.0",
    );
    let mut deep = call();
    for _ in 0..128 {
        deep = expr(K::Unary {
            op: UnaryOp::Neg,
            value: Box::new(deep),
        });
    }
    for path in ["requires.0", "body", "ensures.0"] {
        compared(&deep, path);
    }
    let wide = expr(K::Call {
        name: "wide".into(),
        type_arguments: vec![],
        args: (0..256).map(|_| call()).collect(),
    });
    compared(&wide, "body");
    let (_, bad, old_bytes) = crate::bounded_output::with_limit_usage(1_000_000, || {
        super::super::old_walk::visit_ast_expression_sites(&wide, "body", &mut |_, _| Ok(()))
    });
    assert!(!bad);
    let (_, bad, new_bytes) = crate::bounded_output::with_limit_usage(1_000_000, || {
        visit_ast_expression_sites(&wide, "body", &mut |_, _| Ok(()))
    });
    assert!(!bad);
    assert!(new_bytes < old_bytes);
}
#[test]
fn path_replacements_charge_full_overlap_exactly_and_refuse_one_short_before_growth() {
    let exact = std::mem::size_of::<Path>() + 32 + 64 + 128;
    for limit in [exact, exact - 1] {
        let ((ok, capacity, length), overflowed, used) =
            crate::bounded_output::with_limit_usage(limit, || {
                let mut path = Path::new("r").unwrap();
                path.write_str(&"x".repeat(40)).unwrap();
                path.value.truncate(1);
                let ok = path.write_str(&"x".repeat(70)).is_ok();
                (ok, path.value.capacity(), path.value.len())
            });
        assert_eq!(ok, limit == exact);
        assert_eq!(overflowed, limit != exact);
        assert_eq!(capacity, if ok { 128 } else { 64 });
        assert_eq!(length, if ok { 71 } else { 1 });
        assert_eq!(
            used,
            if ok {
                exact
            } else {
                std::mem::size_of::<Path>() + 32 + 64
            }
        );
    }
    let mut path = Path::new("é:😀").unwrap();
    let original = path.value.clone();
    let failure = child(&call(), format_args!(".arg.1"), &mut path, &mut |_, _| {
        Err(super::refusal())
    });
    assert!(failure.is_err());
    assert_eq!(path.value, original);
}
