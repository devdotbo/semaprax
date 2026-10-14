//! Complete old receiver comparisons for borrowed membership and traversal.
use super::{
    original, original_receiver_calls, rewrite_declaration, rewrite_declaration_runtime_cost,
    rewrite_declaration_runtime_cost_with_layout, rewrite_declaration_with_layout,
    template_is_admitted, visit_types, visit_types_mut, with_original_receiver,
};
use crate::ast::{
    Program, Type, TypeDeclaration, TypeDeclarationKind, TypeParameterDeclaration,
    VariantCaseDeclaration,
};
use crate::bounded_output::{active_refused, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::expected_projection::cost::generic_rewrite_test_bridge as cost_bridge;

fn program(source: &str) -> Program {
    crate::parse(source, "fixture.spx").unwrap()
}
pub(super) fn record() -> TypeDeclaration {
    program(r#"module fixture.provider; @id("row") record Row<T> { @id("row.x") x:T, }"#)
        .types
        .remove(0)
}
fn fixture(kind: usize, count: usize, ty: Type) -> TypeDeclaration {
    let mut declaration = record();
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        unreachable!()
    };
    let mut field = fields[0].clone();
    field.ty = ty;
    let fields = vec![field; count];
    declaration.kind = match kind {
        0 => TypeDeclarationKind::Record { fields },
        1 => {
            let mut method =
                program("module fixture.method; @id(\"method\") fn method(value:i64)->i64 {value}")
                    .functions
                    .remove(0);
            method.params[0].ty = Type::Bool;
            method.return_type = Type::Usize;
            TypeDeclarationKind::Class {
                fields,
                methods: vec![method; count],
            }
        }
        2 => {
            let case = VariantCaseDeclaration {
                stable_id: "case".into(),
                explicit_id: true,
                name: "Case".into(),
                name_span: declaration.name_span,
                fields,
                span: declaration.span,
            };
            TypeDeclarationKind::Variant {
                cases: vec![case; 2],
            }
        }
        _ => unreachable!(),
    };
    declaration
}
pub(super) fn contexts() -> (Program, Vec<Program>) {
    let provider = program(
        r#"module fixture.provider;
@id("target") record Target { @id("target.value") value:i64, }
@id("other") record Other { @id("other.value") value:bool, }"#,
    );
    let caller = program(
        r#"module fixture.caller;
use type @id("target") from fixture.provider as Imported;
use type @id("other") from fixture.provider as Another;
@id("main") fn main()->i64 {0}"#,
    );
    (caller, vec![provider])
}
fn named(name: &str) -> Type {
    Type::Named {
        name: name.into(),
        arguments: vec![],
    }
}

#[test]
fn complete_old_template_membership_preserves_duplicates_empty_unicode_and_arguments() {
    let names = [vec![], vec!["T"], vec!["T", "T"], vec!["", "雪", "T", "雪"]];
    let types = [
        Type::I64,
        Type::Bytes,
        Type::String,
        Type::Never,
        named("T"),
        named(""),
        named("雪"),
        named("Missing"),
        Type::Named {
            name: "T".into(),
            arguments: vec![Type::I64],
        },
    ];
    for parameters in names {
        for ty in &types {
            for count in [0, 1, 17, 129] {
                let mut declaration = fixture(0, count, ty.clone());
                declaration.type_parameters = parameters
                    .iter()
                    .map(|name| TypeParameterDeclaration {
                        name: (*name).into(),
                        span: declaration.span,
                    })
                    .collect();
                let (before, old) = measure(|| original::template_is_admitted(&declaration));
                let (after, current) = measure(|| template_is_admitted(&declaration).unwrap());
                assert_eq!(after, before);
                assert!(!old.invalid && !current.invalid);
                assert_eq!(old.live, 0);
                assert_eq!(current.live, 0);
                assert_eq!(current.allocated, 0);
                if !parameters.is_empty() {
                    assert!(old.allocated > 0);
                }
            }
        }
    }
    for kind in [1, 2] {
        let declaration = fixture(kind, 1, named("T"));
        assert_eq!(
            template_is_admitted(&declaration).unwrap(),
            original::template_is_admitted(&declaration)
        );
        assert!(!template_is_admitted(&declaration).unwrap());
    }
}

#[test]
fn ordered_record_class_method_and_variant_visits_match_complete_old_rewrites() {
    let (caller, programs) = contexts();
    for kind in 0..3 {
        for count in [0, 1, 16, 17, 129] {
            let declaration = fixture(kind, count, named("Target"));
            let mut current = declaration.clone();
            let mut before = declaration.clone();
            assert_eq!(
                rewrite_declaration(&mut current, "fixture.provider", &caller, &programs),
                original::rewrite_declaration(&mut before, "fixture.provider", &caller, &programs)
            );
            assert_eq!(current, before);
            let mut first = vec![];
            visit_types(&declaration.kind, &mut |ty| {
                first.push(ty.clone());
                Ok(())
            })
            .unwrap();
            let mut second = vec![];
            let mut walked = declaration.clone();
            visit_types_mut(&mut walked.kind, &mut |ty| {
                second.push(ty.clone());
                Ok(())
            })
            .unwrap();
            assert_eq!(first, second);
            assert_eq!(walked, declaration);
            let expected_count = match kind {
                0 => count,
                1 => 3 * count,
                _ => 2 * count,
            };
            assert_eq!(first.len(), expected_count);
            if kind == 1 {
                assert!(first[..count].iter().all(|ty| *ty == named("Target")));
                for pair in first[count..].chunks_exact(2) {
                    assert_eq!(pair, [Type::Bool, Type::Usize]);
                }
            }
            for (index, _) in first.iter().enumerate() {
                let mut visited = 0;
                let result = visit_types(&declaration.kind, &mut |_| {
                    if visited == index {
                        return Err(vec![]);
                    }
                    visited += 1;
                    Ok(())
                });
                assert!(result.is_err());
                assert_eq!(visited, index);
            }
        }
    }
}

#[test]
fn full_cost_fields_alias_order_and_first_errors_match_old_with_layout_receivers() {
    let (caller, programs) = contexts();
    for kind in 0..3 {
        for ty in [
            named("Target"),
            named("Missing"),
            Type::Named {
                name: "Target".into(),
                arguments: vec![Type::I64],
            },
        ] {
            let mut declaration = fixture(kind, 17, ty.clone());
            let failure = ty != named("Target");
            // First succeeds; the next exact type selects success or the first failure.
            visit_types_mut(&mut declaration.kind, &mut |ty| {
                *ty = named("Target");
                Ok(())
            })
            .unwrap();
            let ty_for_second = ty;
            let mut position = 0;
            visit_types_mut(&mut declaration.kind, &mut |ty| {
                if position == 1 {
                    *ty = ty_for_second.clone();
                }
                position += 1;
                Ok(())
            })
            .unwrap();
            for layout in [false, true] {
                let mut old = declaration.clone();
                let mut current = declaration.clone();
                let before = if layout {
                    original::rewrite_declaration_with_layout(
                        &mut old,
                        "fixture.provider",
                        &caller,
                        &programs,
                        None,
                    )
                } else {
                    original::rewrite_declaration(&mut old, "fixture.provider", &caller, &programs)
                };
                let after = if layout {
                    rewrite_declaration_with_layout(
                        &mut current,
                        "fixture.provider",
                        &caller,
                        &programs,
                        None,
                    )
                } else {
                    rewrite_declaration(&mut current, "fixture.provider", &caller, &programs)
                };
                assert_eq!(before, after);
                assert_eq!(old, current);
                assert_eq!(after.is_err(), failure);
                let mut old_cost = cost_bridge::new();
                let mut current_cost = cost_bridge::new();
                let before = if layout {
                    original::rewrite_declaration_runtime_cost_with_layout(
                        &declaration,
                        "fixture.provider",
                        &caller,
                        &programs,
                        &mut old_cost,
                        None,
                    )
                } else {
                    original::rewrite_declaration_runtime_cost(
                        &declaration,
                        "fixture.provider",
                        &caller,
                        &programs,
                        &mut old_cost,
                    )
                };
                let after = if layout {
                    rewrite_declaration_runtime_cost_with_layout(
                        &declaration,
                        "fixture.provider",
                        &caller,
                        &programs,
                        &mut current_cost,
                        None,
                    )
                } else {
                    rewrite_declaration_runtime_cost(
                        &declaration,
                        "fixture.provider",
                        &caller,
                        &programs,
                        &mut current_cost,
                    )
                };
                assert_eq!(before, after);
                assert_eq!(
                    cost_bridge::snapshot(&old_cost),
                    cost_bridge::snapshot(&current_cost)
                );
            }
        }
    }
}

#[test]
fn unpriced_membership_and_traversal_backing_disappears_without_ledger_changes() {
    let (caller, programs) = contexts();
    for kind in 0..3 {
        for count in [0, 1, 16, 17, 129] {
            let declaration = fixture(kind, count, named("T"));
            for cost_route in [false, true] {
                let observe = |old: bool| {
                    let mut input = declaration.clone();
                    with_limit_usage_refusal(0, || {
                        measure(|| {
                            if cost_route {
                                let mut cost = cost_bridge::new();
                                let result = if old {
                                    original::rewrite_declaration_runtime_cost(
                                        &input,
                                        "fixture.provider",
                                        &caller,
                                        &programs,
                                        &mut cost,
                                    )
                                } else {
                                    rewrite_declaration_runtime_cost(
                                        &input,
                                        "fixture.provider",
                                        &caller,
                                        &programs,
                                        &mut cost,
                                    )
                                };
                                assert!(result.is_ok());
                                assert_eq!(cost_bridge::snapshot(&cost).2[0], 0);
                            } else {
                                let result = if old {
                                    original::rewrite_declaration(
                                        &mut input,
                                        "fixture.provider",
                                        &caller,
                                        &programs,
                                    )
                                } else {
                                    rewrite_declaration(
                                        &mut input,
                                        "fixture.provider",
                                        &caller,
                                        &programs,
                                    )
                                };
                                assert!(result.is_ok());
                            }
                        })
                    })
                };
                let (((), before), refused, used, first) = observe(true);
                let (((), after), new_refused, new_used, new_first) = observe(false);
                assert!(!refused && !new_refused);
                assert_eq!(used, 0);
                assert_eq!(new_used, 0);
                assert_eq!(first, new_first);
                assert!(!before.invalid && !after.invalid);
                assert_eq!(before.live, 0);
                assert_eq!(after.live, 0);
                assert_eq!(before.allocated, before.freed);
                assert_eq!(after.allocated, after.freed);
                assert!(before.allocated > after.allocated);
                assert_eq!(after.allocated, 0);
            }
        }
    }
}

#[test]
fn alias_paid_exact_short_zero_and_sticky_refusals_keep_complete_old_outputs_through_drop() {
    let (caller, programs) = contexts();
    for kind in 0..3 {
        let declaration = fixture(kind, 17, named("Target"));
        let observe = |limit, old: bool| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut output = declaration.clone();
                    let result = if old {
                        original::rewrite_declaration(
                            &mut output,
                            "fixture.provider",
                            &caller,
                            &programs,
                        )
                    } else {
                        rewrite_declaration(&mut output, "fixture.provider", &caller, &programs)
                    };
                    let succeeded = result.is_ok();
                    if active_refused() {
                        assert!(crate::workspace_graph::reserve_builder_structure(0).is_err());
                    }
                    drop(result);
                    drop(output);
                    succeeded
                })
            })
        };
        let ((ok, before), refused, required, first) = observe(usize::MAX, true);
        let ((new_ok, after), new_refused, new_required, new_first) = observe(usize::MAX, false);
        assert!(ok && new_ok && !refused && !new_refused && required > 0);
        assert_eq!(required, new_required);
        assert_eq!(first, new_first);
        assert!(before.allocated > after.allocated);
        for limit in [required, required - 1, 0] {
            let ((old_ok, before), refused, used, first) = observe(limit, true);
            let ((new_ok, after), new_refused, new_used, new_first) = observe(limit, false);
            assert_eq!(old_ok, new_ok);
            assert_eq!(refused, new_refused);
            assert_eq!(used, new_used);
            assert_eq!(first, new_first);
            assert_eq!(refused, limit < required);
            for metrics in [before, after] {
                assert!(!metrics.invalid);
                assert_eq!(metrics.live, 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
            let execute = |old: bool| {
                let mut output = declaration.clone();
                let (result, _, _, _) = with_limit_usage_refusal(limit, || {
                    if old {
                        original::rewrite_declaration(
                            &mut output,
                            "fixture.provider",
                            &caller,
                            &programs,
                        )
                    } else {
                        rewrite_declaration(&mut output, "fixture.provider", &caller, &programs)
                    }
                });
                (output, result)
            };
            assert_eq!(execute(true), execute(false));
        }
    }
}

#[test]
fn real_workspace_linked_hir_and_graph_keep_complete_original_generic_rewrite_route() {
    let raw = [
        (
            "provider.spx",
            r#"module fixture.provider;
@id("target") record Target { @id("target.value") value:i64, }
@id("box") record Box<T> { @id("box.value") value:T, }
@id("identity") fn identity(value:i64)->i64 {value}"#,
        ),
        (
            "caller.spx",
            r#"module fixture.caller;
use type @id("target") from fixture.provider as Imported;
use type @id("box") from fixture.provider as ImportedBox;
use function @id("identity") from fixture.provider as identity;
@id("main") fn main()->i64 {identity(7)}"#,
        ),
    ];
    let sources: Vec<_> = raw
        .iter()
        .map(|(path, text)| crate::workspace_graph::WorkspaceSource {
            path: (*path).into(),
            source: crate::format::canonical(&program(text)),
        })
        .collect();
    let run = || {
        let build = crate::workspace_graph::build_owned(sources.clone()).unwrap();
        let program = build.linked_scalar_program("fixture.caller").unwrap();
        (
            format!("{program:?}"),
            crate::graph::to_hir_json(&program, "same-revision").unwrap(),
        )
    };
    let before = original_receiver_calls();
    let current = run();
    assert_eq!(original_receiver_calls(), before);
    let old = with_original_receiver(run);
    let after = original_receiver_calls();
    // This is the actual workspace route, including generic template admission,
    // runtime cost and constructor rewriting; an unused switch cannot pass.
    for operation in [0, 2, 4] {
        assert!(
            after[operation] > before[operation],
            "old route {operation} was not reached"
        );
    }
    assert_eq!(current, old);
}

#[test]
fn complete_original_receiver_and_all_joined_source_inverses_remain_byte_exact() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/borrowed-generic-type-rewrite-141387/whole-source-inverses.json"
    ))
    .unwrap();
    let inputs=[
        (include_str!("../generic_type_import.rs"),include_str!("../../../docs/evidence/borrowed-generic-type-rewrite-141387/generic_type_import.rs.before.txt")),
        (include_str!("../expected_projection/cost.rs"),include_str!("../../../docs/evidence/borrowed-generic-type-rewrite-141387/cost.rs.before.txt")),
        (include_str!("../../../tests/workspace/semantic_graph/source_inventory.rs"),include_str!("../../../docs/evidence/borrowed-generic-type-rewrite-141387/source_inventory.rs.before.txt")),
    ];
    for (index, (input, before)) in inputs.into_iter().enumerate() {
        let mut restored = input.to_owned();
        for hook in rows[index]["hooks"].as_array().unwrap() {
            let after = hook["after"].as_str().unwrap();
            let old = hook["before"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, old, 1);
        }
        assert_eq!(restored, before, "complete source {index}");
    }
    assert_eq!(include_str!("original.rs"), inputs[0].1);
    let root = inputs[0].0;
    for path in [
        "generic_type_import/original.rs",
        "generic_type_import/borrowed_tests.rs",
    ] {
        assert!(root.contains(&format!("#[cfg(test)]\n#[path = \"{path}\"]")));
        assert_eq!(
            inputs[2]
                .0
                .matches(&format!(
                    "include_str!(\"../../src/workspace_graph/{path}\")"
                ))
                .count(),
            1
        );
    }
    assert!(inputs[1]
        .0
        .contains("#[cfg(test)]\n#[path = \"cost/generic_rewrite_test_bridge.rs\"]"));
    assert_eq!(inputs[2].0.matches("\"../../src/workspace_graph/expected_projection/cost/generic_rewrite_test_bridge.rs\"").count(),1);
}
