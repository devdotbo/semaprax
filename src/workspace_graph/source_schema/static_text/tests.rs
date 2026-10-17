//! Retained authority is independently replayed, never inferred from v77.
use super::*;
mod paid_report;
mod retained_lexical_composition;
const WRITER: &str = include_str!("../../../../tests/owned_data/static_utf8_writer.spx");
fn build(source: &str) -> WorkspaceGraphBuild {
    let sources = [
        ("writer.spx", source),
        (
            "support.spx",
            "module support.ready; @id(\"ready\") fn ready()->i64 {0}",
        ),
    ]
    .into_iter()
    .map(|(path, source)| WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&crate::parse(source, path).unwrap()),
    })
    .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .unwrap()
        .0
}
fn writer(build: &WorkspaceGraphBuild) -> &WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "static.writer")
        .unwrap()
}
fn writer_mut(build: &mut WorkspaceGraphBuild) -> &mut WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter_mut()
        .find(|module| module.module == "static.writer")
        .unwrap()
}
fn refused(build: &WorkspaceGraphBuild) {
    let errors = super::super::schema(writer(build), &build.hir.modules, &build.hir.declarations)
        .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G410"));
}
#[test]
fn retained_writer_replays_source_sites_and_composes_helper_renewal() {
    let build = build(WRITER);
    let module = writer(&build);
    assert_eq!(
        schema(module, &build.hir.declarations).unwrap(),
        Some("semaprax.graph.v77")
    );
    assert_eq!(
        build.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v77"
    );
    assert_eq!(module.static_text.as_ref().unwrap().sites.len(), 2);
    assert!(module.functions.iter().any(
        |function| function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    ));
    assert_eq!(
        graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &module.types,
            &module.functions,
            &module.function_templates,
            &module.function_instances
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let changed = build_source_changed();
    assert_ne!(
        module.static_text.as_ref().unwrap().sites[0].text.as_str(),
        writer(&changed).static_text.as_ref().unwrap().sites[0]
            .text
            .as_str()
    );
}
fn build_source_changed() -> WorkspaceGraphBuild {
    build(&WRITER.replace("aé", "bé"))
}
#[test]
fn source_inventory_content_identity_namespace_and_presence_fail_closed() {
    for attack in 0..6 {
        let mut build = build(WRITER);
        let module = writer_mut(&mut build);
        let inventory = module.static_text.as_mut().unwrap();
        match attack {
            0 => inventory.sites[0].text = hir::StaticUtf8::from_text("forged"),
            1 => inventory.sites[0].path.push('x'),
            2 => inventory.sites[0].expression = module.functions[0].body.id.clone(),
            3 => inventory.path.push('x'),
            4 => inventory.sites[0].explicit = !inventory.sites[0].explicit,
            _ => module.static_text = None,
        }
        refused(&build);
    }
}
#[test]
fn retained_generic_literals_match_template_and_concrete_execution_sites() {
    let source = "module static.writer; @id(\"generic\") fn generic<T>(value:T)->T {let key=str\"é\";let second=str\"x\";value} @id(\"app.main\") fn main()->i64 {generic<i64>(42)}";
    let mut build = build(source);
    let module = writer(&build);
    assert_eq!(
        schema(module, &build.hir.declarations).unwrap(),
        Some("semaprax.graph.v77")
    );
    assert_eq!(module.function_templates.len(), 1);
    assert!(!module.function_instances.is_empty());
    let module = writer_mut(&mut build);
    let replacement = module.functions[0].body.id.clone();
    let instance = &mut module.function_instances[0].function;
    let hir::ResolvedExprKind::Block { statements, .. } = &mut instance.body.kind else {
        panic!("generic body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("generic literal")
    };
    value.id = replacement;
    refused(&build);
    let mut duplicate = self::build(source);
    let module = writer_mut(&mut duplicate);
    let hir::ResolvedExprKind::Block { statements, .. } =
        &mut module.function_instances[0].function.body.kind
    else {
        panic!("generic body")
    };
    let hir::ResolvedStatement::Let { value: first, .. } = &statements[0] else {
        panic!("first literal")
    };
    let repeated = first.clone();
    let hir::ResolvedStatement::Let { value: second, .. } = &mut statements[1] else {
        panic!("second literal")
    };
    *second = repeated;
    // Even exact duplicate ID/span/content cannot substitute for the omitted
    // second source site; this selector proves its own per-instance bijection.
    refused(&duplicate);
}

const LEXICAL_SOURCE: &str = r#"module static.writer;
@id("static.lexical") fn run()->i64 {
 let key=str"x"; let mut output=bytes_zeroed(1usize); let mut at=0;
 while at<3 {
  output=bytes_set(output,0usize,65u8); at=at+1;
  if at==1 {continue;}; 0
 }
 return str_len_bytes(key);
}
@id("app.main") fn main()->i64 {run()}
"#;
#[test]
fn lexical_byte_renewal_has_direct_proof_and_retained_composition_stays_closed() {
    let source = LEXICAL_SOURCE;
    let ast = crate::check(source, "writer.spx").unwrap();
    let canonical = format::canonical(&ast);
    let ast = crate::check(&canonical, "writer.spx").unwrap();
    assert_eq!(format::canonical(&ast), canonical);
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.lexical")
        .unwrap();
    assert!(crate::byte_ops::requires_same_owner_set(function));
    assert_eq!(
        function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
    );
    assert!(hir::lexical_exit::requires(function).unwrap());
    let graph = graph::to_json(&ast).unwrap();
    graph::verify_json(&ast, &graph).unwrap();
    let changed = crate::check(&canonical.replace("str\"x\"", "str\"y\""), "writer.spx").unwrap();
    assert_eq!(
        graph::verify_json(&changed, &graph).unwrap_err()[0].code,
        "SPX-G411"
    );
    for attack in 0..5 {
        let mut candidate = resolved.clone();
        let function = candidate
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        let unrelated = function.body.id.clone();
        match attack {
            0 => function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17,
            1 => {
                function
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .loops[0]
                    .condition = unrelated.clone()
            }
            _ => {
                let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind
                else {
                    panic!("block")
                };
                let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
                    panic!("literal")
                };
                match attack {
                    2 => value.ty = hir::ResolvedType::String,
                    3 => value.ownership = hir::OwnershipMode::Own,
                    _ => value.id = unrelated.clone(),
                }
            }
        }
        assert_eq!(hir::validate(&candidate).unwrap_err().code, "SPX-H006");
    }
    // The legacy route without an authenticated primitive source inventory
    // remains closed: v18 presence itself grants no authority.
    let mut retained = build(&canonical);
    // The legacy detached route has no source-derived primitive declaration context.
    writer_mut(&mut retained)
        .static_text
        .as_mut()
        .unwrap()
        .lexical = None;
    let errors = retained
        .source_graph_schemas()
        .expect_err("retained composite schema route remains explicitly closed");
    assert!(errors.iter().any(|error| error.code == "SPX-G410"));
}

#[test]
fn retained_primitive_static_lexical_replays_graph_and_runtime() {
    let retained = build(LEXICAL_SOURCE);
    assert_eq!(
        schema(writer(&retained), &retained.hir.declarations).unwrap(),
        Some("semaprax.graph.v77"),
        "exact literal-site authentication precedes primitive lexical replay"
    );
    assert_eq!(
        retained.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v77"
    );
    let ast = crate::check(LEXICAL_SOURCE, "writer.spx").unwrap();
    let canonical = format::canonical(&ast);
    let current = crate::check(&canonical, "writer.spx").unwrap();
    assert_eq!(format::canonical(&current), canonical);
    let resolved = hir::resolve(&current).unwrap();
    let evaluated =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&resolved, "app.main", 10000).unwrap();
    assert!(matches!(
        evaluated.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
}

#[test]
fn retained_primitive_lexical_rejects_source_signature_schema_and_certificate_drift() {
    for attack in 0..10 {
        let mut retained = build(LEXICAL_SOURCE);
        let module = writer_mut(&mut retained);
        let function = module
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        match attack {
            0 => function.name.push('x'),
            1 => function.return_type = hir::ResolvedType::Bool,
            2 => function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19,
            3 => {
                function
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .exits[0]
                    .site = function.body.id.clone()
            }
            4 => module
                .static_text
                .as_mut()
                .unwrap()
                .lexical
                .as_mut()
                .unwrap()
                .functions[0]
                .name
                .push('x'),
            _ => {
                // These preserve the captured source and function signatures.
                // Every expected body digest remains private and unchanged.
                let condition = match &function.body.kind {
                    hir::ResolvedExprKind::Block { statements, .. } => statements
                        .iter()
                        .find_map(|statement| match statement {
                            hir::ResolvedStatement::LexicalWhile { condition, .. } => {
                                Some(condition.as_ref().clone())
                            }
                            _ => None,
                        })
                        .unwrap(),
                    _ => panic!("source block"),
                };
                let main = module
                    .functions
                    .iter_mut()
                    .find(|f| f.id.as_str() == "app.main")
                    .unwrap();
                match attack {
                    5 => main.body.kind = hir::ResolvedExprKind::Int(7),
                    6 => {
                        let hir::ResolvedExprKind::Block { tail, .. } = &mut main.body.kind else {
                            panic!("source main block")
                        };
                        let hir::ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
                            panic!("source call")
                        };
                        *callee = main.id.clone();
                    }
                    7 => main.requires.push(condition),
                    8 => main.body.ty = hir::ResolvedType::Bool,
                    _ => main.span.end = main.span.end.checked_add(1).unwrap(),
                }
            }
        }
        refused(&retained);
    }
}

// The complete old selector is independently compiled, with one name change.
// It documents the genuine prior widening; it is never used for admission.
mod missing_lexical_authority {
    use super::*;
    use super::super::super::*;
fn before_select<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut byte_helper::Proofs<'a>,
    static_schema: Option<&'static str>,
    lexical_static: bool,
) -> Result<&'static str, Vec<Diagnostic>> {
    let helper_schema = byte_helper::schema_with_proofs(module, modules, facts, proofs)?;
    let mut string_slots = false;
    for function in module.functions.iter().chain(
        module
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            hir::visit_resolved_calls(root, &mut |callee, _, _| {
                string_slots |= matches!(
                    callee.as_str(),
                    crate::vec_ops::string_slot::ID | crate::vec_ops::string_slot::BYTES_ID
                );
            });
        }
    }
    let finite_scratch = module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::finite_byte_scratch::function_requires);
    let admission = |operation: crate::vec_ops::VecOp, element: &hir::ResolvedType| {
        if crate::vec_ops::resolved_element_is_admitted(element)
            && crate::vec_ops::resolved_operation_element_is_admitted(operation, element)
        {
            return Some(hir::OwnershipMode::Value);
        }
        if *element == hir::ResolvedType::String && operation.admits_owned_leaf()
            || *element == hir::ResolvedType::Bytes
                && crate::vec_ops::resolved_operation_element_is_admitted(operation, element)
        {
            return Some(hir::OwnershipMode::Own);
        }
        let mode = record_mode(element, module, modules, facts)?;
        match mode {
            hir::OwnershipMode::Value if !operation.owned_leaf_only() => Some(mode),
            hir::OwnershipMode::Own if operation.admits_owned_leaf() => Some(mode),
            _ => None,
        }
    };
    let authority = |function: &hir::ResolvedFunction| {
        hir::vec_loop_renewal::requires_with_admission(function, admission)
    };
    let consuming = module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(|function| {
            hir::vec_loop_renewal::consuming::requires_with_admission(function, admission)
        });
    // Static modules independently replay the other renewal obligations too.
    if static_schema.is_none() && !consuming && !string_slots {
        if finite_scratch {
            return Ok("semaprax.graph.v78");
        }
        if let Some(schema) = helper_schema {
            return Ok(schema);
        }
    }
    for function in module.functions.iter().chain(
        module
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        if lexical_static && hir::lexical_exit::requires(function).map_err(|error| vec![error])? {
            continue;
        }
        if helper_schema.is_some()
            && function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
        {
            continue;
        }
        let lexical = hir::lexical_exit::requires(function).map_err(|error| vec![error])?;
        if (function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18) != lexical
        {
            return Err(vec![graph_error(
                "SPX-G410",
                "lexical cleanup schema disagrees with authored exits",
            )]);
        }
        let byte = !lexical && crate::byte_ops::requires_same_owner_set(function);
        let string = !lexical && !byte && crate::string_ops::replacement::requires(function);
        let ordinary = !lexical && !byte && !string && authority(function);
        for (schema, expected, message) in [
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17,
                byte,
                "byte-buffer renewal cleanup schema disagrees",
            ),
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16,
                string,
                "String replacement cleanup schema disagrees",
            ),
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15,
                ordinary,
                "ordinary Vec renewal cleanup schema disagrees",
            ),
        ] {
            if (function.cleanup_plan.schema == schema) != expected {
                return Err(vec![graph_error("SPX-G410", message)]);
            }
        }
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(|f| {
            std::iter::once(&f.body)
                .chain(&f.requires)
                .chain(&f.ensures)
                .any(crate::vec_field::expression_uses)
        })
    {
        return Ok("semaprax.graph.v85");
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::hir::copy_discard_function_uses)
    {
        return Ok("semaprax.graph.v84");
    }
    // Byte traversal is the outer v83 layer, preserving row-loan v82 facts.
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::hir::byte_traversal_function_uses)
    {
        return Ok("semaprax.graph.v83");
    }
    if graph::row_loan::functions_require(
        module
            .functions
            .iter()
            .chain(module.function_instances.iter().map(|i| &i.function)),
    ) {
        return Ok(graph::row_loan::SCHEMA);
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .try_fold(false, |found, f| {
            crate::hir::vec_constructor::function_uses(f).map(|uses| found || uses)
        })
        .map_err(|e| vec![e])?
    {
        return Ok("semaprax.graph.v81");
    }
    if string_slots {
        return Ok("semaprax.graph.v80");
    }
    if consuming {
        return Ok("semaprax.graph.v79");
    }
    if let Some(schema) = static_schema {
        return Ok(if finite_scratch {
            "semaprax.graph.v78"
        } else {
            schema
        });
    }
    reserve_builder_structure(nominal_context::RECEIVING_CONTROLS)?;
    let types = nominal_context::Types {
        module,
        modules,
        facts,
    };
    if !module
        .functions
        .iter()
        .chain(
            module
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| authority(function) && !hir::vec_loop_renewal::requires(function))
    {
        return graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &types,
            &module.functions,
            &module.function_templates,
            &module.function_instances,
        )
        .map_err(|error| vec![error]);
    }
    graph::graph_schema_from_parts_and_instances_with_renewal_authority(
        &module.interfaces,
        &types,
        &module.functions,
        &module.function_templates,
        &module.function_instances,
        Some(&authority),
    )
    .map_err(|error| vec![error])
}

    #[test]
    fn absent_private_lexical_inventory_cannot_inherit_static_or_helper_schema_authority() {
        let mut build = build(LEXICAL_SOURCE);
        assert_eq!(build.source_graph_schemas().unwrap()["writer.spx"], "semaprax.graph.v77");
        writer_mut(&mut build).static_text.as_mut().unwrap().lexical = None;
        let module = writer(&build);
        let modules = &build.hir.modules;
        let facts = &build.hir.declarations;
        let static_schema = schema(module, facts).unwrap();
        assert_eq!(static_schema, Some("semaprax.graph.v77"));
        assert!(module.functions.iter().any(|f| hir::lexical_exit::requires(f).unwrap()));
        assert_eq!(before_select(module, modules, facts, &mut byte_helper::Proofs::new(modules, facts), static_schema, false).unwrap(), "semaprax.graph.v77");
        let expected = REPORT.legacy();
        assert_eq!(super::super::super::select(module, modules, facts, &mut byte_helper::Proofs::new(modules, facts), static_schema, false).unwrap_err(), expected, "complete six fixed public fields, never version-derived authority");
        assert_eq!(build.source_graph_schemas().unwrap_err(), expected);
        // Replay remains mandatory even when a forged member advertises a
        // helper cleanup generation. Its genuine authored lexical body wins.
        for function in writer_mut(&mut build).functions.iter_mut() {
            if hir::lexical_exit::requires(function).unwrap() {
                function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19;
            }
        }
        assert!(build.source_graph_schemas().is_err());
    }

    #[test]
    fn authenticated_lexical_and_nonlexical_static_positive_routes_keep_their_own_authority() {
        for source in [LEXICAL_SOURCE, WRITER] {
            let build = build(source);
            assert_eq!(build.source_graph_schemas().unwrap()["writer.spx"], "semaprax.graph.v77");
        }
        for attack in 0..2 {
            let mut build = build(LEXICAL_SOURCE);
            writer_mut(&mut build).static_text.as_mut().unwrap().lexical = None;
            if attack == 0 {
                writer_mut(&mut build).module.push_str(".foreign");
            } else {
                let f = writer_mut(&mut build).functions.iter_mut().find(|f| f.id.as_str() == "static.lexical").unwrap();
                f.return_type = hir::ResolvedType::Bool;
            }
            assert!(build.source_graph_schemas().unwrap_err().iter().any(|e| e.code == "SPX-G410"));
        }
    }
}
