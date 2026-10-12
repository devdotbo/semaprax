//! Saved source snapshot publication from 3b4610391.
use super::*;
pub(super) struct Publication<'a> {
    pub(super) target: &'a mut BTreeMap<ValueId, ValidationBinding>,
    pub(super) published: BTreeMap<ValueId, ValidationBinding>,
    pub(super) enabled: bool,
}
impl Publication<'_> {
    pub(super) fn publish(&mut self, scope: &BTreeMap<ValueId, ValidationBinding>) {
        if self.enabled {
            self.published.clone_from(scope);
        }
    }
}
impl Drop for Publication<'_> {
    fn drop(&mut self) {
        std::mem::swap(self.target, &mut self.published);
    }
}
