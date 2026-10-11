use crate::{ast, hir};
const SOURCE: &str = "module constructor; @id(\"main\") fn main()->i64 {let rows=vec_of<i64>(19,23); if vec_len<i64>(rows)==2usize {42}else{0}}";
fn parsed() -> ast::Program {
    crate::check(SOURCE, "constructor.spx").unwrap()
}
fn marker(program: &ast::Program) -> &ast::Expr {
    let ast::ExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
        panic!()
    };
    let ast::Statement::Let { value, .. } = &statements[0] else {
        panic!()
    };
    value
}
fn marker_mut(program: &mut ast::Program) -> &mut ast::Expr {
    let ast::ExprKind::Block { statements, .. } = &mut program.functions[0].body.kind else {
        panic!()
    };
    let ast::Statement::Let { value, .. } = &mut statements[0] else {
        panic!()
    };
    value
}
#[test]
fn constructor_roundtrips_source_cache_and_selective_graph_without_a_second_result() {
    let source = parsed();
    let canonical = crate::format::canonical(&source);
    assert!(canonical.contains("vec_of<i64>(19, 23)"));
    let reparsed = crate::check(&canonical, "constructor.spx").unwrap();
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    let encoded = crate::cache_codec::encode(marker(&source)).unwrap();
    let decoded: ast::Expr = crate::cache_codec::decode(&encoded).unwrap();
    assert_eq!(&decoded, marker(&source));
    let typed = hir::resolve(&source).unwrap();
    hir::validate(&typed).unwrap();
    assert_eq!(
        crate::graph::graph_schema(&typed).unwrap(),
        "semaprax.graph.v81"
    );
    assert!(crate::graph::legacy_graph_schema(&typed).is_err());
    let graph = crate::graph::to_json(&source).unwrap();
    assert!(graph.contains("\"kind\":\"vec_constructor\""));
    assert!(graph.contains("\"count\":2"));
    let hir::ResolvedExprKind::Block { statements, .. } = &typed.functions[0].body.kind else {
        panic!()
    };
    let hir::ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!()
    };
    super::authenticate(value).unwrap();
    let encoded = crate::cache_codec::encode(value).unwrap();
    let decoded: hir::ResolvedExpr = crate::cache_codec::decode(&encoded).unwrap();
    assert_eq!(&decoded, value);
    let expansion = super::expansion(value).unwrap();
    assert_eq!(expansion.statements.len(), 3);
    assert_eq!(typed.functions[0].cleanup.slots.iter().filter(|slot| matches!(&slot.origin, crate::cleanup::CleanupStorageOrigin::Temporary { expression } if expression == &value.id)).count(), 1);
}
#[test]
fn constructor_parser_skips_later_authored_names_and_empty_constructor_is_capacity_zero() {
    let source = crate::check("module constructor; @id(\"main\") fn main()->i64 {let rows=vec_of<i64>(); let _if1=42; if vec_len<i64>(rows)==0usize {_if1}else{0}}", "hygiene.spx").unwrap();
    let expansion = ast::vec_constructor::inspect(marker(&source)).unwrap();
    assert_eq!(expansion.count, 0);
    let ast::Statement::Let { name, .. } = &expansion.statements[0] else {
        panic!()
    };
    assert_ne!(name, "_if1");
    assert!(crate::format::canonical(&source).contains("vec_of<i64>()"));
    hir::validate(&hir::resolve(&source).unwrap()).unwrap();
}
#[test]
fn constructor_forged_count_type_owner_and_mutability_refuse_before_projection() {
    for mutation in 0..4 {
        let mut source = parsed();
        let ast::ExprKind::VecConstructor {
            count,
            element,
            value,
        } = &mut marker_mut(&mut source).kind
        else {
            panic!()
        };
        let ast::ExprKind::Block { statements, tail } = &mut value.kind else {
            panic!()
        };
        match mutation {
            0 => *count += 1,
            1 => *element = ast::Type::Bool,
            2 => tail.kind = ast::ExprKind::Var("foreign".into()),
            _ => {
                let ast::Statement::Let { mutable, .. } = &mut statements[0] else {
                    panic!()
                };
                *mutable = true;
            }
        }
        assert!(ast::vec_constructor::inspect(marker(&source)).is_err());
        assert!(!crate::format::canonical(&source).contains("vec_of<"));
        assert!(crate::verify::verify(&source)
            .iter()
            .any(|d| d.code == "SPX-T318"));
    }
}
#[test]
fn constructor_forged_authored_item_reader_and_leading_zero_names_never_hide() {
    for leading_zero in [false, true] {
        let mut source = parsed();
        let ast::ExprKind::VecConstructor { value, .. } = &mut marker_mut(&mut source).kind else {
            panic!()
        };
        let ast::ExprKind::Block { statements, .. } = &mut value.kind else {
            panic!()
        };
        let ast::Statement::Let { name, .. } = &statements[0] else {
            panic!()
        };
        let name = name.clone();
        if leading_zero {
            let ast::Statement::Let { name, .. } = &mut statements[0] else {
                panic!()
            };
            *name = "_if01".into();
        } else {
            let ast::Statement::Let { value, .. } = &mut statements[1] else {
                panic!()
            };
            let ast::ExprKind::Call { args, .. } = &mut value.kind else {
                panic!()
            };
            args[1].kind = ast::ExprKind::Var(name);
        }
        assert!(!crate::format::canonical(&source).contains("vec_of<"));
        assert!(crate::verify::verify(&source)
            .iter()
            .any(|d| d.code == "SPX-T318"));
    }
}
#[test]
fn typed_constructor_rejects_chain_metadata_drift_with_original_canonical_cleanup_untouched() {
    let typed = hir::resolve(&parsed()).unwrap();
    for mutation in 0..4 {
        let mut forged = typed.clone();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut forged.functions[0].body.kind
        else {
            panic!()
        };
        let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!()
        };
        let hir::ResolvedExprKind::VecConstructor {
            count,
            element,
            statements,
            tail,
        } = &mut value.kind
        else {
            panic!()
        };
        match mutation {
            0 => *count += 1,
            1 => *element = hir::ResolvedType::Bool,
            2 => {
                let hir::ResolvedStatement::Let { mutable, .. } = &mut statements[1] else {
                    panic!()
                };
                *mutable = true;
            }
            _ => tail.ty = hir::ResolvedType::I64,
        }
        assert_eq!(
            forged.functions[0].cleanup_plan,
            typed.functions[0].cleanup_plan
        );
        assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
    }
}
#[test]
fn explicit_chain_does_not_gain_constructor_profile_and_old_array_diagnostic_remains() {
    let source = crate::check("module constructor; @id(\"main\") fn main()->i64 {let empty=vec_with_capacity<i64>(2usize); let first=vec_push<i64>(empty,19); let rows=vec_push<i64>(first,23); if vec_len<i64>(rows)==2usize {42}else{0}}", "chain.spx").unwrap();
    let typed = hir::resolve(&source).unwrap();
    hir::validate(&typed).unwrap();
    assert_ne!(
        crate::graph::graph_schema(&typed).unwrap(),
        "semaprax.graph.v81"
    );
    assert!(crate::check(
        "module constructor; @id(\"main\") fn main()->i64 {let x=[1,2];42}",
        "array.spx"
    )
    .unwrap_err()
    .iter()
    .any(|d| d.code == "SPX-T262"));
    assert_eq!(crate::parse("module constructor; @id(\"vec\") fn vec_of()->i64 {42} @id(\"main\") fn main()->i64 {42}", "reserved.spx").unwrap_err().code, "SPX-P220");
}

#[test]
fn marker_absence_preserves_ordinary_512_depth_and_overdepth_refuses() {
    let source = crate::check(
        "module depth; @id(\"main\") fn main()->i64 {0}",
        "depth.spx",
    )
    .unwrap();
    let mut typed = hir::resolve(&source).unwrap();
    for _ in 1..512 {
        let previous = typed.functions[0].body.clone();
        typed.functions[0].body.kind = hir::ResolvedExprKind::Unary {
            op: ast::UnaryOp::Neg,
            value: Box::new(previous),
        };
    }
    assert!(!super::program_uses(&typed).unwrap());
    let previous = typed.functions[0].body.clone();
    typed.functions[0].body.kind = hir::ResolvedExprKind::Unary {
        op: ast::UnaryOp::Neg,
        value: Box::new(previous),
    };
    assert_eq!(super::program_uses(&typed).unwrap_err().code, "SPX-H006");
}

#[test]
fn generated_expansion_reservation_is_exact_and_one_short_refuses() {
    let (result, refused, used) =
        crate::bounded_output::with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            crate::parse(SOURCE, "reservation.spx")
        });
    result.unwrap();
    assert!(!refused);
    assert!(used > 0);
    let (exact, refused, exact_used) =
        crate::bounded_output::with_limit_usage(used, || crate::parse(SOURCE, "reservation.spx"));
    exact.unwrap();
    assert!(!refused);
    assert_eq!(exact_used, used);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(used - 1, || {
        crate::parse(SOURCE, "reservation.spx")
    });
    assert!(refused);
    assert_eq!(short.unwrap_err().code, "SPX-P220");
}
#[test]
fn typed_item_cannot_read_a_generated_partial_owner() {
    let mut typed = hir::resolve(&parsed()).unwrap();
    hir::validate(&typed).unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut typed.functions[0].body.kind else {
        panic!()
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!()
    };
    let hir::ResolvedExprKind::VecConstructor { statements, .. } = &mut value.kind else {
        panic!()
    };
    let hir::ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!()
    };
    let generated = binding.id.clone();
    let hir::ResolvedStatement::Let { value: push, .. } = &mut statements[1] else {
        panic!()
    };
    let hir::ResolvedExprKind::Call { args, .. } = &mut push.kind else {
        panic!()
    };
    args[1].kind = hir::ResolvedExprKind::Place(hir::Place {
        root: generated,
        projections: Vec::new(),
    });
    assert_eq!(
        super::authenticate(value).unwrap_err().message,
        "resolved Vec constructor item captures a generated owner"
    );
}
#[test]
fn adopted_examples_preserve_exact_original_tasks_and_canonical_roundtrip() {
    let shiftsim = include_str!("../../../examples/shiftsim-typed-record-successor/src/tests.spx");
    let original = include_str!("tests/shiftsim_explicit_tests.txt");
    let start = original.find("    let servers0 =").unwrap();
    let end = original[start..].find("    let patients0 =").unwrap() + start;
    assert_eq!(shiftsim.replace("    let servers = vec_of<string>(\"S7\", \"S6\", \"S5\", \"S4\", \"S3\", \"S2\", \"S1\", \"S0\");\n", &original[start..end]), original);
    let catalog = include_str!("../../../examples/catalog-scoped-record-successor/src/tests.spx");
    let original = include_str!("tests/catalog_explicit_tests.txt");
    assert_eq!(catalog.replace("vec_of<InputItem>(first, second)", "vec_push<InputItem>(vec_push<InputItem>(vec_with_capacity<InputItem>(2usize), first), second)"), original);
    for source in [shiftsim, catalog] {
        let parsed = crate::parse(source, "adoption.spx").unwrap();
        let canonical = crate::format::canonical(&parsed);
        assert_eq!(
            crate::format::canonical(&crate::parse(&canonical, "adoption.spx").unwrap()),
            canonical
        );
    }
}
