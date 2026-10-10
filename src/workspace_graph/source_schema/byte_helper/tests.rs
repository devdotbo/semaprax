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
fn allocating_same_owner_helper_outside_loop_retains_the_ordinary_profile() {
    let source = r#"module app.main;
@id("replace") fn replace(output:own Bytes)->Bytes {
 let length=byte_len(bytes_as_slice(output)); bytes_zeroed(length)
}
@id("main") fn main()->i64 {
 let mut output=bytes_zeroed(1usize); output=replace(output); 7
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
