//! Exact finite work-site census, pinned to the complete compiler AST producers.
//! Producer reads actual AST; this metadata gives no authored-source authority.
pub(super) const NAMES: [&str; 9] = [
    "Vec",
    "Box",
    "Iter",
    "IterStep",
    "List",
    "ListStep",
    "StdinReader",
    "Map",
    "Set",
];
const fn text(value: &str) -> usize {
    2 * value.len()
}
const fn record(id: &str, name: &str, parameters: &[&str]) -> usize {
    let mut n = 3 + id.len() + text(name);
    let mut i = 0;
    while i < parameters.len() {
        n += 1 + text(parameters[i]);
        i += 1;
    }
    n
}
const fn nominal_argument(current: usize, target: usize) -> usize {
    let mut n = 1;
    let mut i = 0;
    while i < current {
        n += NAMES[i].len() + NAMES[target].len();
        if i == target {
            break;
        }
        i += 1;
    }
    n + 3 // enter T, compare its two one-byte spellings
}
const fn case(id: &str, name: &str) -> usize {
    1 + id.len() + text(name)
}
const fn field(id: &str, name: &str, ty: usize) -> usize {
    1 + id.len() + text(name) + ty
}
const fn iterator_step() -> usize {
    record(crate::iterator_ops::STEP_ID, "IterStep", &["T"])
        + case(crate::iterator_ops::DONE_ID, "Done")
        + case(crate::iterator_ops::YIELD_ID, "Yield")
        + field(crate::iterator_ops::ITEM_ID, "item", 3)
        + field(crate::iterator_ops::REST_ID, "rest", nominal_argument(3, 2))
}
const fn list_step() -> usize {
    record(crate::list_ops::STEP_ID, "ListStep", &["T"])
        + case(crate::list_ops::NIL_CASE_ID, "Nil")
        + case(crate::list_ops::CONS_CASE_ID, "Cons")
        + field(crate::list_ops::HEAD_ID, "head", 3)
        + field(crate::list_ops::TAIL_ID, "tail", nominal_argument(5, 4))
}
pub(super) const WORK: [usize; 9] = [
    record(crate::prelude::VEC_ID, "Vec", &["T"]),
    record(crate::prelude::BOX_ID, "Box", &["T"]),
    record(crate::iterator_ops::ITER_ID, "Iter", &["T"]),
    iterator_step(),
    record(crate::list_ops::LIST_ID, "List", &["T"]),
    list_step(),
    record(crate::stdin_stream_ops::READER_ID, "StdinReader", &[]),
    record(crate::map_ops::MAP_ID, "Map", &["K", "V"]),
    record(crate::map_ops::SET_ID, "Set", &["K"]),
];
