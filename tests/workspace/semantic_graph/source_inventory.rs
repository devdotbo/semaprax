// Complete private-layout source inventory; original includes and literal bytes retained.
macro_rules! semantic_graph_private_layout_source_inventory {
    () => { concat!(
concat!(
        include_str!("../../src/workspace_graph/expected_projection/private_layout.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/registry.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/sorting.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/ledger.rs"),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/registry/tests.rs"
        ),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/names.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/shape.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/roots.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/owner_index.rs"),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/owner_index/tests.rs"
        ),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/authority.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/authority_invocation.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/authority_invocation/tests.rs"),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/authority_tests.rs"
        ),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/authority_view.rs"
        ),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/authority_oracle.rs"
        ),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/authority_view_tests.rs"
        ),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/tests.rs"),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/function_tests.rs"
        ),
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/function_runtime.rs"
        ),
    ),
        include_str!("semantic_graph/source_inventory.rs")
    ) };
}
