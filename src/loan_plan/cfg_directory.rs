//! Exact caller-paid private ordered CFG scratch. No attached plan supplies
//! lookup or sizing authority; the public builder keeps its old BTree policy.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
#[cfg(test)]
mod tests;
mod tree;

pub(super) struct Map<K, V> {
    root: Option<Box<tree::Node<K, V>>>,
    len: usize,
}
impl<K: Ord, V> Map<K, V> {
    pub(super) fn new(payer: &CfgReservation) -> Result<Self, Diagnostic> {
        payer
            .reserve(std::mem::size_of::<Self>())
            .map_err(|_| storage::marker())?;
        Ok(Self { root: None, len: 0 })
    }
    pub(super) fn get(&self, key: &K) -> Option<&V> {
        tree::get(self.root.as_deref(), key)
    }
    fn insert(
        &mut self,
        key: K,
        value: V,
        payer: &CfgReservation,
    ) -> Result<Option<V>, Diagnostic> {
        payer.reserve(0).map_err(|_| storage::marker())?;
        let mut leaf = if self.get(&key).is_none() {
            let Some(bytes) = std::mem::size_of::<tree::Node<K, V>>().checked_mul(2) else {
                payer.arithmetic();
                return Err(storage::marker());
            };
            // Box heap and literal initializer are paid before construction.
            payer.reserve(bytes).map_err(|_| storage::marker())?;
            Some(Box::new(tree::Node::leaf(key, value)))
        } else {
            return Ok(tree::replace(&mut self.root, &key, value));
        };
        // No allocation, payer call or failure follows the prepared leaf.
        tree::insert(&mut self.root, &mut leaf);
        self.len += 1;
        Ok(None)
    }
}
pub(super) struct Set<K>(Map<K, ()>);
impl<K: Ord> Set<K> {
    pub(super) fn new(payer: &CfgReservation) -> Result<Self, Diagnostic> {
        Map::new(payer).map(Self)
    }
    pub(super) fn into_vec(mut self, payer: &CfgReservation) -> Result<Vec<K>, Diagnostic> {
        let mut values = vector(self.0.len, payer)?;
        while let Some((key, ())) = tree::pop_first(&mut self.0.root) {
            values.push(key);
        }
        Ok(values)
    }
}
pub(super) fn map_insert<K: Ord, V>(
    values: &mut Map<K, V>,
    key: K,
    value: V,
    payer: Option<&CfgReservation>,
) -> Result<Option<V>, Diagnostic> {
    values.insert(key, value, payer.ok_or_else(storage::marker)?)
}
pub(super) fn set_insert<K: Ord>(
    values: &mut Set<K>,
    key: K,
    payer: Option<&CfgReservation>,
) -> Result<bool, Diagnostic> {
    values
        .0
        .insert(key, (), payer.ok_or_else(storage::marker)?)
        .map(|old| old.is_none())
}
pub(super) fn vector<T>(capacity: usize, payer: &CfgReservation) -> Result<Vec<T>, Diagnostic> {
    payer
        .reserve(std::mem::size_of::<Vec<T>>())
        .map_err(|_| storage::marker())?;
    storage::vector(capacity, Some(payer))
}
pub(super) fn fresh<T>(payer: &CfgReservation) -> Result<Vec<T>, Diagnostic> {
    vector(0, payer)
}
pub(super) enum Nodes {
    Legacy(BTreeMap<LoanProgramPoint, u16>),
    Funded(Map<LoanProgramPoint, u16>),
}
impl Nodes {
    pub(super) fn get(&self, key: &LoanProgramPoint) -> Option<&u16> {
        match self {
            Self::Legacy(values) => values.get(key),
            Self::Funded(values) => values.get(key),
        }
    }
}
