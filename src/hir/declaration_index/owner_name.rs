//! Logarithmic borrowed queries for the unchanged tuple-key BTree indexes.
//! Ord is exactly the stored tuple order: DeclarationId::Ord, then String/str byte order.
use super::DeclarationId;
use std::{borrow::Borrow, cmp::Ordering, collections::BTreeMap};

trait OwnerName {
    fn parts(&self) -> (&DeclarationId, &str);
}
impl OwnerName for (DeclarationId, String) {
    fn parts(&self) -> (&DeclarationId, &str) {
        (&self.0, self.1.as_str())
    }
}
struct Query<'a>(&'a DeclarationId, &'a str);
impl OwnerName for Query<'_> {
    fn parts(&self) -> (&DeclarationId, &str) {
        (self.0, self.1)
    }
}
impl<'a> Borrow<dyn OwnerName + 'a> for (DeclarationId, String) {
    fn borrow(&self) -> &(dyn OwnerName + 'a) {
        self
    }
}
impl PartialEq for dyn OwnerName + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.parts() == other.parts()
    }
}
impl Eq for dyn OwnerName + '_ {}
impl PartialOrd for dyn OwnerName + '_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for dyn OwnerName + '_ {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parts().cmp(&other.parts())
    }
}

pub(super) fn get<'a>(
    values: &'a BTreeMap<(DeclarationId, String), DeclarationId>,
    owner: &DeclarationId,
    name: &str,
) -> Option<&'a DeclarationId> {
    let query = Query(owner, name);
    values.get::<dyn OwnerName + '_>(&query)
}

#[cfg(test)]
mod tests;
