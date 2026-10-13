use super::*;

#[test]
fn raw_inventory_classifier_keeps_complete_original_diagnostic_fields() {
    for program in tests::fixtures() {
        for function in &program.functions {
            let mut budget = ReplayBudget::new();
            let (results, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                    .unwrap();
                let mut results = Vec::new();
                for ty in function
                    .params
                    .iter()
                    .map(|p| &p.ty)
                    .chain(std::iter::once(&function.return_type))
                {
                    results.push((
                        crate::cleanup::type_needs_resource_cleanup(&program, ty)
                            .map_err(|m| Diagnostic::io("SPX-H006", m)),
                        inventory_needs_drop(
                            &program,
                            ty,
                            &mut tests::payer(function, &mut budget),
                        ),
                    ));
                }
                results
            });
            assert!(!refused);
            for (old, current) in results {
                assert_eq!(current, old);
            }
        }
    }
}

#[test]
fn raw_missing_cleanup_facts_refusal_keeps_message_without_replay_prefix() {
    let program = tests::fixtures().into_iter().next().unwrap();
    let function = program.functions.first().unwrap();
    let ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("missing.raw.inventory"),
        arguments: Vec::new(),
    };
    let old = crate::cleanup::type_needs_resource_cleanup(&program, &ty)
        .map_err(|m| Diagnostic::io("SPX-H006", m));
    let (current, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget).unwrap();
        inventory_needs_drop(&program, &ty, &mut tests::payer(function, &mut budget))
    });
    assert!(!refused);
    assert_eq!(current, old);
    assert!(current.is_err());
}

#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
    let (_, tail) = rest
        .split_once("#[cfg(test)]\nthread_local! { static SAVED:")
        .unwrap();
    let projected = format!("{head}#[cfg(test)]\nthread_local! {{ static SAVED:{tail}");
    let projected = projected
        .strip_suffix("\nmod inventory_text;\n")
        .unwrap()
        .strip_suffix("\n#[cfg(test)]\nmod inventory_substitute_tests;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_substitute;\n\nmod inventory_clone;\n")
        .unwrap()
        .strip_suffix("\n#[cfg(test)]\nmod inventory_tests;\n")
        .unwrap();
    assert_eq!(
        projected,
        include_str!("../../../../docs/evidence/schema-summary/paid_type_facts.rs.before")
    );
}

#[test]
fn full_droppability_source_keeps_complete_original_plus_raw_text_adapter() {
    let current = include_str!("droppability.rs");
    let (original, adapter) = current
        .split_once("\n// Raw ordinary inventory text")
        .expect("documented additive raw text adapter");
    assert_eq!(
        original,
        include_str!("../../../../docs/evidence/schema-summary/droppability.rs.before")
    );
    assert!(adapter.contains("text::raw(payer, message)"));
}
