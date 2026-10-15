use super::*;
const WRITER: &str = include_str!("../../../tests/owned_data/static_utf8_writer.spx");

#[test]
fn writer_literal_roots_round_trip_current_source_graph_and_cache() {
    let source = crate::check(WRITER, "static-writer.spx").unwrap();
    let canonical = crate::format::canonical(&source);
    let source = crate::check(&canonical, "static-writer.spx").unwrap();
    assert_eq!(crate::format::canonical(&source), canonical);
    let wire = crate::cache_codec::encode(&source).unwrap();
    let decoded: crate::ast::Program = crate::cache_codec::decode(&wire).unwrap();
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), wire);
    let program = crate::hir::resolve(&decoded).unwrap();
    assert!(crate::hir::validate_with_static_text(&program).unwrap());
    let wire = crate::cache_codec::encode(&program).unwrap();
    let decoded: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&decoded).unwrap();
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), wire);
    let graph = crate::graph::to_json(&source).unwrap();
    crate::graph::verify_json(&source, &graph).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v77");
    assert_eq!(document["prelude"]["schema"], "semaprax.prelude.v16");
    assert_eq!(document["static_borrowed_utf8"]["authority"], false);
    assert_eq!(
        document["static_borrowed_utf8"]["literals"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        document["byte_helper_renewal"]["schema"],
        "semaprax.byte-helper-renewal.v1"
    );
    let stale = crate::check(&canonical.replace("aé", "bé"), "static-writer.spx").unwrap();
    assert_eq!(
        crate::graph::verify_json(&stale, &graph).unwrap_err()[0].code,
        "SPX-G411"
    );
}

#[test]
fn decoded_pool_exact_limit_and_plus_one_preserve_owned_string_semantics() {
    let module = |text: &str, owned: &str| {
        format!("module literal.bound; @id(\"main\") fn main()->i64 {{let key=str\"{text}\"; {owned} str_len_bytes(key)}}")
    };
    let exact = crate::check(
        &module(&"x".repeat(MAX_POOL_BYTES), ""),
        "literal-bound.spx",
    )
    .unwrap();
    assert!(crate::hir::validate_with_static_text(&crate::hir::resolve(&exact).unwrap()).unwrap());
    let excess = crate::check(
        &module(&"x".repeat(MAX_POOL_BYTES + 1), ""),
        "literal-bound.spx",
    )
    .unwrap_err();
    assert!(excess.iter().any(|d| d.code == "SPX-T314"));
    let mixed = crate::check(
        &module(&"x".repeat(MAX_POOL_BYTES), "let owned=\"y\";"),
        "literal-bound.spx",
    )
    .unwrap();
    // Resolution includes mandatory HIR validation: the extra owned byte is
    // already refused there, rather than producing an invalid resolved module.
    let mixed_errors = crate::hir::resolve(&mixed).unwrap_err();
    assert!(mixed_errors.iter().any(|error| error.code == "SPX-H006"
        && error.message == "selected decoded text pool exceeds 65536 bytes"));
    let ordinary = crate::check(
        "module literal.ordinary; @id(\"main\") fn main()->i64 {let text=\"x\";string_len(text)}",
        "ordinary.spx",
    )
    .unwrap();
    assert!(
        !crate::hir::validate_with_static_text(&crate::hir::resolve(&ordinary).unwrap()).unwrap()
    );
    assert_ne!(
        crate::prelude::selected_for_program(&ordinary).0,
        crate::prelude::SCHEMA_V16
    );
}

#[test]
fn backing_reserves_exact_carrier_and_bytes_before_allocation() {
    let required = StaticUtf8::OWNED_ALLOCATION_CARRIER_BYTES + "é\0".len();
    let (value, refused, used) =
        crate::bounded_output::with_limit_usage(required, || StaticUtf8::from_text("é\0"));
    assert!(!refused);
    assert_eq!(used, required);
    assert_eq!(value.as_str(), "é\0");
    assert!(value.is_backed());
    let (value, refused, used) =
        crate::bounded_output::with_limit_usage(required - 1, || StaticUtf8::from_text("é\0"));
    assert!(refused);
    assert!(!value.is_backed());
    assert_eq!(used, 0);
}

#[test]
fn view_extent_and_mutability_are_independently_replayed_without_owner_slots() {
    let source = crate::check(WRITER, "static-root.spx").unwrap();
    let mut program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let run = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.writer")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &run.body.kind else {
        panic!("writer block")
    };
    let (first, producer) = statements
        .iter()
        .find_map(|statement| match statement {
            crate::hir::ResolvedStatement::Let { binding, value, .. }
                if binding.name == "first" =>
            {
                Some((&binding.id, &value.id))
            }
            _ => None,
        })
        .unwrap();
    let view = statements
        .iter()
        .find_map(|statement| match statement {
            crate::hir::ResolvedStatement::Let { binding, .. } if binding.name == "first_view" => {
                Some(&binding.id)
            }
            _ => None,
        })
        .unwrap();
    let provenance = program.declarations.byte_slice_roots.get(view).unwrap();
    assert_eq!(
        provenance.root_kind,
        crate::hir::ByteSliceRootKind::SourceExprStatic
    );
    assert_eq!(&provenance.root, first);
    assert_eq!(provenance.producer.as_ref(), Some(producer));
    assert_eq!(provenance.length, crate::hir::ByteSliceExtent::Constant(4));
    assert!(run.cleanup_plan.slots.iter().all(|slot| slot.storage
        != crate::cleanup_plan::StorageId::Value(first.clone())
        && slot.storage != crate::cleanup_plan::StorageId::Value(view.clone())));
    let view = view.clone();
    program
        .declarations
        .byte_slice_roots
        .get_mut(&view)
        .unwrap()
        .length = crate::hir::ByteSliceExtent::Constant(5);
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
    let mutable = crate::check(
        &WRITER.replace("let first=", "let mut first="),
        "mutable-literal.spx",
    )
    .unwrap_err();
    assert!(mutable.iter().any(|d| d.code == "SPX-T264"));
}

#[test]
fn interpreter_reentry_reads_static_roots_without_foreign_input_or_owned_slots() {
    let source = crate::check(WRITER, "static-interpreter.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    for _ in 0..4 {
        let evaluated =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&program, "app.main", 100_000)
                .unwrap();
        assert_eq!(
            evaluated.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        );
        assert!(evaluated.failure.is_none());
    }
    // A static root is module input, never an invocation's borrowed argument.
    let source = crate::check(
        "module literal.internal; @id(\"main\") fn main()->i64 {let key=str\"é\\u{0}\";let view=str_as_bytes(key);if byte_len(view)==3usize {42}else{0}}",
        "literal-internal.spx",
    ).unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    assert!(program
        .functions
        .iter()
        .all(|f| f.cleanup_plan.slots.is_empty()));
    let evaluated =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&program, "main", 100_000).unwrap();
    assert_eq!(
        evaluated.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    );
}

#[test]
fn adjacent_spelling_and_frozen_graph_do_not_coerce_ordinary_text() {
    let ordinary = crate::parse(
        "module literal.name; @id(\"main\") fn main()->i64 {let str=3;str}",
        "literal-name.spx",
    )
    .unwrap();
    assert!(!crate::format::canonical(&ordinary).contains("str\""));
    assert!(crate::parse(
        "module literal.space; @id(\"main\") fn main()->i64 {let key=str \"x\";0}",
        "literal-space.spx",
    )
    .is_err());
    let source = crate::check(WRITER, "static-frozen.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    assert_eq!(
        crate::graph::graph_schema_from_parts_and_instances(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
            &program.function_instances,
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let generic = crate::check(
        "module literal.template; @id(\"unused\") fn unused<T>(value:T)->i64 {let key=str\"x\";str_len_bytes(key)} @id(\"main\") fn main()->i64 {0}",
        "literal-template.spx",
    ).unwrap();
    let generic = crate::hir::resolve(&generic).unwrap();
    assert!(crate::hir::validate_with_static_text(&generic).unwrap());
    assert_eq!(generic.function_templates.len(), 1);
    assert_eq!(
        crate::graph::graph_schema_from_parts_and_instances(
            &generic.interfaces,
            &generic.types,
            &generic.functions,
            &generic.function_templates,
            &generic.function_instances,
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    // Static descriptor mutation cannot substitute String owning authority.
    let owned_argument = crate::check(
        "module literal.own; @id(\"take\") fn take(value:string)->i64 {string_len(value)} @id(\"main\") fn main()->i64 {let key=str\"x\";take(key)}",
        "literal-own.spx",
    ).unwrap_err();
    assert!(owned_argument.iter().any(|d| d.code == "SPX-T205"));
}
