//! Source-owned transitive scalar providers and independent local controls.
use super::*;
const CALLER: &str = r#"module scalar.app;
use function @id("helper.step") from scalar.helper as step;
@id("app.main") fn main()->i64 {let label=str"é\u{0}";let result=step(41);return result;}
"#;
const PROVIDER: &str = r#"module scalar.helper;
use function @id("leaf.step") from scalar.leaf as leaf_step;
@id("helper.step") fn step(n:i64)->i64 {leaf_step(n)}
@id("unrelated") fn unrelated()->Bytes {bytes_with_capacity(1usize)}
"#;
const LEAF: &str = r#"module scalar.leaf; @id("leaf.step") fn step(n:i64)->i64 {n+1}"#;
fn build(caller: &str, provider: &str, leaf: &str) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
    let sources = [
        ("caller.spx", caller),
        ("provider.spx", provider),
        ("leaf.spx", leaf),
    ]
    .into_iter()
    .map(|(path, source)| WorkspaceSource {
        path: path.into(),
        source: crate::format::canonical(&crate::parse(source, path).unwrap()),
    })
    .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .map(|(build, _)| build)
}
fn caller(build: &WorkspaceGraphBuild) -> &WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "scalar.app")
        .unwrap()
}
fn schema(build: &WorkspaceGraphBuild) -> Result<&'static str, Vec<Diagnostic>> {
    crate::workspace_graph::source_schema::schema(
        caller(build),
        &build.hir.modules,
        &build.hir.declarations,
    )
}
fn local(leaf: &str) -> hir::ResolvedProgram {
    let start = leaf.find("@id(").unwrap();
    let source = format!(
        "module scalar.local;\n{}\n@id(\"helper.step\") fn helper(n:i64)->i64 {{step(n)}}\n@id(\"app.main\") fn main()->i64 {{let label=str\"é\\u{{0}}\";let result=helper(41);return result;}}",
        &leaf[start..]
    );
    crate::hir::resolve(&crate::check(&source, "local.spx").unwrap()).unwrap()
}
#[test]
fn three_modules_use_actual_scalar_providers_without_literal_inventories_or_name_collisions() {
    let build = build(CALLER, PROVIDER, LEAF).unwrap();
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
    for name in ["scalar.helper", "scalar.leaf"] {
        let provider = build.hir.modules.iter().find(|m| m.module == name).unwrap();
        assert!(provider.static_text.is_none());
        assert!(provider.scalar_helpers.is_some());
    }
    let provider = build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "scalar.helper")
        .unwrap();
    assert_eq!(
        provider
            .scalar_helpers
            .as_ref()
            .unwrap()
            .functions
            .iter()
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        ["helper.step"]
    );
    let linked = build
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    let control = local(LEAF);
    for _ in 0..3 {
        let actual =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                .unwrap();
        let expected =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&control, "app.main", 100_000)
                .unwrap();
        assert_eq!(actual.outcome, expected.outcome);
        assert_eq!(
            actual.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        );
    }
}
#[test]
fn checked_arithmetic_and_contract_failure_match_saved_all_local_controls() {
    for leaf in [
        LEAF.to_owned(),
        LEAF.replace("n+1", "n+9223372036854775807"),
        LEAF.replace("n+1", "n/0"),
        LEAF.replace("->i64 {", "->i64 requires false {"),
        LEAF.replace("->i64 {", "->i64 ensures false {"),
    ] {
        let build = build(CALLER, PROVIDER, &leaf).unwrap();
        let linked = build
            .linked_owned_data_api_program_with_roots("scalar.app", &[])
            .unwrap();
        let control = local(&leaf);
        for _ in 0..3 {
            assert_eq!(
                crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                    .unwrap()
                    .outcome,
                crate::interpreter::evaluate_resolved_zero_arg_i64(&control, "app.main", 100_000)
                    .unwrap()
                    .outcome
            );
        }
        runtime::run(&linked, &control);
    }
}
#[test]
fn current_source_body_contract_target_and_namespace_guards_refuse_stale_providers() {
    for attack in 0..9 {
        let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
        match attack {
            0 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                module.functions[0].name = "forged".into();
            }
            1 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                module.functions[0].body.span.end += 1;
            }
            2 => {
                let module = build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap();
                let body = module.functions[0].body.clone();
                module.functions[0].ensures.push(body);
            }
            3 => {
                build
                    .hir
                    .modules
                    .iter_mut()
                    .find(|m| m.module == "scalar.leaf")
                    .unwrap()
                    .scalar_helpers = None
            }
            4 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().kind =
                    hir::DeclarationKind::Record
            }
            5 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().origin =
                    hir::IdentityOrigin::Automatic
            }
            6 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().owner =
                    Some("helper.step".into())
            }
            7 => {
                build.hir.declarations.get_mut("leaf.step").unwrap().path =
                    Some("forged.spx".into())
            }
            _ => {
                build.hir.declarations.get_mut("leaf.step").unwrap().module =
                    Some("scalar.helper".into())
            }
        }
        assert!(
            schema(&build)
                .unwrap_err()
                .iter()
                .any(|e| e.code == "SPX-G410"),
            "attack {attack}"
        );
    }
}
#[test]
fn actual_provider_plan_mutations_cannot_borrow_source_digest_authority() {
    for attack in 0..6 {
        let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
        let certificate = caller(&build).functions[0]
            .cleanup_plan
            .lexical_control
            .as_ref()
            .unwrap()
            .clone();
        let module = build
            .hir
            .modules
            .iter_mut()
            .find(|m| m.module == "scalar.leaf")
            .unwrap();
        let function = &mut module.functions[0];
        match attack {
            0 => {
                function.cleanup_plan.status_sources.pop().unwrap();
            }
            1 => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| {
                        matches!(
                            exit.continuation,
                            crate::cleanup_plan::ExitContinuation::CommitResult { .. }
                        )
                    })
                    .unwrap();
                let crate::cleanup_plan::ExitContinuation::CommitResult {
                    source: crate::cleanup_plan::CleanupResultSource::Scalar { expression },
                } = &mut exit.continuation
                else {
                    panic!("scalar result")
                };
                *expression = hir::ExpressionId::new(
                    &hir::FunctionExecutionId::Monomorphic(function.id.clone()),
                    "forged.result",
                );
            }
            2 => function.cleanup_plan.lexical_control = Some(certificate),
            3 => {
                let edge = &mut function.cleanup_plan.edges[0];
                edge.to = crate::cleanup_plan::BlockId(u32::MAX);
            }
            4 => function.loan_plan.schema = "forged.loan.schema",
            _ => {
                let block = function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .find(|b| {
                        b.transitions.iter().any(|t| {
                            matches!(
                                t,
                                crate::cleanup_plan::CleanupTransition::SelectFailure { .. }
                            )
                        })
                    })
                    .unwrap();
                let transition = block
                    .transitions
                    .iter()
                    .find(|t| {
                        matches!(
                            t,
                            crate::cleanup_plan::CleanupTransition::SelectFailure { .. }
                        )
                    })
                    .unwrap()
                    .clone();
                block.transitions.push(transition);
            }
        }
        assert!(
            schema(&build)
                .unwrap_err()
                .iter()
                .any(|e| e.code == "SPX-G410"),
            "plan attack {attack}"
        );
    }
}

fn measured(
    build: &WorkspaceGraphBuild,
    work: usize,
    bytes: usize,
) -> (
    Result<(usize, usize), Vec<Diagnostic>>,
    crate::test_allocation_observer::Metrics,
) {
    crate::test_allocation_observer::measure(|| {
        let module = caller(build);
        let anchor = &module.functions[0];
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, work, bytes).map_err(|_| fail())?;
        let context = authenticate(
            module,
            &build.hir.modules,
            &build.hir.declarations,
            &mut resources,
        )?;
        let mut session = SelectedReplaySession::from_preparation(&context, anchor, resources)
            .map_err(|_| fail())?;
        for helper in &context.helpers {
            session
                .validate_scalar(helper.function)
                .map_err(|_| fail())?;
        }
        session.validate(anchor).map_err(|_| fail())?;
        let (remaining, charged) = session.metrics();
        drop(session);
        drop(context);
        Ok((work.checked_sub(remaining).unwrap(), charged))
    })
}
#[test]
fn aggregate_authentication_to_final_drop_work_and_backing_are_exact_and_one_short() {
    let build = build(CALLER, PROVIDER, LEAF).unwrap();
    let (wide, allocation) = measured(&build, 32_000_000, 1 << 26);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    let (exact, allocation) = measured(&build, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    assert!(measured(&build, work - 1, bytes).0.is_err());
    assert!(measured(&build, work, bytes - 1).0.is_err());
}
#[test]
fn cycles_and_unsupported_scalar_provider_extensions_remain_closed() {
    for provider in [
        PROVIDER.replace("{leaf_step(n)}", "{step(n)}"),
        PROVIDER.replace(
            "fn step(n:i64)->i64 {leaf_step(n)}",
            "fn step(n:i64)->i64 {let label=str\"x\";if str_len_bytes(label)==1usize {n}else{0}}",
        ),
        PROVIDER.replace(
            "fn step(n:i64)->i64 {leaf_step(n)}",
            "fn step(n:i64)->i64 {let values=vec_with_capacity<i64>(1usize);n}",
        ),
    ] {
        assert!(build(CALLER, &provider, LEAF).is_err());
    }
}
#[test]
fn cloned_function_and_failed_later_provider_cannot_restart_a_lease_or_allowance() {
    let build = build(CALLER, PROVIDER, LEAF).unwrap();
    let module = caller(&build);
    let anchor = &module.functions[0];
    let mut resources = SelectedReplayPreparation::new(
        anchor,
        crate::cleanup_plan::ReplayPhysicalReservation::new(1 << 26),
    )
    .unwrap();
    let context = authenticate(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut resources,
    )
    .unwrap();
    let cloned = context.helpers[0].function.clone();
    let mut session = SelectedReplaySession::from_preparation(&context, anchor, resources).unwrap();
    assert_eq!(
        session.validate_scalar(&cloned).unwrap_err().code,
        "SPX-H006"
    );
    let metrics = session.metrics();
    assert_eq!(session.validate(anchor).unwrap_err().code, "SPX-H006");
    assert_eq!(session.metrics(), metrics);
    drop(session);
    drop(context);
    let mut malformed = self::build(CALLER, PROVIDER, LEAF).unwrap();
    malformed
        .hir
        .declarations
        .get_mut("leaf.step")
        .unwrap()
        .owner = Some("wrong.owner".into());
    let module = caller(&malformed);
    let anchor = &module.functions[0];
    let mut resources = SelectedReplayPreparation::new(
        anchor,
        crate::cleanup_plan::ReplayPhysicalReservation::new(1 << 26),
    )
    .unwrap();
    assert!(
        authenticate(
            module,
            &malformed.hir.modules,
            &malformed.hir.declarations,
            &mut resources
        )
        .is_err()
    );
    assert_eq!(
        resources
            .charge(anchor, 0, "forbidden retry")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}

mod runtime;
