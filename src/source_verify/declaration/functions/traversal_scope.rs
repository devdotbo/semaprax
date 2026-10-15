//! Ordinary static bindings used while replaying lexical generic mappings.
use super::*;

pub(super) fn static_binding(ty: Type) -> Binding {
    Binding {
        contract_local: false,
        traversal_row: None,
        ty,
        mode: ParamMode::Value,
        availability: Availability::Available,
        moved_places: HashMap::new(),
        definitely_partial: HashSet::new(),
        native_unit_discard: false,
        mutable: false,
        active_loans: BTreeSet::new(),
        borrow_origin: None,
    }
}
