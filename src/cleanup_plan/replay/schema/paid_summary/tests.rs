use super::*;
#[test]
fn complete_schema_original_and_receiving_body_remain_byte_exact() {
    let root = include_str!("../../schema.rs");
    assert_eq!(
        root.strip_suffix("\npub(super) mod paid_summary;\n")
            .unwrap(),
        include_str!("../../../../../docs/evidence/schema-summary/schema.rs.before")
    );
    let old=include_str!("original_receiver.rs").strip_prefix("//! Complete pre-adoption receiving function, independently compiled for controls.\nuse super::*;\n").unwrap();
    assert_eq!(
        old.replacen(
            "pub(super) fn validate_original<'hir>(",
            "fn validate_structure_with_budget_storage<'hir>(",
            1
        ),
        include_str!("../../../../../docs/evidence/schema-summary/validation-receiver.rs.before")
    );
}
fn original(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
    original_receiver::validate_original(program, function, &mut budget, None, None)
}
fn current(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
    validate_structure_with_budget(program, function, &mut budget, None)
}
#[test]
fn full_schema_and_receiving_oracle_keep_late_tampering_all_diagnostic_fields() {
    for program in paid_type_facts::tests::fixtures() {
        for function in &program.functions {
            for replacement in [None, Some("forged.schema")] {
                let mut subject = function.clone();
                if let Some(schema) = replacement {
                    subject.cleanup_plan.schema = schema;
                }
                let expected = crate::bounded_output::with_limit_usage(usize::MAX, || {
                    original(&program, &subject)
                })
                .0;
                let (actual, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        current(&program, &subject)
                    });
                assert!(!refused);
                assert_eq!(actual, expected);
            }
            if !function.cleanup_plan.slots.is_empty() {
                let mut subject = function.clone();
                subject.cleanup_plan.slots[0].storage_index =
                    subject.cleanup_plan.slots[0].storage_index.wrapping_add(1);
                let expected = crate::bounded_output::with_limit_usage(usize::MAX, || {
                    original(&program, &subject)
                })
                .0;
                let (actual, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        current(&program, &subject)
                    });
                assert!(!refused);
                assert_eq!(actual, expected);
            }
        }
    }
}
#[test]
fn full_unfunded_compatibility_and_selected_reservation_receiver_stay_original() {
    for program in paid_type_facts::tests::fixtures() {
        for function in &program.functions {
            let expected = super::super::selected_schema_with_index(&program, function, None);
            let mut budget = ReplayBudget::new();
            assert!(!budget.pays_builder());
            assert_eq!(
                selected_with_budget(&program, function, None, &mut budget),
                expected
            );
            let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
            assert!(!budget.pays_builder());
            assert_eq!(
                selected_with_budget(&program, function, None, &mut budget),
                expected
            );
        }
    }
}
