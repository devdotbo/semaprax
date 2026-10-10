use super::*;
#[test]
fn explicit_owned_keys_sort_without_allocation_clone_or_authority_replacement_o0_o2() {
    native::run(
        include_str!("../../fixtures/explicit-owned-sort-keys.spx"),
        0,
        29,
        native::Failure::None,
    );
}
