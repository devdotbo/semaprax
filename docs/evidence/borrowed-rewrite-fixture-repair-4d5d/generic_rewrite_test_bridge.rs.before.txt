//! Test-only full-field access; no production cost or visibility changes.
use super::StructuralCost;
pub(in crate::workspace_graph) fn new() -> StructuralCost {
    StructuralCost::new()
}
pub(in crate::workspace_graph) fn snapshot(cost: &StructuralCost) -> (bool, bool, [usize; 15]) {
    let StructuralCost {
        total,
        string_bytes,
        embedded_headers,
        inline_values,
        scalar_identity_discount,
        literal_fixed_discount,
        identity_carriers,
        single_identity_carriers,
        exact_fixed_discount,
        exact_expressions,
        exact_absent_roots,
        exact_carrier_free_roots,
        exact_carrier_free_ifs,
        exact_carrier_free_vars,
        exact_carrier_free_blocks,
    } = cost;
    (
        *embedded_headers,
        *inline_values,
        [
            *total,
            *string_bytes,
            *scalar_identity_discount,
            *literal_fixed_discount,
            *identity_carriers,
            *single_identity_carriers,
            *exact_fixed_discount,
            *exact_expressions,
            exact_absent_roots[0],
            exact_absent_roots[1],
            exact_absent_roots[2],
            *exact_carrier_free_roots,
            *exact_carrier_free_ifs,
            *exact_carrier_free_vars,
            *exact_carrier_free_blocks,
        ],
    )
}
