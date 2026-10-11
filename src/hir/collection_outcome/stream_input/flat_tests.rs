use super::*;
const SOURCE: &str = include_str!("../../../../tests/fixtures/normalized-input-flat-outcome.spx");
fn copy_source() -> String {
    SOURCE
        .replace("label: string", "label: bool")
        .replace("label: \"ok\"", "label: true")
        .replace("label == \"ok\"", "label")
        .replace("match own first", "match first")
        .replace(
            "vec_clone_at<Row>(rows, 0usize)",
            "vec_get<Row>(rows, 0usize)",
        )
}
fn join(program: &ResolvedProgram) -> &ResolvedExpr {
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "join")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("join body")
    };
    tail
}
#[test]
fn both_flat_families_and_unrelated_names_replay_normalized_input_source_hir_cache_and_graph() {
    for source in [
        SOURCE.to_owned(),
        copy_source(),
        SOURCE
            .replace("Input", "Carrier")
            .replace("Output", "Answer")
            .replace("Ready", "Done")
            .replace("Error", "Rejected"),
    ] {
        let ast = crate::check(&source, "flat-input-join.spx").unwrap();
        let canonical = crate::format::canonical(&ast);
        let ast = crate::check(&canonical, "flat-input-join.spx").unwrap();
        assert_eq!(crate::format::canonical(&ast), canonical);
        let program = resolve(&ast).unwrap();
        assert!(super::super::runtime_admitted(
            &program.declarations,
            &join(&program).ty
        ));
        assert!(super::super::match_join(
            &program.declarations,
            join(&program)
        ));
        validate(&program).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "join")
            .unwrap();
        let ResolvedExprKind::Match { arms, .. } = &join(&program).kind else {
            unreachable!()
        };
        let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
            unreachable!()
        };
        let bytes = crate::cleanup_plan::StorageId::Value(fields[0].binding.id.clone());
        let region = function
            .cleanup_plan
            .regions
            .iter()
            .find(|region| region.slots.contains(&bytes))
            .unwrap();
        let exit = &function.cleanup_plan.exits[region.normal_scope_end.0 as usize];
        assert!(
            exit.finalize_in_order
                .iter()
                .any(|action| action.source.storage == bytes)
        );
        assert!(
            matches!(
                exit.continuation,
                crate::cleanup_plan::ExitContinuation::Continue(_)
            ),
            "input settles at its arm boundary before result publication"
        );
        assert!(
            exit.finalize_in_order
                .iter()
                .all(|action| action.source.storage
                    != crate::cleanup_plan::StorageId::Temporary(join(&program).id.clone()))
        );

        crate::codegen::emit_hir_c(&program).unwrap();
        wasmparser::Validator::new()
            .validate_all(&crate::wasm::emit_resolved_module(&program).unwrap())
            .unwrap();
        let wire = crate::cache_codec::encode(&program).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        validate(&restored).unwrap();
        assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
        let graph = crate::graph::to_json(&ast).unwrap();
        crate::graph::verify_json(&ast, &graph).unwrap();
    }
}
#[test]
fn flat_join_refuses_borrowed_publication_wrong_result_modes_and_missing_or_duplicate_transfers() {
    for source in [SOURCE.to_owned(), copy_source()] {
        let original =
            resolve(&crate::check(&source, "flat-input-hostility.spx").unwrap()).unwrap();
        assert!(super::super::match_join(
            &original.declarations,
            join(&original)
        ));
        for mutation in 0..8 {
            let mut changed = original.clone();
            let function = changed
                .functions
                .iter_mut()
                .find(|function| function.id.as_str() == "join")
                .unwrap();
            let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Match { mode, arms, .. } = &mut tail.kind else {
                unreachable!()
            };
            match mutation {
                0 => *mode = ResolvedMatchMode::Borrow,
                1 => arms[0].value.ownership = OwnershipMode::Borrow,
                2 => arms[1].value.ty = ResolvedType::String,
                5 => {
                    let edge = function
                        .cleanup_plan
                        .edges
                        .iter_mut()
                        .find(|edge| {
                            matches!(
                                edge.condition,
                                crate::cleanup_plan::EdgeCondition::VariantCase { .. }
                            )
                        })
                        .unwrap();
                    let crate::cleanup_plan::EdgeCondition::VariantCase { case, .. } =
                        &mut edge.condition
                    else {
                        unreachable!()
                    };
                    *case = DeclarationId::new("out.error");
                }
                7 => {
                    let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[1].pattern else {
                        unreachable!()
                    };
                    fields[0].field = DeclarationId::new("foreign.bytes");
                }
                6 => {
                    let exit = function
                        .cleanup_plan
                        .exits
                        .iter_mut()
                        .find(|exit| !exit.finalize_in_order.is_empty())
                        .unwrap();
                    exit.finalize_in_order.remove(0);
                }
                _ => {
                    let destination = crate::cleanup_plan::StorageId::Temporary(tail.id.clone());
                    let transitions = function.cleanup_plan.blocks.iter_mut().find_map(|block| {
                        block.transitions.iter().position(|transition| matches!(transition,
                            crate::cleanup_plan::CleanupTransition::Transfer { destination: place, .. }
                            | crate::cleanup_plan::CleanupTransition::TransferVariant { destination: place, .. }
                            if place.storage == destination)).map(|index| (&mut block.transitions, index))
                    }).expect("real arm-result transfer");
                    if mutation == 3 {
                        transitions.0.remove(transitions.1);
                    } else {
                        let duplicate = transitions.0[transitions.1].clone();
                        transitions.0.insert(transitions.1, duplicate);
                    }
                }
            }
            assert_eq!(
                validate(&changed).unwrap_err().code,
                "SPX-H006",
                "mutation {mutation}"
            );
            assert!(crate::codegen::emit_hir_c(&changed).is_err());
            assert!(crate::wasm::emit_resolved_module(&changed).is_err());
        }
    }
}

#[test]
fn imported_flat_join_reconstructs_current_provider_layout_and_rejects_missing_or_foreign_roots() {
    for source in [SOURCE.to_owned(), copy_source()] {
        let (provider, body) = source.split_once("@id(\"join\")").unwrap();
        let caller = format!(
            r#"module normalized.consumer;
use type @id("input") from normalized.flat as Input;
use type @id("row") from normalized.flat as Row;
use type @id("out") from normalized.flat as Output;
use function @id("decode") from normalized.flat as decode;
@id("join"){body}"#
        );
        let build = |provider: &str, caller: &str| {
            let sources = [("provider.spx", provider), ("app.spx", caller)]
                .into_iter()
                .map(|(path, source)| {
                    let ast = crate::parse(source, path).unwrap();
                    crate::workspace_graph::WorkspaceSource {
                        path: path.into(),
                        source: crate::format::canonical(&ast),
                    }
                })
                .collect();
            crate::workspace_graph::build_owned(sources)
        };
        let built = build(provider, &caller).unwrap();
        let linked = built
            .linked_owned_data_api_program_with_roots("normalized.consumer", &[])
            .unwrap();
        validate(&linked).unwrap();
        assert!(super::super::match_join(
            &linked.declarations,
            join(&linked)
        ));
        let observed =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                .unwrap();
        assert!(matches!(
            observed.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        ));
        crate::codegen::emit_hir_c(&linked).unwrap();
        wasmparser::Validator::new()
            .validate_all(&crate::wasm::emit_resolved_module(&linked).unwrap())
            .unwrap();
        // A fresh explicit child identity is valid only after current provider
        // reconstruction; a stale HIR field identity is refused above.
        assert!(
            build(
                &provider.replace("@id(\"input.bytes\")", "@id(\"fresh.bytes\")"),
                &caller
            )
            .is_ok()
        );
        for changed in [
            provider.replace("bytes: Bytes", "bytes: string"),
            provider.replace("length: usize", "length: i64"),
            provider.replace("@id(\"row\")", "@id(\"foreign.row\")"),
        ] {
            assert!(
                build(&changed, &caller).is_err(),
                "stale/foreign provider {changed}"
            );
        }
        for import in [
            "use type @id(\"input\") from normalized.flat as Input;",
            "use type @id(\"out\") from normalized.flat as Output;",
        ] {
            assert!(
                build(provider, &caller.replace(import, "")).is_err(),
                "missing root {import}"
            );
        }
    }
}
