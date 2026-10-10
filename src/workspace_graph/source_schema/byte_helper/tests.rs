use super::*;

const LOCAL: &str = r#"module app.main;
@id("fill") fn fill(output:own Bytes)->Bytes {
 let mut next=output; next=bytes_set(next,0usize,65u8); next
}
@id("main") fn main()->i64 {
 let mut output=bytes_zeroed(1usize); output=fill(output); 7
}
"#;
const APP: &str = r#"module app.main;
use type @id("row") from lib.leaf as Row;
use function @id("bridge") from lib.bridge as fill;
@id("main") fn main()->i64 {
 let row=Row{label:"meta",index:0usize};
 let mut output=bytes_zeroed(1usize); output=fill(output,row); 7
}
"#;
const BRIDGE: &str = r#"module lib.bridge;
use type @id("row") from lib.leaf as Row;
use function @id("leaf") from lib.leaf as leaf;
@id("bridge") fn fill(output:own Bytes,row:borrow Row)->Bytes { leaf(output,row) }
"#;
const LEAF: &str = r#"module lib.leaf;
@id("row") record Row { @id("row.label") label:string, @id("row.index") index:usize, }
@id("leaf") fn leaf(output:own Bytes,row:borrow Row)->Bytes {
 let mut next=output; next=bytes_set(next,row.index,65u8); next
}
"#;

fn build(sources: &[(&str, &str)]) -> WorkspaceGraphBuild {
    let mut sources = sources.to_vec();
    if sources.len() == 1 {
        sources.push((
            "support.spx",
            "module support.fixture; @id(\"support.ready\") fn ready()->i64 {0}",
        ));
    }
    let sources = sources
        .iter()
        .map(|(path, text)| WorkspaceSource {
            path: (*path).to_owned(),
            source: format::canonical(
                &crate::parse(text, Path::new(path)).expect("fixture parses"),
            ),
        })
        .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .expect("fixture is a checked retained workspace")
        .0
}
fn imported() -> WorkspaceGraphBuild {
    build(&[("app.spx", APP), ("bridge.spx", BRIDGE), ("leaf.spx", LEAF)])
}
fn selected(build: &WorkspaceGraphBuild) -> Result<Option<&'static str>, Vec<Diagnostic>> {
    let module = build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "app.main")
        .unwrap();
    schema(module, &build.hir.modules, &build.hir.declarations)
}
fn function_mut<'a>(build: &'a mut WorkspaceGraphBuild, id: &str) -> &'a mut hir::ResolvedFunction {
    build
        .hir
        .modules
        .iter_mut()
        .flat_map(|module| &mut module.functions)
        .find(|function| function.id.as_str() == id)
        .unwrap()
}
fn refused(build: &WorkspaceGraphBuild) {
    assert_eq!(selected(build).unwrap_err()[0].code, "SPX-G410");
}

#[test]
fn checked_local_helper_selects_new_schema_but_pure_parts_have_no_authority() {
    let build = build(&[("app.spx", LOCAL)]);
    assert_eq!(selected(&build).unwrap(), Some("semaprax.graph.v76"));
    let module = &build.hir.modules[0];
    assert_eq!(
        super::super::schema(module, &build.hir.modules, &build.hir.declarations).unwrap(),
        "semaprax.graph.v76"
    );
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
}

#[test]
fn direct_and_transitive_imports_replay_the_actual_callee_module_facts() {
    let transitive = imported();
    assert_eq!(selected(&transitive).unwrap(), Some("semaprax.graph.v76"));
    let direct_app = APP.replace(
        "use function @id(\"bridge\") from lib.bridge as fill;",
        "use function @id(\"leaf\") from lib.leaf as fill;",
    );
    let direct = build(&[("app.spx", &direct_app), ("leaf.spx", LEAF)]);
    assert_eq!(selected(&direct).unwrap(), Some("semaprax.graph.v76"));
    // These facts exist only because this module's checked signature/body used
    // Row. An imported caller's equivalent fact cannot authenticate this callee.
    let mut forged = imported();
    forged
        .hir
        .modules
        .iter_mut()
        .find(|module| module.module == "lib.leaf")
        .unwrap()
        .signature_types
        .clear();
    refused(&forged);
}

#[test]
fn helper_authority_rejects_missing_duplicate_and_wrong_source_owners() {
    for mutation in 0..7 {
        let mut build = imported();
        match mutation {
            0 => {
                build.hir.declarations.remove("leaf");
            }
            1 => {
                build.hir.declarations.get_mut("leaf").unwrap().origin =
                    hir::IdentityOrigin::Automatic;
            }
            2 => {
                build.hir.declarations.get_mut("leaf").unwrap().path = Some("app.spx".to_owned());
            }
            3 => {
                build.hir.declarations.get_mut("leaf").unwrap().owner = Some("main".to_owned());
            }
            4 => {
                build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|module| module.module == "lib.leaf")
                    .unwrap()
                    .functions
                    .clear();
            }
            5 => {
                let duplicate = function_mut(&mut build, "leaf").clone();
                build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|module| module.module == "app.main")
                    .unwrap()
                    .functions
                    .push(duplicate);
            }
            _ => {
                build.hir.declarations.get_mut("leaf").unwrap().module =
                    Some("lib.bridge".to_owned());
            }
        }
        refused(&build);
    }
}

#[test]
fn borrowed_nominal_authority_rejects_layout_field_type_and_provenance_drift() {
    for mutation in 0..7 {
        let mut build = imported();
        match mutation {
            0 => {
                build.hir.declarations.remove("row");
            }
            1 => {
                build.hir.declarations.get_mut("row").unwrap().origin =
                    hir::IdentityOrigin::Automatic;
            }
            2 => {
                build.hir.declarations.get_mut("row.index").unwrap().owner =
                    Some("main".to_owned());
            }
            3 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|module| module.module == "lib.leaf")
                    .unwrap();
                let hir::ResolvedTypeDeclarationKind::Record { fields } = &mut module.types[0].kind
                else {
                    panic!("record");
                };
                fields[0].ty = ResolvedType::Bool;
            }
            4 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|module| module.module == "lib.leaf")
                    .unwrap();
                module
                    .signature_types
                    .values_mut()
                    .find(|(kind, _)| *kind == hir::DeclarationKind::Record)
                    .unwrap()
                    .1
                    .layout_key
                    .push('!');
            }
            5 => {
                let record = build
                    .hir
                    .modules
                    .iter()
                    .find(|module| module.module == "lib.leaf")
                    .unwrap()
                    .types[0]
                    .clone();
                build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|module| module.module == "lib.bridge")
                    .unwrap()
                    .types
                    .push(record);
            }
            _ => {
                function_mut(&mut build, "leaf").params[1].ty = ResolvedType::Bytes;
            }
        }
        refused(&build);
    }
}

#[test]
fn hidden_allocating_literal_and_cyclic_body_fail_closed() {
    let mut literal = imported();
    function_mut(&mut literal, "leaf").body.kind =
        ResolvedExprKind::String("hidden allocation".to_owned());
    refused(&literal);
    let mut cycle = imported();
    let bridge = function_mut(&mut cycle, "bridge");
    // Keep the checked call's complete signature and arguments; only replace
    // the stable target with the active caller identity.
    let ResolvedExprKind::Block { tail, .. } = &mut bridge.body.kind else {
        panic!("block");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
        panic!("call");
    };
    *callee = bridge.id.clone();
    refused(&cycle);
}

#[test]
fn forged_schema_and_wrong_same_owner_argument_cannot_select_helper_authority() {
    let mut absent = build(&[(
        "app.spx",
        "module app.main; @id(\"main\") fn main()->i64 {7}",
    )]);
    function_mut(&mut absent, "main").cleanup_plan.schema =
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19;
    refused(&absent);
    let mut wrong = build(&[("app.spx", LOCAL)]);
    let function = function_mut(&mut wrong, "main");
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block");
    };
    let hir::ResolvedStatement::Assign { value, .. } = statements
        .iter_mut()
        .find(|statement| candidate(statement))
        .unwrap()
    else {
        panic!("assignment");
    };
    let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
        panic!("call");
    };
    let ResolvedExprKind::Place(place) = &mut args[0].kind else {
        panic!("place");
    };
    place.root = function.result_id.clone();
    refused(&wrong);
}

#[test]
fn complete_absence_census_has_zero_budget_and_zero_scratch_debit() {
    let build = build(&[(
        "app.spx",
        r#"module app.main;
@id("plain") fn plain(value:i64)->i64 { value+1 }
@id("main") fn main()->i64 { let n=plain(6); if n==7 {7}else{0} }
"#,
    )]);
    let (selected, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || selected(&build));
    assert_eq!(selected.unwrap(), None);
    assert!(!overflowed);
    assert_eq!(
        used, 0,
        "an absent helper profile materializes no proof scratch"
    );
}

#[test]
fn allocating_same_owner_call_outside_loop_retains_the_ordinary_profile() {
    let source = r#"module app.main;
@id("replace") fn replace(output:own Bytes)->Bytes {
 let length=byte_len(bytes_as_slice(output)); bytes_zeroed(length)
}
@id("main") fn main()->i64 {
 let mut output=bytes_zeroed(1usize); let next=replace(output); 7
}
"#;
    let build = build(&[("app.spx", source)]);
    assert_eq!(selected(&build).unwrap(), None);
    assert_ne!(
        super::super::schema(
            &build.hir.modules[0],
            &build.hir.modules,
            &build.hir.declarations
        )
        .unwrap(),
        "semaprax.graph.v76"
    );
}

#[test]
fn allocating_same_owner_assignment_remains_outside_explicit_mutation_profile() {
    let source = r#"module app.main;
@id("replace") fn replace(output:own Bytes)->Bytes {
 let length=byte_len(bytes_as_slice(output)); bytes_zeroed(length)
}
@id("main") fn main()->i64 {
 let mut output=bytes_zeroed(1usize); output=replace(output); 7
}
"#;
    let errors = crate::check(source, "allocating-assignment.spx").unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-U105"));
}

#[test]
fn imported_allocating_body_cannot_acquire_identity_prototype_authority() {
    let allocating = LEAF.replace(
        "next=bytes_set(next,row.index,65u8);",
        "let ignored=bytes_zeroed(1usize); next=bytes_set(next,row.index,65u8);",
    );
    let sources = [
        ("app.spx", APP),
        ("bridge.spx", BRIDGE),
        ("leaf.spx", allocating.as_str()),
    ]
    .into_iter()
    .map(|(path, source)| WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&crate::parse(source, Path::new(path)).unwrap()),
    })
    .collect();
    let errors = build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .err()
        .expect("real allocating provider must not become a nonallocating prototype");
    assert!(errors.iter().any(|error| error.code == "SPX-U105"));
}

#[test]
fn repeated_retained_assignments_share_one_positive_proof_and_exact_provider_context() {
    let repeated = APP.replace(
        "output=fill(output,row);",
        &"output=fill(output,row);".repeat(64),
    );
    let build = build(&[
        ("app.spx", &repeated),
        ("bridge.spx", BRIDGE),
        ("leaf.spx", LEAF),
    ]);
    let module = build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "app.main")
        .unwrap();
    let mut proofs = Proofs::new(&build.hir.modules, &build.hir.declarations);
    assert_eq!(
        schema_with_proofs(
            module,
            &build.hir.modules,
            &build.hir.declarations,
            &mut proofs
        )
        .unwrap(),
        Some("semaprax.graph.v76")
    );
    assert_eq!(
        proofs.proof_count, 1,
        "sixty-four sites independently check one immutable transitive callee"
    );
    let callee = target(
        &build.hir.modules,
        &build.hir.declarations,
        &hir::DeclarationId::new("bridge"),
    )
    .unwrap()
    .1;
    let count: usize = build.hir.modules.iter().map(|m| m.functions.len()).sum();
    let (warm, overflowed, used) = crate::bounded_output::with_limit_usage(0, || {
        proofs.prove(&build.hir.modules, &build.hir.declarations, callee, count)
    });
    assert!(warm.unwrap());
    assert!(!overflowed);
    assert_eq!(used, 0);
    let foreign = imported();
    let callee = target(
        &foreign.hir.modules,
        &foreign.hir.declarations,
        &hir::DeclarationId::new("bridge"),
    )
    .unwrap()
    .1;
    assert_eq!(
        proofs
            .prove(
                &foreign.hir.modules,
                &foreign.hir.declarations,
                callee,
                count
            )
            .unwrap_err()[0]
            .code,
        "SPX-G410"
    );
}

#[test]
fn retained_helper_cursor_exact_backing_and_one_short_preserve_budget_refusal() {
    let build = imported();
    let modules = &build.hir.modules;
    let facts = &build.hir.declarations;
    let callee = target(modules, facts, &hir::DeclarationId::new("bridge"))
        .unwrap()
        .1;
    let count: usize = modules.iter().map(|m| m.functions.len()).sum();
    let required = std::mem::size_of::<Proofs<'_>>()
        + count * std::mem::size_of::<&hir::ResolvedFunction>()
        + std::mem::size_of::<crate::loop_calls::byte_writer::ProofCursor<'_>>();
    for limit in [required, required - 1] {
        let (result, overflowed, used) = crate::bounded_output::with_limit_usage(limit, || {
            Proofs::new(modules, facts).prove(modules, facts, callee, count)
        });
        if limit == required {
            assert!(result.unwrap());
            assert!(!overflowed);
            assert_eq!(used, required);
        } else {
            assert!(overflowed);
            assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
            assert!(used < required);
        }
    }
}

#[test]
fn helper_positive_proof_never_authenticates_changed_provider_body_or_facts() {
    let original = imported();
    let mut proofs = Proofs::new(&original.hir.modules, &original.hir.declarations);
    let callee = target(
        &original.hir.modules,
        &original.hir.declarations,
        &hir::DeclarationId::new("bridge"),
    )
    .unwrap()
    .1;
    assert!(proofs
        .prove(&original.hir.modules, &original.hir.declarations, callee, 3)
        .unwrap());
    for mutate_body in [false, true] {
        let mut changed = imported();
        if mutate_body {
            function_mut(&mut changed, "leaf")
                .effects
                .push("fs.read".to_owned());
        } else {
            changed
                .hir
                .modules
                .iter_mut()
                .find(|m| m.module == "lib.leaf")
                .unwrap()
                .signature_types
                .clear();
        }
        refused(&changed);
    }
}

#[test]
fn borrowed_text_row_vector_replays_the_checked_declared_layout_priority() {
    let app = r#"module app.main;
use type @id("row") from lib.leaf as Row;
use function @id("bridge") from lib.bridge as fill;
@id("main") fn main()->i64 {
 let mut rows=vec_with_capacity<Row>(1usize);
 rows=vec_push<Row>(rows,Row{label:"meta",index:0usize});
 let mut output=bytes_zeroed(2usize);output=fill(output,rows);7
}"#;
    let bridge = BRIDGE.replace("row:borrow Row", "row:borrow Vec<Row>");
    let leaf = LEAF
        .replace("row:borrow Row", "row:borrow Vec<Row>")
        .replace("next,row.index,65u8", "next,vec_len<Row>(row),65u8");
    let checked = build(&[
        ("app.spx", app),
        ("bridge.spx", &bridge),
        ("leaf.spx", &leaf),
    ]);
    assert_eq!(selected(&checked).unwrap(), Some("semaprax.graph.v76"));
    let mut forged = checked;
    let module = forged
        .hir
        .modules
        .iter_mut()
        .find(|m| m.module == "lib.leaf")
        .unwrap();
    let (_, facts) = module
        .signature_types
        .values_mut()
        .find(|(_, facts)| facts.layout_key.starts_with("declared-vector-only:"))
        .unwrap();
    facts.layout_key =
        facts
            .layout_key
            .replacen("declared-vector-only:", "owned-leaf-vector-v1:", 1);
    refused(&forged);
}

#[test]
fn retained_schema_walk_reuses_only_physical_backing_and_rechecks_claims() {
    let checked = imported();
    let modules = &checked.hir.modules;
    let facts = &checked.hir.declarations;
    let module = modules.iter().find(|m| m.module == "app.main").unwrap();
    let mut proofs = Proofs::new(modules, facts);
    assert_eq!(
        schema_with_proofs(module, modules, facts, &mut proofs).unwrap(),
        Some("semaprax.graph.v76")
    );
    let (result, refused, bytes) = crate::bounded_output::with_limit_usage(0, || {
        schema_with_proofs(module, modules, facts, &mut proofs)
    });
    assert_eq!(result.unwrap(), Some("semaprax.graph.v76"));
    assert!(!refused);
    assert_eq!(bytes, 0);
    assert_eq!(proofs.proof_count, 1);
}
