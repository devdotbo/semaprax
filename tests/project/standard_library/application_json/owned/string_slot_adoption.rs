//! Exact pre-adoption wrapper sources; corpus expectations remain independent.
use std::path::Path;
const CATALOG: &[(&str, &str)] = &[
    (
        "src/model.spx",
        include_str!("string_slot_adoption/catalog_model_wrapper.txt"),
    ),
    (
        "src/order.spx",
        include_str!("string_slot_adoption/catalog_order_wrapper.txt"),
    ),
    (
        "src/restock.spx",
        include_str!("string_slot_adoption/catalog_restock_wrapper.txt"),
    ),
];
const SHIFTSIM: &[(&str, &str)] = &[
    (
        "src/model.spx",
        include_str!("string_slot_adoption/shiftsim_model_wrapper.txt"),
    ),
    (
        "src/order.spx",
        include_str!("string_slot_adoption/shiftsim_order_wrapper.txt"),
    ),
    (
        "src/schedule.spx",
        include_str!("string_slot_adoption/shiftsim_schedule_wrapper.txt"),
    ),
];
fn restore(root: &Path, sources: &[(&str, &str)]) {
    for (path, source) in sources {
        std::fs::write(root.join(path), source).unwrap();
    }
}
pub(super) fn catalog(root: &Path) {
    restore(root, CATALOG);
}
pub(super) fn shiftsim(root: &Path) {
    restore(root, SHIFTSIM);
}
pub(super) fn catalog_order() -> &'static str {
    CATALOG[1].1
}

pub(super) fn shiftsim_schedule() -> &'static str {
    SHIFTSIM[2].1
}
