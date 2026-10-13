// Shared complete byte-writer/frontier source join for both owning audits.
// No test bodies, assertions or literal contents are relocated here.
macro_rules! byte_writer_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../src/loop_calls/byte_writer.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/source.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/source_index.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/resolved.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/resolved/validation.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/validation_frontier.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/validation_frontier/storage.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/validation_frontier/ordinary_receiver.rs"),
            include_str!("../../../../../src/cleanup_plan/renewal_profile/ordinary/validation.rs"),
            include_str!("../../../../../src/cleanup_plan/build/helper_index/ordinary_view.rs"),
            include_str!("../../../../../src/cleanup_plan/build/helper_index/validation.rs"),
            include_str!("../../../../../src/cleanup_plan/build/lexical_control/validation.rs"),
            include_str!("../../../../../src/cleanup_plan/validate.rs"),
            include_str!("../../../../../src/cleanup_plan/validate/owned.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/bindings.rs"),
            include_str!("../../../../../src/loop_calls/byte_writer/module_index.rs"),
        )
    };
}

// Full generic directory implementation and unchanged original test owner.
macro_rules! generic_values_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../src/hir/validation/generic_template/values.rs"),
            include_str!("../../../../../src/hir/validation/generic_template/values/charge.rs"),
            include_str!(
                "../../../../../src/hir/validation/generic_template/values/charge/refusal_tests.rs"
            ),
            include_str!(
                "../../../../../src/hir/validation/generic_template/values/refusal_tests.rs"
            ),
            include_str!("../../../../../src/hir/validation/generic_template/values/tree.rs"),
            include_str!("../../../../../src/hir/validation/generic_template/values/tests.rs"),
            include_str!(
                "../../../../../src/hir/validation/generic_template/values/checkpoint_tests.rs"
            ),
        )
    };
}

// Complete original loan scratch source join, plus private ordered carrier.
macro_rules! loan_cfg_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../src/loan_plan/storage.rs"),
            include_str!("../../../../../src/loan_plan/funded_flat.rs"),
            include_str!("../../../../../src/loan_plan/funded_flat/tests.rs"),
            include_str!("../../../../../src/loan_plan/paid_candidate.rs"),
            include_str!("../../../../../src/loan_plan/paid_cfg.rs"),
            include_str!("../../../../../src/loan_plan/paid_children.rs"),
            include_str!("../../../../../src/loan_plan/paid_patterns.rs"),
            include_str!("../../../../../src/loan_plan/paid_plan.rs"),
            include_str!("../../../../../src/loan_plan/paid_predicates.rs"),
            include_str!("../../../../../src/loan_plan/paid_substitute.rs"),
            include_str!("../../../../../src/loan_plan/paid_substitute/tests.rs"),
            include_str!("../../../../../src/loan_plan/paid_materialize.rs"),
            include_str!("../../../../../src/loan_plan/traversal/paid.rs"),
            include_str!("../../../../../src/bounded_output/cfg_reservation.rs"),
            include_str!("../../../../../src/bounded_output/cfg_reservation/tests.rs"),
            include_str!("../../../../../src/loan_plan/cfg_directory.rs"),
            include_str!("../../../../../src/loan_plan/cfg_directory/tree.rs"),
            include_str!("../../../../../src/loan_plan/cfg_directory/tests.rs"),
            include_str!("../../../../../src/loan_plan/adjacency.rs"),
            include_str!("../../../../../src/loan_plan/directory_access.rs"),
            include_str!("../../../../../src/loan_plan/origin.rs"),
            include_str!("../../../../../src/loan_plan/origin/funded.rs"),
            include_str!("../../../../../src/loan_plan/native_view.rs"),
            include_str!("../../../../../src/loan_plan/traversal.rs"),
            include_str!("../../../../../src/loan_plan/funded_flat/directories_tests.rs"),
            include_str!("../../../../../src/loan_plan/cfg_directory/planner_receivers_tests.rs"),
            include_str!("../../../../../src/loan_plan/funded_flat_legacy.rs"),
            include_str!("../../../../../src/loan_plan/native_view_legacy.rs"),
            include_str!("../../../../../src/loan_plan/paid_candidate_legacy.rs"),
            include_str!("../../../../../src/loan_plan/paid_patterns_legacy.rs"),
            include_str!("../../../../../src/loan_plan/paid_plan_legacy.rs"),
            include_str!("../../../../../src/loan_plan/traversal/paid_legacy.rs"),
            include_str!("../../../../../src/loan_plan/paid_live.rs"),
            include_str!("../../../../../src/loan_plan/paid_live/tests.rs"),
            include_str!("../../../../../src/loan_plan/cfg_directory/live_receivers_tests.rs"),
            include_str!("../../../../../src/loan_plan/call_arguments_legacy.rs"),
            include_str!("../../../../../src/loan_plan/paid_materialize_legacy.rs"),
            include_str!("../../../../../src/loan_plan/paid_predicates_legacy.rs"),
        )
    };
}

// Full static diagnostic and ordinary scope/frontier owners, including old tests.
macro_rules! diagnostic_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../src/diagnostic.rs"),
            include_str!("../../../../../src/diagnostic/refusal_tests.rs"),
            include_str!("../../../../../src/workspace_report.rs"),
            include_str!("../../../../../src/workspace_report/tests.rs"),
            include_str!("../../../../../src/semantic_workspace/reporting.rs"),
            include_str!("../../../../../src/semantic_workspace/reporting/tests.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/charge.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/tree.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/views.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/merge.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/marks.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/storage.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/paths.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/tests.rs"),
            include_str!("../../../../../src/hir/validation/scope_checkpoint/refusal_tests.rs"),
            include_str!("../../../../../src/hir/validation/iterator_loops/field_scope.rs"),
            include_str!(
                "../../../../../src/hir/validation/iterator_loops/field_scope/refusal_tests.rs"
            ),
        )
    };
}

// Complete substituters and the saved current caller, alongside original gates.
macro_rules! substitution_leaf_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../src/hir/monomorphize.rs"),
            include_str!("../../../../../src/hir/monomorphize/substitution_saved.rs"),
            include_str!("../../../../../src/hir/monomorphize/substitution_leaf_tests.rs"),
        )
    };
}
macro_rules! reachability_leaf_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../src/hir/type_reachability.rs"),
            include_str!("../../../../../src/hir/type_reachability/leaf_saved.rs"),
            include_str!("../../../../../src/hir/type_reachability/leaf_tests.rs"),
            include_str!("../../../../../src/hir/type_reachability/nominal_saved.rs"),
            include_str!("../../../../../src/hir/type_reachability/nominal_tests.rs"),
        )
    };
}

// Joined source audit inventory for the additive final-validation children.
macro_rules! hir_final_validation_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../src/hir/nodes/import_result_type.rs"),
            include_str!("../../../../../src/hir/nodes/import_result_type/tests.rs"),
            include_str!("../../../../../src/hir/validation/callable_types/inline_frames.rs"),
            include_str!("../../../../../src/hir/validation/callable_types/inline_frames/tests.rs"),
            include_str!("../../../../../src/hir/validation/callable_types/original_receiver.rs"),
            include_str!("ledger_capacity/source_inventory.rs")
        )
    };
}
