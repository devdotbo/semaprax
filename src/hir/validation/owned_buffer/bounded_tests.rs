use super::*;
const SOURCE: &str = r#"module bounded;
@id("app.main") fn main()->i64{let request=0usize;let bytes=bytes_zeroed_bounded(request,64usize);i64_from_usize(byte_len(bytes_as_slice(bytes)))}
"#;
#[test]
fn bounded_allocation_replays_literal_authority_and_rejects_forged_bound() {
    let source = crate::check(SOURCE, "bounded.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let canonical = crate::format::canonical(&source);
    let roundtrip = crate::check(&canonical, "bounded-roundtrip.spx").unwrap();
    assert_eq!(
        crate::graph::to_json(&source).unwrap(),
        crate::graph::to_json(&roundtrip).unwrap()
    );
    for forged in [131073u64, u64::MAX] {
        let mut changed = program.clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("block")
        };
        let ResolvedStatement::Let { value, .. } = &mut statements[1] else {
            panic!("allocation")
        };
        let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
            panic!("call")
        };
        args[1].kind = ResolvedExprKind::Usize(forged);
        assert_eq!(crate::hir::validate(&changed).unwrap_err().code, "SPX-H006");
    }
    let mut changed = program.clone();
    let function = &mut changed.functions[0];
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("allocation")
    };
    let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
        panic!("call")
    };
    args[1] = args[0].clone();
    assert_eq!(crate::hir::validate(&changed).unwrap_err().code, "SPX-H006");
}
#[test]
fn bounded_allocation_charges_literal_payload_not_runtime_request() {
    let source = crate::check(SOURCE, "bounded-capacity.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let summary = crate::hir::byte_capacity::analyze_byte_data_capacity(&program).unwrap();
    let function = summary.function("app.main").unwrap();
    assert_eq!(function.bytes_copy_sites, 1);
    assert_eq!(function.owned_byte_payload_bytes, 64);
}

#[test]
fn inline_fixed_copy_array_views_replay_without_granting_computed_or_owned_views() {
    const SOURCE: &str = "module array_read;@id(\"main\") fn main()->i64 {let key=[65u8,66u8];let mut at=0usize;while at<2usize{let byte=byte_get(array_as_slice(key),at);at=at+1usize;0}i64_from_usize(at)}";
    let ast = crate::check(SOURCE, "inline-array-loop.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let canonical = crate::format::canonical(&ast);
    let roundtrip = crate::check(&canonical, "inline-array-loop-roundtrip.spx").unwrap();
    assert_eq!(
        crate::graph::to_json(&ast).unwrap(),
        crate::graph::to_json(&roundtrip).unwrap()
    );
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("body")
    };
    let body = statements[2].child(1).expect("while body");
    let ResolvedExprKind::Block { statements, .. } = &body.kind else {
        panic!("loop body")
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("read")
    };
    let ResolvedExprKind::Call { args, .. } = &value.kind else {
        panic!("byte_get")
    };
    let view = &args[0];
    let validator = HirValidator::new(&program).unwrap();
    validator.validate_while_admission(view).unwrap();
    let ResolvedExprKind::BorrowPlace { operation, place } = &view.kind else {
        panic!("named array view lowers to an authenticated BorrowPlace")
    };
    assert_eq!(operation.as_str(), crate::byte_ops::ARRAY_AS_SLICE_ID);
    let mut legacy_view = view.clone();
    let mut legacy_root = view.clone();
    legacy_root.kind = ResolvedExprKind::Place(place.clone());
    legacy_root.ty = ResolvedType::ArrayU8(2);
    legacy_root.ownership = OwnershipMode::Value;
    legacy_view.kind = ResolvedExprKind::Call {
        callee: operation.clone(),
        type_arguments: Vec::new(),
        instance: None,
        args: vec![legacy_root],
    };
    validator.validate_while_admission(&legacy_view).unwrap();
    // Retain every original forged Call-form control too.
    for hostile in 0..4 {
        let mut changed = legacy_view.clone();
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            args,
            ..
        } = &mut changed.kind
        else {
            panic!("array view")
        };
        match hostile {
            0 => *callee = DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
            1 => type_arguments.push(ResolvedType::Usize),
            2 => args[0].ownership = OwnershipMode::Own,
            3 => args[0].kind = ResolvedExprKind::ArrayU8(vec![65, 66]),
            _ => unreachable!(),
        }
        assert_eq!(
            validator
                .validate_while_admission(&changed)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    for hostile in 0..4 {
        let mut changed = view.clone();
        if hostile == 0 {
            changed.ownership = OwnershipMode::Own;
        } else if hostile == 1 {
            changed.ty = ResolvedType::Str;
        } else {
            let ResolvedExprKind::BorrowPlace { operation, place } = &mut changed.kind else {
                unreachable!()
            };
            if hostile == 2 {
                *operation = DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID);
            } else {
                place.root = ValueId::new("foreign.array.root".to_owned());
            }
        }
        assert_eq!(
            validator
                .validate_while_admission(&changed)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    let mut forged = program.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("body")
    };
    let body = match &mut statements[2] {
        ResolvedStatement::While { body, .. } => body,
        _ => panic!("while"),
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("loop body")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("read")
    };
    let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
        panic!("byte_get")
    };
    let ResolvedExprKind::BorrowPlace { place, .. } = &mut args[0].kind else {
        panic!("named array view")
    };
    place.root = ValueId::new("forged.array.owner".to_owned());
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
}
