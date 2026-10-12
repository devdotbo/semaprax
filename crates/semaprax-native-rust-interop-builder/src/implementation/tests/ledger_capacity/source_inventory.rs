// Shared complete byte-writer/frontier source join for both owning audits.
// No test bodies, assertions or literal contents are relocated here.
macro_rules! byte_writer_source_inventory {
    () => {
        concat!(
            include_str!("../../../../../../src/loop_calls/byte_writer.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/source.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/source_index.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/resolved.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/resolved/validation.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/validation_frontier.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/validation_frontier/storage.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/validation_frontier/ordinary_receiver.rs"),
            include_str!("../../../../../../src/cleanup_plan/renewal_profile/ordinary/validation.rs"),
            include_str!("../../../../../../src/cleanup_plan/build/helper_index/ordinary_view.rs"),
            include_str!("../../../../../../src/cleanup_plan/build/helper_index/validation.rs"),
            include_str!("../../../../../../src/cleanup_plan/build/lexical_control/validation.rs"),
            include_str!("../../../../../../src/cleanup_plan/validate.rs"),
            include_str!("../../../../../../src/cleanup_plan/validate/owned.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/bindings.rs"),
            include_str!("../../../../../../src/loop_calls/byte_writer/module_index.rs"),
        )
    };
}
