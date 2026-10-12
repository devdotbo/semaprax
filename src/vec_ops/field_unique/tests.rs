use super::*;
const SOURCE: &str = "module unique; @id(\"unique.row\") record Row { @id(\"unique.row.id\") id:string, @id(\"unique.row.rank\") rank:i64, @id(\"unique.row.score\") score:f64, @id(\"unique.row.blob\") blob:Bytes, } @id(\"unique.main\") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); let unique=vec_unique_by<Row>(rows,\"id\"); if unique {0} else {1} }";
const COPY: &str = "module copy; @id(\"copy.row\") record Row { @id(\"copy.row.rank\") rank:i64, @id(\"copy.row.mark\") mark:char, @id(\"copy.row.weight\") weight:f32, } @id(\"copy.main\") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); let unique=vec_unique_by<Row>(rows,\"mark\"); if unique {0} else {1} }";
fn resolved(source: &str) -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "unique.spx").unwrap()).unwrap()
}
fn call<'a>(
    program: &'a mut crate::hir::ResolvedProgram,
    id: &str,
) -> &'a mut crate::hir::ResolvedExpr {
    let main = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == id)
        .unwrap();
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("block")
    };
    let crate::hir::ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("let")
    };
    value
}
fn refused(source: &str, code: &str) -> bool {
    crate::check(source, "bad.spx")
        .err()
        .is_some_and(|errors| errors.iter().any(|d| d.code == code))
}

#[test]
fn literal_descriptor_canonical_graph_cache_replay_and_selective_prelude() {
    for (source, id) in [(SOURCE, "unique.main"), (COPY, "copy.main")] {
        let ast = crate::check(source, "unique.spx").unwrap();
        let canonical = crate::format::canonical(&ast);
        assert_eq!(
            crate::format::canonical(&crate::check(&canonical, "unique.spx").unwrap()),
            canonical
        );
        assert!(canonical.contains("vec_unique_by<Row>("));
        assert_eq!(
            crate::prelude::selected_for_program(&ast).0,
            crate::prelude::SCHEMA_V18
        );
        let graph = crate::graph::to_json(&ast).unwrap();
        crate::graph::verify_json(&ast, &graph).unwrap();
        assert!(graph.contains(PREFIX));
        assert!(graph.contains("semaprax.prelude.v18"));
        assert!(crate::graph::verify_json(
            &ast,
            &graph.replace("semaprax.prelude.v18", "semaprax.prelude.v17")
        )
        .is_err());
        let mut program = resolved(source);
        crate::hir::validate(&program).unwrap();
        let expression = call(&mut program, id).clone();
        let used = inspect(&expression).unwrap();
        assert_eq!(expression.ty, ResolvedType::Bool);
        assert_eq!(expression.ownership, OwnershipMode::Value);
        assert!(borrowed_source(&expression, 0) && !borrowed_source(&expression, 1));
        assert!(position(&program.declarations, used.element, used.identity).is_some());
        let wire = crate::cache_codec::encode(&program).unwrap();
        let restored: crate::hir::ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        crate::hir::validate(&restored).unwrap();
        assert_eq!(wire, crate::cache_codec::encode(&restored).unwrap());
    }
    // Programs without the operation keep their exact older selection.
    let legacy = crate::check(
        &SOURCE.replace("vec_unique_by<Row>(rows,\"id\")", "vec_len<Row>(rows)==0usize"),
        "old.spx",
    )
    .unwrap();
    assert_ne!(
        crate::prelude::selected_for_program(&legacy).0,
        crate::prelude::SCHEMA_V18
    );
    assert!(!crate::prelude::field_unique::contract_bytes().is_empty());
}

#[test]
fn independent_source_and_resolved_derivations_agree_and_close_the_key_profile() {
    let ast = crate::check(SOURCE, "unique.spx").unwrap();
    let program = resolved(SOURCE);
    let row = ast.types.iter().find(|t| t.name == "Row").unwrap();
    let element = ResolvedType::Nominal {
        declaration: DeclarationId::new("unique.row"),
        arguments: vec![],
    };
    for field in ["id", "rank"] {
        let from_source = source(row, field).unwrap();
        assert_eq!(
            resolve(&program.declarations, &element, field),
            Some(from_source.clone())
        );
        let identity = from_source.identity().unwrap();
        assert_eq!(decode(&identity), Some(from_source.clone()));
        assert_eq!(
            replay(&program.declarations, &element, &identity),
            Some(from_source)
        );
    }
    // Floating and Bytes keys stay outside v1; selectors are one identifier.
    for field in ["score", "blob", "missing", "", "id:asc", "rank,id", "1id", "i d"] {
        assert!(source(row, field).is_none(), "{field}");
        assert!(resolve(&program.declarations, &element, field).is_none(), "{field}");
    }
    assert!(!literal(&"a".repeat(MAX_FIELD_BYTES + 1)));
    // A reused sort selector never authorizes the distinct uniqueness opcode.
    let sorted = crate::vec_ops::sort_keys::resolve(&program.declarations, &element, "id:asc")
        .unwrap()
        .identity()
        .unwrap();
    assert!(decode(&sorted).is_none());
    assert!(replay(&program.declarations, &element, &sorted).is_none());
}

#[test]
fn dynamic_missing_foreign_float_bytes_generic_temporary_and_moved_roots_refuse() {
    for (from, to) in [
        ("\"id\")", "\"score\")"),
        ("\"id\")", "\"blob\")"),
        ("\"id\")", "\"missing\")"),
        ("\"id\")", "\"id:asc\")"),
        ("\"id\")", "\"\")"),
        ("\"id\")", "\"rank,id\")"),
        (
            "let unique=vec_unique_by<Row>(rows,\"id\");",
            "let field=\"id\"; let unique=vec_unique_by<Row>(rows,field);",
        ),
        (
            "vec_unique_by<Row>(rows,\"id\")",
            "vec_unique_by<Row>(vec_with_capacity<Row>(0usize),\"id\")",
        ),
        ("vec_unique_by<Row>(rows,\"id\")", "vec_unique_by<Row>(rows)"),
        (
            "vec_unique_by<Row>(rows,\"id\")",
            "vec_unique_by<Row>(rows,\"id\",\"rank\")",
        ),
        ("vec_unique_by<Row>(rows,\"id\")", "vec_unique_by<i64>(rows,\"id\")"),
        (
            "let unique=vec_unique_by<Row>(rows,\"id\");",
            "let moved=rows; let unique=vec_unique_by<Row>(rows,\"id\");",
        ),
        (
            "@id(\"unique.row.rank\") rank:i64,",
            "rank:i64,",
        ),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE, "{to}");
        assert!(refused(&source, "SPX-T319"), "{to}");
    }
    let legacy = "module legacy; @id(\"legacy.row\") record Row { @id(\"legacy.row.a\") a:Bytes, @id(\"legacy.row.b\") b:Bytes, @id(\"legacy.row.n\") n:i64, } @id(\"legacy.main\") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); if vec_unique_by<Row>(rows,\"n\") {0} else {1} }";
    assert!(refused(legacy, "SPX-T319"));
    let generic = format!(
        "{SOURCE} @id(\"unique.helper\") fn helper<T>(values: borrow Vec<T>) -> bool {{ vec_unique_by<T>(values,\"id\") }}"
    );
    assert!(crate::check(&generic, "generic.spx").is_err());
    let mut spoof = SOURCE.to_owned();
    spoof.push_str(&format!(" @id(\"{PREFIX}forged\") fn helper()->i64 {{0}}"));
    assert!(refused(&spoof, "SPX-S113"));
}

#[test]
fn stale_foreign_layout_float_root_result_and_cached_descriptors_refuse() {
    let mut original = resolved(SOURCE);
    let crate::hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        ..
    } = &call(&mut original, "unique.main").kind
    else {
        panic!("call")
    };
    let id = callee.as_str().to_owned();
    let element = type_arguments[0].clone();
    let descriptor = decode(&id).unwrap();
    let mut foreign = descriptor.clone();
    foreign.field = DeclarationId::new("foreign");
    let mut float = descriptor.clone();
    float.field = DeclarationId::new("unique.row.score");
    let mut layout = descriptor.clone();
    layout.layout.push('x');
    let other = resolve(&original.declarations, &element, "rank")
        .unwrap()
        .identity()
        .unwrap();
    for forged in [
        format!("{id}trailing"),
        String::from(PREFIX),
        foreign.identity().unwrap(),
        float.identity().unwrap(),
        layout.identity().unwrap(),
    ] {
        assert!(replay(&original.declarations, &element, &forged).is_none());
        let mut bad = original.clone();
        let crate::hir::ResolvedExprKind::Call { callee, .. } =
            &mut call(&mut bad, "unique.main").kind
        else {
            panic!()
        };
        *callee = DeclarationId::new(forged);
        assert!(crate::hir::validate(&bad).is_err());
        if let Ok(wire) = crate::cache_codec::encode(&bad) {
            if let Ok(restored) = crate::cache_codec::decode::<crate::hir::ResolvedProgram>(&wire) {
                assert!(crate::hir::validate(&restored).is_err());
            }
        }
    }
    // A different valid field is independently valid HIR; source replay owns
    // the binding to this literal (workspace expected-projection control).
    let mut substituted = original.clone();
    let crate::hir::ResolvedExprKind::Call { callee, .. } =
        &mut call(&mut substituted, "unique.main").kind
    else {
        panic!()
    };
    *callee = DeclarationId::new(other);
    crate::hir::validate(&substituted).unwrap();
    for mutate in [0, 2, 3] {
        let mut bad = original.clone();
        let expression = call(&mut bad, "unique.main");
        match mutate {
            0 => expression.ty = ResolvedType::I64,
            2 => {
                let crate::hir::ResolvedExprKind::Call { type_arguments, .. } =
                    &mut expression.kind
                else {
                    panic!()
                };
                type_arguments[0] = ResolvedType::I64;
            }
            _ => {
                let crate::hir::ResolvedExprKind::Call { args, .. } = &mut expression.kind else {
                    panic!()
                };
                let source = args[0].clone();
                args.push(source);
            }
        }
        assert!(crate::hir::validate(&bad).is_err(), "{mutate}");
    }
}

#[test]
fn length_bound_canonical_decimal_and_truncation_are_unforgeable() {
    assert!(decode(&format!("{PREFIX}01:r6:layout1:f")).is_none());
    let descriptor = Descriptor {
        element: "r".into(),
        layout: "layout".into(),
        field: DeclarationId::new("field"),
    };
    let id = descriptor.identity().unwrap();
    assert_eq!(decode(&id), Some(descriptor));
    for end in 0..id.len() {
        assert!(decode(&id[..end]).is_none());
    }
    let huge = Descriptor {
        element: "r".into(),
        layout: "x".repeat(MAX_DESCRIPTOR_BYTES),
        field: DeclarationId::new("f"),
    };
    assert!(huge.identity().is_none());
}
