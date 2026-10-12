#[test]
fn core_validation_has_no_resolution_cleanup_build_or_physical_authority() {
    let validation = concat!(
        concat!(
            concat!(
                include_str!("../src/hir/validation.rs"),
                include_str!("../src/hir/validation/aggregate_metadata.rs"),
                include_str!("../src/hir/validation/continuation_path.rs"),
                include_str!("../src/hir/validation/continuation_path/tests.rs"),
                include_str!("../src/hir/validation/aggregate_metadata/tests.rs"),
                include_str!("../src/hir/inspection/calls.rs"),
                include_str!("../src/hir/validation/function_lookup.rs"),
                include_str!("../src/hir/validation/function_lookup/permits.rs"),
                include_str!("../src/hir/function_directory.rs"),
                include_str!("../src/hir/validation/type_requirement.rs"),
                include_str!("../src/hir/validation/canonical_loans.rs"),
                include_str!("../src/hir/validation/constructor_seed.rs"),
                include_str!("../src/hir/canonical_loan_storage.rs"),
                include_str!("../src/hir/canonical_loan_storage/deferred.rs"),
                include_str!("../src/hir/canonical_loan_storage/deferred/tests.rs"),
                include_str!("../src/hir/canonical_loan_storage/deferred/tests/selected.rs"),
                include_str!("../src/hir/validation/canonical_loans/tests.rs"),
                concat!(
                    include_str!("../src/hir/vec_constructor.rs"),
                    include_str!("../src/hir/vec_constructor/capture.rs")
                )
            ),
            include_str!("../src/hir/validation/assign_field.rs"),
            include_str!("../src/hir/validation/contract_owners.rs"),
            "\n",
            include_str!("../src/hir/validation/byte_range.rs"),
            "\n",
            include_str!("../src/hir/validation/byte_traversal.rs"),
            "\n",
            include_str!("../src/hir/validation/byte_traversal/tests.rs"),
            "\n",
            include_str!("../src/hir/validation/lexical_exit.rs"),
            "\n",
            include_str!("../src/hir/validation/record_traversal.rs"),
            "\n",
            include_str!("../src/hir/validation/string_intrinsic.rs"),
            "\n",
            include_str!("../src/string_ops/take.rs")
        ),
        include_str!("../src/hir/validation/vec_field.rs"),
        concat!(
            include_str!("../src/hir/validation/row_loan.rs"),
            "\n",
            include_str!("../src/hir/row_loan/validate.rs"),
            "\n",
            include_str!("../src/hir/row_loan/validate/budget.rs"),
            "\n",
            include_str!("../src/hir/row_loan/validate/comparisons.rs"),
            "\n",
            include_str!("../src/loan_plan/traversal/row_metered.rs"),
            "\n",
            include_str!("../src/loan_plan/traversal/row_metered/tests.rs"),
            "\n",
            include_str!("../src/hir/row_loan/producer.rs"),
            "\n",
            include_str!("../src/hir/row_loan/producer/tests.rs"),
            "\n",
            include_str!("../src/hir/validation/row_loan/budget_tests.rs")
        ),
        include_str!("../src/hir/validation/fused_selection.rs"),
        include_str!("../src/hir/validation/place_resolution.rs"),
        include_str!("../src/hir/validation/upcast.rs"),
        include_str!("../src/hir/validation/literal_format_result_tests.rs"),
        include_str!("../src/hir/validation/call_parameters.rs"),
        include_str!("../src/hir/validation/call_signature.rs"),
        include_str!("../src/hir/validation/effect_keys.rs"),
        include_str!("../src/hir/validation/call_parameters/tests.rs"),
        include_str!("../src/hir/validation/nominal_facts.rs"),
        include_str!("../src/hir/validation/nominal_facts/tests.rs"),
        include_str!("../src/hir/validation/borrowed_argument.rs"),
        include_str!("../src/hir/projected_string_view.rs"),
        include_str!("../src/hir/collection_outcome.rs"),
        include_str!("../src/hir/collection_outcome/owned.rs"),
        include_str!("../src/hir/collection_outcome/stream_input.rs"),
        include_str!("../src/hir/collection_outcome/nested.rs"),
        include_str!("../src/hir/collection_outcome/nested/profile.rs"),
        include_str!("../src/hir/validation/stdin_stream.rs"),
        include_str!("../src/stdin_stream_ops/analysis.rs"),
        include_str!("../src/hir/validation/borrowed_bytes_call_tests.rs"),
        concat!(
            include_str!("../src/hir/validation/iterator_loops.rs"),
            include_str!("../src/hir/validation/iterator_loops/field_scope.rs"),
            "\n",
            include_str!("../src/finite_byte_scratch.rs"),
            "\n",
            include_str!("../src/finite_byte_scratch/source.rs"),
            "\n",
            include_str!("../src/finite_byte_scratch/resolved.rs")
        ),
        include_str!("../src/hir/validation/string_replacement.rs"),
        include_str!("../src/hir/iterator_loop.rs"),
        include_str!("../src/hir/iterator_loop/renewal.rs"),
        include_str!("../src/hir/vec_loop_renewal.rs"),
        include_str!("../src/hir/vec_loop_renewal/consuming.rs"),
        include_str!("../src/hir/vec_loop_renewal/consuming/empty_clone.rs"),
        include_str!("../src/hir/vec_loop_renewal/consuming/tests.rs"),
        include_str!("../src/hir/vec_loop_renewal/tests.rs"),
        include_str!("../src/hir/validation/callable_types.rs"),
        include_str!("../src/hir/validation/closure.rs"),
        include_str!("../src/hir/generic_result.rs"),
        include_str!("../src/hir/generic_mapping.rs"),
        include_str!("../src/hir/validation/box_intrinsic.rs"),
        include_str!("../src/hir/validation/branch_merge.rs"),
        include_str!("../src/hir/validation/borrowed_str.rs"),
        include_str!("../src/hir/validation/generic_template.rs"),
        include_str!("../src/hir/validation/generic_variant.rs"),
        include_str!("../src/hir/validation/generic_record_composition.rs"),
        include_str!("../src/hir/validation/host_command.rs"),
        concat!(
            include_str!("../src/hir/validation/owned_buffer.rs"),
            "\n",
            include_str!("../src/hir/validation/owned_buffer/iterator_view.rs"),
            "\n",
            include_str!("../src/hir/validation/owned_buffer/iterator_view/tests.rs")
        ),
        include_str!("../src/hir/validation/owned_result_try.rs"),
        include_str!("../src/hir/validation/owner_renewal.rs"),
        concat!(
            include_str!("../src/loop_calls/byte_writer.rs"),
            "\n",
            include_str!("../src/loop_calls/byte_writer/source.rs"),
            "\n",
            include_str!("../src/loop_calls/byte_writer/source_index.rs")
        ),
        concat!(
            include_str!("../src/loop_calls/byte_writer/resolved.rs"),
            "\n",
            include_str!("../src/loop_calls/byte_writer/resolved/selected.rs")
        ),
        include_str!("../src/loop_calls/byte_writer/bindings.rs"),
        concat!(
            include_str!("../src/loop_calls/byte_writer/module_index.rs"),
            "\n",
            include_str!("../src/loop_calls/byte_writer/module_index/directory.rs"),
            "\n",
            include_str!("../src/loop_calls/byte_writer/module_index/textual.rs"),
            include_str!("../src/loop_calls/byte_writer/module_index/ordinary.rs"),
            include_str!("../src/loop_calls/byte_writer/module_index/ordinary/numeric.rs"),
            include_str!("../src/hir/renewal_probes.rs"),
            include_str!("../src/hir/renewal_probes/phase.rs"),
            include_str!("../src/cleanup_plan/replay/ordinary_renewals.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/compare.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/consuming.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/debug_contains.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/iterator.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/iterator_shape.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/leaf.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/mod.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/scratch.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/sort.rs"),
            include_str!("../src/cleanup_plan/renewal_profile/ordinary/vector.rs")
        ),
        include_str!("../src/hir/validation/proof_return.rs"),
        include_str!("../src/hir/validation/identity_scratch.rs"),
        include_str!("../src/hir/validation/identity_scratch/tests.rs"),
        include_str!("../src/hir/validation/type_profiles.rs"),
        include_str!("../src/hir/validation/type_profiles/owned_bytes_tests.rs"),
        include_str!("../src/hir/validation/type_profiles/owned_bytes_tests/old.rs"),
        include_str!("../src/hir/validation/unsafe_scan.rs"),
        include_str!("../src/hir/validation/vec_intrinsic.rs"),
        include_str!("../src/hir/validation/native_borrow.rs"),
        include_str!("../src/hir/validation/variant_or.rs"),
        include_str!("../src/hir/validation/variant_guard_state.rs"),
        include_str!("../src/hir/validation/iterative_while_admission_tests.rs"),
    );
    for forbidden in [
        "struct Resolver",
        "impl Resolver",
        "fn scoped_identity(",
        "fn materialize_function_template",
        "fn specialize_source_function",
        "pub fn resolve(",
        "crate::cleanup::build_inventory",
        "crate::cleanup_plan::build_plan",
        "std::fs",
        "std::process",
        "platform::",
    ] {
        assert!(
            !validation.contains(forbidden),
            "core HIR validation admitted `{forbidden}`"
        );
    }

    let root = include_str!("../src/hir.rs");
    assert!(root.contains("mod validation;"));
    assert!(
        root.contains("pub(crate) use inspection::validate_core_with_storage as validate_core;")
    );

    let format_macro = root.find("macro_rules! format {").unwrap();
    let validation_module = root.find("mod validation;").unwrap();
    assert!(format_macro < validation_module);
    assert!(root[format_macro..validation_module]
        .contains("crate::bounded_output::budgeted_format(format_args!"));
    assert!(validation.contains("format!("));
    for forbidden in ["std::format", "::format!", "use std::format"] {
        assert!(
            !validation.contains(forbidden),
            "core HIR validation bypassed the budgeted format macro with `{forbidden}`"
        );
    }
}
