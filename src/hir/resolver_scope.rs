//! Call-local shared scopes: borrow the complete input until an actual mutation.
use super::Binding;
use std::{borrow::Cow, collections::BTreeMap, rc::Rc};
pub(super) type Scope<'scope> = Rc<Cow<'scope, BTreeMap<String, Binding>>>;
pub(super) fn borrowed(bindings: &BTreeMap<String, Binding>) -> Scope<'_> {
    Rc::new(Cow::Borrowed(bindings))
}
#[cfg(test)]
pub(super) fn owned(bindings: BTreeMap<String, Binding>) -> Scope<'static> {
    Rc::new(Cow::Owned(bindings))
}
#[cfg(test)]
pub(super) fn owned_capacity(scope: &Scope<'_>) -> usize {
    // Real Rc value/control storage is retained even when its map is borrowed.
    std::mem::size_of::<Cow<'_, BTreeMap<String, Binding>>>()
        + 2 * std::mem::size_of::<usize>()
        + match scope.as_ref() {
            Cow::Borrowed(_) => 0,
            Cow::Owned(map) => super::capacity_probe::resolver_scope_owned_capacity(map),
        }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod source_audit;
