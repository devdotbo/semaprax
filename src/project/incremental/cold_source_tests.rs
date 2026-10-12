use super::*;
use crate::ast::program_source::SyntheticProgramView;

fn pass(source: &str, program: &Program) -> FrontendPass {
    FrontendPass {
        indexed_imports: Vec::new(),
        entries: BTreeMap::from([(
            program.path.clone(),
            Arc::new(CachedModule {
                source: source.to_owned(),
                program: Arc::new(program.clone()),
            }),
        )]),
        parsed: 0,
        reused: 0,
        canonicalizations: 0,
        parsed_bytes: 0,
        reused_bytes: 0,
        resolved: 0,
        retained_source_bytes: 0,
        ast_budget: 0,
        semantic: true,
        checked: BTreeMap::new(),
        next_checked: BTreeMap::new(),
        checked_reused: 0,
        functions_resolved: 0,
        functions_reused: 0,
    }
}
#[test]
fn checked_cold_source_preserves_limits_costs_current_key_and_body_reuse_fallback() {
    let source = "module cold; @id(\"helper\") fn helper(value:i64)->i64 {value+1} @id(\"main\") fn main()->i64 {helper(1)}";
    let program = crate::check(source, "cold.spx").unwrap();
    let mut pass = pass(source, &program);
    assert!(pass.permits_borrowed_cold_source(&program.path));
    let view = SyntheticProgramView::new(&program, Vec::new()).unwrap();
    let old = pass
        .resolve_functions(&program.path, &program, None)
        .unwrap()
        .unwrap();
    let new = pass
        .resolve_borrowed_cold_source(&program.path, &view, None)
        .unwrap()
        .unwrap();
    assert_eq!(old.resolved, new.resolved);
    assert_eq!(old.function_costs, new.function_costs);
    assert!(!new.function_costs.is_empty());
    assert_eq!(
        (old.reused_functions, old.static_text),
        (new.reused_functions, new.static_text)
    );
    let _owner = pass.resolved_module(
        &program.path,
        program.clone(),
        new.resolved,
        0,
        0,
        new.function_costs,
        new.reused_functions,
    );
    pass.checked = std::mem::take(&mut pass.next_checked);
    assert!(!pass.permits_borrowed_cold_source(&program.path));
    assert!(pass
        .resolve_borrowed_cold_source(&program.path, &view, None)
        .is_none());
    assert!(pass.checked_module(&program.path, &program).is_some());
    let changed = crate::check(&source.replace("helper(1)", "helper(2)"), "cold.spx").unwrap();
    assert!(pass.checked_module(&program.path, &changed).is_none());
    let reused = pass
        .resolve_functions(&program.path, &changed, None)
        .unwrap()
        .unwrap();
    assert_eq!(reused.reused_functions, 1);
    assert_eq!(reused.resolved, crate::hir::resolve(&changed).unwrap());
    // Both receivers refuse count before duplicate IDs/source semantics.
    let mut excessive = program.clone();
    excessive.functions.resize(
        MAX_PROJECT_CHECKED_FUNCTIONS + 1,
        program.functions[0].clone(),
    );
    let fresh = self::pass(source, &program);
    let view = SyntheticProgramView::new(&excessive, Vec::new()).unwrap();
    let old = fresh
        .resolve_functions(&program.path, &excessive, None)
        .unwrap()
        .err()
        .unwrap();
    let new = fresh
        .resolve_borrowed_cold_source(&program.path, &view, None)
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(old, new);
    assert!(old[0]
        .message
        .contains("checked function inventory exceeds its bound"));
    // Checked cold runs before linked-source Vec/Box scopes, exactly like
    // the original selective receiver. Foreign identities remain refused.
    for (module, function, code) in [
        ("std.collections", "@id(\"std.collections.vec.with-capacity\") fn with_capacity<T>(capacity:usize)->Vec<T> {vec_with_capacity<T>(capacity)}", "SPX-T283"),
        ("std.mem", "@id(\"std.mem.box.new\") fn new<T>(value:T)->Box<T> {box_new<T>(value)}", "SPX-T286"),
    ] {
        let text = format!("module {module}; {function} @id(\"main\") fn main()->i64 {{0}}");
        let current = crate::parse(&text, "cold.spx").unwrap();
        let fresh = self::pass(&text, &current);
        let view = SyntheticProgramView::new(&current, Vec::new()).unwrap();
        let old = fresh.resolve_functions(&current.path, &current, None).unwrap().unwrap();
        let new = fresh.resolve_borrowed_cold_source(&current.path, &view, None).unwrap().unwrap();
        assert_eq!(old.resolved, new.resolved);
        let mut foreign = current.clone();
        foreign.module = "foreign".to_owned();
        let view = SyntheticProgramView::new(&foreign, Vec::new()).unwrap();
        let old = fresh.resolve_functions(&foreign.path, &foreign, None).unwrap().err().unwrap();
        let new = fresh.resolve_borrowed_cold_source(&foreign.path, &view, None).unwrap().err().unwrap();
        assert_eq!(old, new);
        assert!(new.iter().any(|error| error.code == code));
    }
}

#[test]
fn borrowed_checked_cold_preserves_complete_project_graph_image_warm_and_body_edit() {
    let manifest = ProjectManifest::parse(
        r#"schema = "semaprax.project.v1"
name = "cold"
entry = "cold.app"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = ["answer"]
tests = ["cold.tests"]
"#,
    )
    .unwrap();
    let sources = |delta| {
        [
            ("src/app.spx", format!("module cold.app; @id(\"helper\") fn helper(value:i64)->i64 {{value+{delta}}} @id(\"main\") fn main()->i64 {{helper(40)}} @id(\"answer\") fn answer(value:i64)->i64 {{helper(value)}}")),
            ("src/tests.spx", "module cold.tests; @id(\"tests.main\") fn main()->i64 {0}".to_owned()),
        ].into_iter().map(|(path, text)| {
            let program = crate::parse(&text, path).unwrap();
            let canonical = crate::format::canonical(&program);
            ProjectFrontendSource::new(path, &canonical).unwrap()
        }).collect::<Vec<_>>()
    };
    let same = |actual: &crate::project::ProjectFrontendBuild,
                expected: &crate::project::ProjectFrontendBuild| {
        assert_eq!(
            actual.revision().semantic_graph(),
            expected.revision().semantic_graph()
        );
        assert_eq!(
            actual.revision().project_revision(),
            expected.revision().project_revision()
        );
        for (left, right) in [
            (
                actual.revision().entry_program(),
                expected.revision().entry_program(),
            ),
            (
                actual.revision().public_api_program(),
                expected.revision().public_api_program(),
            ),
            (
                actual.revision().test_program(),
                expected.revision().test_program(),
            ),
        ] {
            assert_eq!(left, right);
            crate::hir::validate(left).unwrap();
        }
        let image = |build: &crate::project::ProjectFrontendBuild| {
            crate::project::ProjectSemanticImage::derive(
                Arc::clone(build.revision()),
                build.revision().project_revision(),
            )
            .unwrap()
            .to_json()
        };
        assert_eq!(image(actual), image(expected));
    };
    let original = sources(2);
    let mut ordinary = ProjectFrontendCache::new();
    let old = ordinary.build(&manifest, &original).unwrap();
    let mut checked = ProjectFrontendCache::new_with_semantic_cache();
    let cold = checked.build(&manifest, &original).unwrap();
    same(&cold, &old);
    let warm = checked.build(&manifest, &original).unwrap();
    same(&warm, &old);
    let report: serde_json::Value = serde_json::from_str(warm.to_json()).unwrap();
    assert_eq!(report["work"]["modules_resolved"], 0);
    let changed = sources(3);
    let old_changed = ProjectFrontendCache::new()
        .build(&manifest, &changed)
        .unwrap();
    let changed_checked = checked.build(&manifest, &changed).unwrap();
    same(&changed_checked, &old_changed);
    let report: serde_json::Value = serde_json::from_str(changed_checked.to_json()).unwrap();
    assert!(
        report["work"]["monomorphic_function_HIR_reused"]
            .as_u64()
            .unwrap()
            > 0
    );
    let changed_warm = checked.build(&manifest, &changed).unwrap();
    same(&changed_warm, &old_changed);
}
