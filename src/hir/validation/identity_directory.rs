//! Paid ordered identity storage. Only shared immutable identity keys are admitted.
//! Snapshots share a root; insertion stages the shared suffix before mutation.
use super::*;
use std::rc::Rc;
mod iter;
#[cfg(test)]
mod oracle_entries;
#[cfg(test)]
mod tests;
mod transaction;
mod sealed {
    pub trait Key {}
    impl Key for super::ExpressionId {}
    impl Key for super::ValueId {}
}
pub(super) trait Key: sealed::Key + Ord + Clone + std::fmt::Debug {}
impl Key for ExpressionId {}
impl Key for ValueId {}
type Root<K> = Option<Rc<Node<K>>>;
struct Node<K> {
    key: K,
    left: Root<K>,
    right: Root<K>,
    height: usize,
    count: usize,
}
#[derive(Clone)]
pub(super) struct Set<K: Key> {
    root: Root<K>,
    // Independent original standard-library storage for complete saved receivers.
    #[cfg(test)]
    legacy: Option<BTreeSet<K>>,
}
impl<K: Key> Set<K> {
    pub(super) fn new() -> Self {
        Self {
            root: None,
            #[cfg(test)]
            legacy: None,
        }
    }
    pub(super) fn len(&self) -> usize {
        #[cfg(test)]
        if let Some(values) = &self.legacy {
            return values.len();
        }
        count(&self.root)
    }
    pub(super) fn contains(&self, key: &K) -> bool {
        #[cfg(test)]
        if let Some(values) = &self.legacy {
            return values.contains(key);
        }
        let mut current = self.root.as_deref();
        while let Some(node) = current {
            match key.cmp(&node.key) {
                std::cmp::Ordering::Less => current = node.left.as_deref(),
                std::cmp::Ordering::Greater => current = node.right.as_deref(),
                std::cmp::Ordering::Equal => return true,
            }
        }
        false
    }
    pub(super) fn insert_paid(&mut self, key: &K) -> Result<bool, Diagnostic> {
        // Duplicate priority precedes acquisition, including a sticky refusal.
        if self.contains(key) {
            return Ok(false);
        }
        #[cfg(test)]
        if let Some(values) = &mut self.legacy {
            return identity_insert::coarse::insert(values, key);
        }
        transaction::insert(&mut self.root, key)?;
        Ok(true)
    }
    pub(super) fn iter(&self) -> iter::Iter<'_, K> {
        #[cfg(test)]
        if let Some(values) = &self.legacy {
            return iter::Iter::Legacy(values.iter());
        }
        iter::Iter::Paid(iter::Paid::new(self.root.as_deref(), self.len()))
    }
    #[cfg(test)]
    pub(super) fn is_legacy(&self) -> bool {
        self.legacy.is_some()
    }
    #[cfg(test)]
    pub(super) fn use_legacy(&mut self) {
        if self.legacy.is_none() {
            let values = self.iter().cloned().collect();
            self.root = None;
            self.legacy = Some(values);
        }
    }
    #[cfg(test)]
    pub(super) fn insert(&mut self, key: K) -> bool {
        if let Some(values) = &mut self.legacy {
            return values.insert(key);
        }
        self.insert_paid(&key)
            .expect("owning identity fixture setup")
    }
    #[cfg(test)]
    pub(super) fn into_btree_projection(self) -> BTreeSet<K> {
        if let Some(values) = self.legacy {
            return values;
        }
        iter::Paid::new(self.root.as_deref(), count(&self.root))
            .cloned()
            .collect()
    }
}
impl<K: Key> std::fmt::Debug for Set<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
impl<K: Key> PartialEq for Set<K> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<K: Key> Eq for Set<K> {}
impl<'a, K: Key> IntoIterator for &'a Set<K> {
    type Item = &'a K;
    type IntoIter = iter::Iter<'a, K>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
fn count<K>(root: &Root<K>) -> usize {
    root.as_ref().map_or(0, |node| node.count)
}
fn height<K>(root: &Root<K>) -> usize {
    root.as_ref().map_or(0, |node| node.height)
}
