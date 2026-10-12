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
    let changed_view = SyntheticProgramView::new(&changed, Vec::new()).unwrap();
    assert!(checked_source_matches(&view, &program));
    assert!(!checked_source_matches(&changed_view, &program));
    assert!(pass.checked_module_source(&program.path, &view).is_some());
    assert!(pass
        .checked_module_source(&program.path, &changed_view)
        .is_none());
    let (borrowed, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut owner =
            crate::bounded_output::identity_reservation::IdentityReservation::acquire().unwrap();
        pass.resolve_borrowed_cold_source_paid(&program.path, &changed_view, None, &mut owner)
            .unwrap()
            .unwrap()
    });
    assert!(!refused);
    assert_eq!(borrowed.resolved, reused.resolved);
    assert_eq!(borrowed.reused_functions, reused.reused_functions);
    // `function_costs` measure the active ledger around each resolved body, so
    // the paid borrowed route is compared with the legacy route under its own
    // separate, equivalent active ledger (the unpaid run above has none).
    let (legacy_active, legacy_refused, _) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            pass.resolve_functions(&program.path, &changed, None)
                .unwrap()
                .unwrap()
        });
    assert!(!legacy_refused);
    assert_eq!(legacy_active.resolved, reused.resolved);
    assert_eq!(legacy_active.reused_functions, reused.reused_functions);
    assert_eq!(borrowed.function_costs, legacy_active.function_costs);
    // The edited body paid real work; the reused unchanged body keeps its
    // stored cost.
    assert_eq!(borrowed.function_costs["helper"], 0);
    assert!(borrowed.function_costs["main"] > 0);
    // No-active compatibility: the same inventory with zero measured cost.
    assert_eq!(
        reused.function_costs.keys().collect::<Vec<_>>(),
        borrowed.function_costs.keys().collect::<Vec<_>>()
    );
    assert!(reused.function_costs.values().all(|cost| *cost == 0));
    crate::hir::validate(&borrowed.resolved).unwrap();
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
fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
    use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
    use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
    let source = "module cold; @id(\"helper\") fn helper(value:i64)->i64 {value+1} @id(\"main\") fn main()->i64 {helper(1)}";
    let program = crate::check(source, "cold.spx").unwrap();
    let mut pass = pass(source, &program);
    let view = SyntheticProgramView::new(&program, Vec::new()).unwrap();
    let old = pass
        .resolve_functions(&program.path, &program, None)
        .unwrap()
        .unwrap();
    // Each full resolution is one module route: it acquires its own owner on
    // its own equivalent real workspace builder ledger. Sharing one arbitrary
    // 1 MiB ledger across both exhausted the independent-replay cursor.
    let (owned, failed, owned_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(owned_used > 0);
    let (borrowed, failed, borrowed_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(borrowed_used > 0);
    // Paid costs are measured on an active ledger, so the legacy route is
    // compared under its own equivalent active ledger. The no-active run keeps
    // its zero-cost compatibility inventory, which is not normalized.
    let (old_active, old_failed, _) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            pass.resolve_functions(&program.path, &program, None)
                .unwrap()
                .unwrap()
        });
    assert!(!old_failed);
    assert_eq!(old_active.resolved, old.resolved);
    assert_eq!(
        old.function_costs.keys().collect::<Vec<_>>(),
        old_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert!(old.function_costs.values().all(|cost| *cost == 0));
    for new in [&owned, &borrowed] {
        assert_eq!(new.resolved, old.resolved);
        assert_eq!(new.function_costs, old_active.function_costs);
        assert_eq!(
            (new.reused_functions, new.static_text),
            (old.reused_functions, old.static_text)
        );
    }
    // Initial identity refusal keeps the exact budget error and moves no HIR.
    let word = std::mem::size_of::<usize>();
    let report =
        std::mem::size_of::<IdentityReservation>() + std::mem::size_of::<Diagnostic>() + 48;
    let (refused, failed, _, receipt) = with_limit_usage_refusal(report, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
            .unwrap()
            .err()
            .unwrap()
    });
    assert!(failed);
    assert!(matches!(
        refused,
        paid_resolution::SelectiveFailure::Resolve(
            crate::hir::identity_resolution::Failure::Identity(Refused::Budget(_))
        )
    ));
    assert_eq!(
        receipt.unwrap().requested,
        (2 * word + 11).div_ceil(word) * word
    );
    // Checked body reuse and the first-checked-only borrowed gate are unchanged.
    let saved_helper_cost = owned.function_costs["helper"];
    let _owner = pass.resolved_module(
        &program.path,
        program.clone(),
        owned.resolved,
        0,
        0,
        owned.function_costs,
        owned.reused_functions,
    );
    pass.checked = std::mem::take(&mut pass.next_checked);
    let changed = crate::check(&source.replace("helper(1)", "helper(2)"), "cold.spx").unwrap();
    let legacy = pass
        .resolve_functions(&program.path, &changed, None)
        .unwrap()
        .unwrap();
    let (paid, failed, _) = with_limit_usage(1 << 20, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let paid = pass
            .resolve_functions_paid(&program.path, &changed, None, &mut owner)
            .unwrap()
            .unwrap();
        assert!(pass
            .resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
            .is_none());
        paid
    });
    assert!(!failed);
    assert_eq!(paid.reused_functions, 1);
    assert_eq!(paid.reused_functions, legacy.reused_functions);
    assert_eq!(paid.resolved, legacy.resolved);
    // Same separate, equivalent 1 MiB active ledger for the legacy route; the
    // no-active `legacy` run above keeps its zero-cost compatibility inventory.
    let (legacy_active, legacy_failed, _) = with_limit_usage(1 << 20, || {
        pass.resolve_functions(&program.path, &changed, None)
            .unwrap()
            .unwrap()
    });
    assert!(!legacy_failed);
    assert_eq!(legacy_active.resolved, legacy.resolved);
    assert_eq!(legacy_active.reused_functions, legacy.reused_functions);
    assert_eq!(paid.function_costs, legacy_active.function_costs);
    assert_eq!(
        legacy.function_costs.keys().collect::<Vec<_>>(),
        legacy_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert_eq!(legacy.function_costs["main"], 0);
    assert_eq!(legacy.function_costs["helper"], saved_helper_cost);
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
            .to_owned()
        };
        assert_eq!(image(actual), image(expected));
    };
    let trace = |label: &str| {
        let trace = crate::bounded_output::stage_debits::take_completed().unwrap();
        assert_eq!(trace.sum(), trace.consumed);
        // Only fixed Copy counters were captured on the real ledger. Formatting
        // happens after construction, outside that ledger and any allocator scope.
        println!("checked-cold source route={label} prebound={} layouts={} mode={} consumed={} stages={:?}",
            trace.prebound, trace.layouts, trace.mode, trace.consumed, trace.debits);
        trace
    };
    let original = sources(2);
    let mut ordinary = ProjectFrontendCache::new();
    let _ = crate::bounded_output::stage_debits::take_completed();
    let old = ordinary.build(&manifest, &original).unwrap();
    let old_trace = trace("ordinary-cold");
    let mut checked = ProjectFrontendCache::new_with_semantic_cache();
    let cold = checked.build(&manifest, &original).unwrap();
    let cold_trace = trace("checked-cold");
    let warm = checked.build(&manifest, &original).unwrap();
    let warm_trace = trace("checked-warm");
    println!("checked-cold complete attempts ordinary={old_trace:?} cold={cold_trace:?} warm={warm_trace:?}");
    same(&cold, &old);
    same(&warm, &old);
    let report: serde_json::Value = serde_json::from_str(warm.to_json()).unwrap();
    assert_eq!(report["work"]["modules_resolved"], 0);
    let changed = sources(3);
    let old_changed = ProjectFrontendCache::new()
        .build(&manifest, &changed)
        .unwrap();
    let changed_old_trace = trace("changed-ordinary-cold");
    let changed_checked = checked.build(&manifest, &changed).unwrap();
    let _ = trace("changed-checked-reuse");
    same(&changed_checked, &old_changed);
    let report: serde_json::Value = serde_json::from_str(changed_checked.to_json()).unwrap();
    assert!(
        report["work"]["monomorphic_function_HIR_reused"]
            .as_u64()
            .unwrap()
            > 0
    );
    let changed_warm = checked.build(&manifest, &changed).unwrap();
    let changed_warm_trace = trace("changed-checked-warm");
    println!(
        "checked-cold changed attempts ordinary={changed_old_trace:?} warm={changed_warm_trace:?}"
    );
    same(&changed_warm, &old_changed);
}
