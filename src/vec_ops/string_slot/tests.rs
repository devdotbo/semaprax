use crate::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedStatement,
    ResolvedType,
};
const SOURCE: &str = r#"module slots;
@id("slots.inspect") fn inspect(values:own Vec<string>)->i64 {
 let text=vec_str_at(values,0usize);
 let bytes=str_as_bytes(vec_str_at(values,0usize));
 str_len_bytes(text)
}
@id("slots.main") fn main()->i64 {0}
"#;
fn checked(source: &str) -> ResolvedProgram {
    hir::resolve(&crate::check(source, "slots.spx").unwrap()).unwrap()
}
fn first(program: &mut ResolvedProgram, index: usize) -> &mut ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "slots.inspect")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("body")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[index] else {
        panic!("view")
    };
    value
}
#[test]
fn slot_views_replay_exact_shape_loans_cache_and_selective_graph() {
    let ast = crate::check(SOURCE, "slots.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let reparsed = crate::check(&canonical, "slots.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&reparsed));
    let program = hir::resolve(&reparsed).unwrap();
    hir::validate(&program).unwrap();
    assert_eq!(
        crate::prelude::selected_for_program(&reparsed).0,
        crate::prelude::SCHEMA_V17
    );
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v80"
    );
    assert!(crate::graph::legacy_graph_schema(&program).is_err());
    let graph = crate::graph::to_json(&reparsed).unwrap();
    assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v80\""));
    crate::graph::verify_json(&reparsed, &graph).unwrap();
    assert!(crate::graph::verify_json(
        &reparsed,
        &graph.replacen("semaprax.graph.v80", "semaprax.graph.v79", 1)
    )
    .is_err());

    assert!(program
        .declarations
        .byte_slice_provenances()
        .any(
            |(_, p)| p.root_kind == hir::ByteSliceRootKind::OwnedVectorStringSlot
                && p.vector_field.is_none()
        ));
    let f = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "slots.inspect")
        .unwrap();
    assert!(f
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.origin.projections.is_empty() && !loan.end_edges.is_empty()));
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    assert_eq!(wire, crate::cache_codec::encode(&restored).unwrap());
}
#[test]
fn source_slot_views_refuse_type_arguments_temporary_projection_and_escape() {
    for (case, source) in [
        SOURCE.replace(
            "vec_str_at(values,0usize)",
            "vec_str_at<string>(values,0usize)",
        ),
        SOURCE.replace(
            "vec_str_at(values,0usize)",
            "vec_str_at(vec_with_capacity<string>(1usize),0usize)",
        ),
        SOURCE.replace("Vec<string>", "Vec<i64>"),
        SOURCE.replace("vec_str_at(values,", "vec_str_at(values.items,"),
        SOURCE.replace("@id(\"slots.inspect\")", "@id(\"slots.take\") fn take(values:own Vec<string>)->usize {0usize}\n@id(\"slots.inspect\")").replace("vec_str_at(values,0usize)","vec_str_at(values,take(values))"),
        SOURCE.replace(
            "let bytes=str_as_bytes(vec_str_at(values,0usize));",
            "let moved=values;",
        ),
    ].into_iter().enumerate() {
        let errors = crate::check(&source, "bad.spx").unwrap_err();
        if case < 4 { assert!(errors.iter().any(|error| error.code == "SPX-T281"), "{errors:?}"); }
    }
    let escape = SOURCE
        .replace("->i64 {\n let text", "->str {\n let text")
        .replace("str_len_bytes(text)", "text");
    assert!(crate::check(&escape, "escape.spx").is_err());
}
#[test]
fn slot_hir_type_identity_root_result_and_index_forgeries_are_rejected() {
    let original = checked(SOURCE);
    for mutation in 0..7 {
        let mut program = original.clone();
        let expression = first(&mut program, 0);
        match mutation {
            0 => {
                let ResolvedExprKind::Call { type_arguments, .. } = &mut expression.kind else {
                    panic!()
                };
                type_arguments.clear();
            }
            1 => {
                let ResolvedExprKind::Call { type_arguments, .. } = &mut expression.kind else {
                    panic!()
                };
                type_arguments[0] = ResolvedType::I64;
            }
            2 => expression.ownership = OwnershipMode::Own,
            3 => {
                let ResolvedExprKind::Call { callee, .. } = &mut expression.kind else {
                    panic!()
                };
                *callee = hir::DeclarationId::new(super::BYTES_ID);
            }
            4 => {
                let ResolvedExprKind::Call { args, .. } = &mut expression.kind else {
                    panic!()
                };
                args[1].ty = ResolvedType::Bool;
            }
            5 => {
                let ResolvedExprKind::Call { args, .. } = &mut expression.kind else {
                    panic!()
                };
                let ResolvedExprKind::Place(place) = &mut args[0].kind else {
                    panic!()
                };
                place
                    .projections
                    .push(hir::PlaceProjection::Field(hir::DeclarationId::new(
                        "foreign.field",
                    )));
            }
            6 => {
                let ResolvedExprKind::Call { args, .. } = &mut expression.kind else {
                    panic!()
                };
                args.swap(0, 1);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            hir::validate(&program).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
        let wire = crate::cache_codec::encode(&program).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert!(hir::validate(&restored).is_err());
    }
}
#[test]
fn fused_slot_view_keeps_inner_child_identities_and_rejects_outer_relabel() {
    let mut program = checked(SOURCE);
    let expression = first(&mut program, 1);
    let slot = super::inspect(expression).unwrap();
    assert!(slot.bytes);
    assert!(slot.source.id.as_str().contains(".arg.0.arg.0"));
    assert!(slot.index.id.as_str().contains(".arg.0.arg.1"));
    let ResolvedExprKind::Call { args, .. } = &mut expression.kind else {
        panic!()
    };
    args[0].id = expression.id.clone();
    assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
}
#[test]
fn removal_of_required_owner_freeze_fails_independent_loan_replay() {
    let mut program = checked(SOURCE);
    let f = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "slots.inspect")
        .unwrap();
    assert!(!f.loan_plan.loans.is_empty());
    f.loan_plan.loans.clear();
    assert!(crate::loan_plan::validate_program(&program).is_err());
}
#[test]
fn unchanged_scalar_program_keeps_older_prelude_and_graph() {
    let source = "module old; @id(\"old.main\") fn main()->i64{0}";
    let ast = crate::check(source, "old.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    assert_ne!(
        crate::prelude::selected_for_program(&ast).0,
        crate::prelude::SCHEMA_V17
    );
    assert_ne!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v80"
    );
}
