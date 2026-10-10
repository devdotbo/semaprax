//! Canonical block provenance retained alongside each producer transition.
use super::*;
use crate::cleanup_plan::{BlockId, CleanupPlan};

pub(super) struct Tagged {
    pub(super) block: BlockId,
    pub(super) ordinary: bool,
    pub(super) transition: CleanupTransition,
}
pub(super) struct Index<K>(BTreeMap<K, Vec<Tagged>>);
impl<K: Ord> Default for Index<K> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}
impl<K: Ord> Index<K> {
    pub(super) fn all(&self) -> impl Iterator<Item = &CleanupTransition> {
        self.0.values().flatten().map(|row| &row.transition)
    }
    pub(super) fn push(&mut self, key: K, row: Tagged) {
        self.0.entry(key).or_default().push(row);
    }
    pub(super) fn insert_context(
        &mut self,
        key: K,
        rows: Vec<Tagged>,
        plan: &CleanupPlan,
    ) -> Result<bool, Diagnostic> {
        let previous = self.0.entry(key).or_default();
        let overlaps = previous.iter().any(|prior| {
            rows.iter().any(|next| {
                if prior.ordinary && next.ordinary {
                    return true;
                }
                plan.lexical_control.as_deref().is_some_and(|control| {
                    control.resumes.iter().any(|resume| {
                        plan.lexical_block_in_context(Some(resume.entry), prior.block)
                            && plan.lexical_block_in_context(Some(resume.entry), next.block)
                    })
                })
            })
        });
        if !overlaps {
            previous.extend(rows);
        }
        Ok(overlaps)
    }
    pub(super) fn select<'a>(
        &'a self,
        context: Option<(&'a CleanupPlan, BlockId)>,
    ) -> Selection<'a, K> {
        Selection {
            index: self,
            context,
        }
    }
}
#[derive(Clone, Copy)]
pub(in crate::codegen) struct Selection<'a, K> {
    index: &'a Index<K>,
    context: Option<(&'a CleanupPlan, BlockId)>,
}
impl<'a, K: Ord> Selection<'a, K> {
    pub(super) fn get(&self, key: &K) -> Option<Slice<'a>> {
        self.index.0.get(key).and_then(|rows| {
            let slice = Slice {
                rows,
                context: self.context,
            };
            slice.iter().next().map(|_| slice)
        })
    }
}
pub(super) struct Slice<'a> {
    rows: &'a [Tagged],
    context: Option<(&'a CleanupPlan, BlockId)>,
}
impl<'a> Slice<'a> {
    pub(super) fn iter(&self) -> Iter<'a> {
        Iter {
            rows: self.rows.iter(),
            context: self.context,
        }
    }
}
impl<'a> IntoIterator for Slice<'a> {
    type Item = &'a CleanupTransition;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}
pub(super) struct Iter<'a> {
    rows: std::slice::Iter<'a, Tagged>,
    context: Option<(&'a CleanupPlan, BlockId)>,
}
impl Iter<'_> {
    fn includes(&self, row: &Tagged) -> bool {
        match self.context {
            None => row.ordinary,
            Some((plan, entry)) => plan.lexical_block_in_context(Some(entry), row.block),
        }
    }
}
impl<'a> Iterator for Iter<'a> {
    type Item = &'a CleanupTransition;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(row) = self.rows.next() {
            if self.includes(row) {
                return Some(&row.transition);
            }
        }
        None
    }
}
impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some(row) = self.rows.next_back() {
            if self.includes(row) {
                return Some(&row.transition);
            }
        }
        None
    }
}
