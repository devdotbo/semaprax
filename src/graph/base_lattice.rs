//! The ordered scalar tail of the frozen Graph base lattice, v17 down to v10.
//! The from-parts/full-program base classifier and the retained scalar lexical
//! receiver share this one mapping. Each caller supplies its own independently
//! derived predicates; they are evaluated lazily, in exactly this order.

/// Select the first family whose predicate holds. A caller may pass a fixed
/// `false` only for a family its own exhaustive census has excluded.
pub(crate) fn scalar_base_lattice(
    has_usize: impl FnOnce() -> bool,
    refutable_match: impl FnOnce() -> bool,
    body_while: impl FnOnce() -> bool,
    templates: bool,
    record_pattern: impl FnOnce() -> bool,
    generic_record: impl FnOnce() -> bool,
    option_try: impl FnOnce() -> bool,
) -> &'static str {
    // Portable Indexed Byte Data v1 selects v17 above the existing v16/v15
    // schemas whenever the authenticated program carries target-independent
    // usize meaning or a borrowed byte view.
    // Programs without the new scalar retain byte-identical v10-v16 output.
    if has_usize() {
        return "semaprax.graph.v17";
    }
    // Refutable Match v1 selects v16 above the whole lower lattice (including
    // the v15 while extension) only when an authenticated refutable node
    // exists; programs without refutable-match syntax keep their exact
    // pre-existing schema and bytes.
    if refutable_match() {
        return "semaprax.graph.v16";
    }
    // Bounded While-Loops v1 selects v15 above the whole lower lattice only
    // when an authenticated while node exists; programs without while syntax
    // keep their exact pre-existing schema and bytes.
    if body_while() {
        return "semaprax.graph.v15";
    }
    if templates {
        return "semaprax.graph.v14";
    }
    if record_pattern() {
        return "semaprax.graph.v13";
    }
    if generic_record() {
        return "semaprax.graph.v12";
    }
    if option_try() {
        "semaprax.graph.v11"
    } else {
        "semaprax.graph.v10"
    }
}
