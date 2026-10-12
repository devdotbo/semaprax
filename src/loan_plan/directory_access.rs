//! Private receiver seam: the saved BTree policy and exact paid directory
//! share borrowed lookups, never allocation estimates or attached authority.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;

pub(super) trait Receiver<K: Ord, V> {
    fn insert_paid(
        &mut self,
        key: K,
        value: V,
        payer: Option<&CfgReservation>,
    ) -> Result<Option<V>, Diagnostic>;
}
impl<K: Ord, V> Receiver<K, V> for BTreeMap<K, V> {
    fn insert_paid(
        &mut self,
        key: K,
        value: V,
        payer: Option<&CfgReservation>,
    ) -> Result<Option<V>, Diagnostic> {
        storage::map_insert(self, key, value, payer)
    }
}
impl<K: Ord, V> Receiver<K, V> for cfg_directory::Map<K, V> {
    fn insert_paid(
        &mut self,
        key: K,
        value: V,
        payer: Option<&CfgReservation>,
    ) -> Result<Option<V>, Diagnostic> {
        cfg_directory::map_insert(self, key, value, payer)
    }
}
pub(super) trait Aliases {
    fn resolve(&self, place: Place, work: &mut WorkCounter) -> Result<Place, Diagnostic>;
}
impl Aliases for BTreeMap<ValueId, Place> {
    fn resolve(&self, place: Place, work: &mut WorkCounter) -> Result<Place, Diagnostic> {
        resolve_origin(self, place, work)
    }
}
impl Aliases for cfg_directory::Map<ValueId, Place> {
    fn resolve(&self, place: Place, work: &mut WorkCounter) -> Result<Place, Diagnostic> {
        origin::funded::resolve(self, place, work)
    }
}
