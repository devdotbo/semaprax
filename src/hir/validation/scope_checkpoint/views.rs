//! Allocation-free ordered reads over both private checkpoints and old maps.
use super::*;
pub(in crate::hir::validation) struct TreeIter<'a> {
    stack: [Option<&'a tree::Node>; 2 * usize::BITS as usize],
    len: usize,
}
impl<'a> TreeIter<'a> {
    fn descend(&mut self, mut node: Option<&'a tree::Node>) {
        while let Some(current) = node {
            assert!(self.len < self.stack.len(), "private AVL scope depth");
            self.stack[self.len] = Some(current);
            self.len += 1;
            node = current.left.as_deref();
        }
    }
    pub(super) fn new(root: Option<&'a tree::Node>) -> Self {
        let mut iter = Self {
            stack: [None; 2 * usize::BITS as usize],
            len: 0,
        };
        iter.descend(root);
        iter
    }
}
impl<'a> Iterator for TreeIter<'a> {
    type Item = (&'a ValueId, &'a ValidationBinding);
    fn next(&mut self) -> Option<Self::Item> {
        self.len = self.len.checked_sub(1)?;
        let current = self.stack[self.len].take().unwrap();
        self.descend(current.right.as_deref());
        Some((&current.key, &current.value))
    }
}
pub(in crate::hir::validation) enum Entries<'a> {
    Old(std::collections::btree_map::Iter<'a, ValueId, ValidationBinding>),
    Checkpoint(TreeIter<'a>),
}
impl<'a> Iterator for Entries<'a> {
    type Item = (&'a ValueId, &'a ValidationBinding);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Old(iter) => iter.next(),
            Self::Checkpoint(iter) => iter.next(),
        }
    }
}
pub(in crate::hir::validation) struct Keys<'a>(pub(in crate::hir::validation) Entries<'a>);
impl<'a> Iterator for Keys<'a> {
    type Item = &'a ValueId;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(id, _)| id)
    }
}
pub(in crate::hir::validation) struct Values<'a>(pub(in crate::hir::validation) Entries<'a>);
impl<'a> Iterator for Values<'a> {
    type Item = &'a ValidationBinding;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }
}
pub(in crate::hir::validation) trait Read {
    fn get(&self, id: &ValueId) -> Option<&ValidationBinding>;
    fn len(&self) -> usize;
    fn iter(&self) -> Entries<'_>;
    fn contains_key(&self, id: &ValueId) -> bool {
        self.get(id).is_some()
    }
    fn keys(&self) -> Keys<'_> {
        Keys(self.iter())
    }
    fn values(&self) -> Values<'_> {
        Values(self.iter())
    }
}
impl Read for BTreeMap<ValueId, ValidationBinding> {
    fn get(&self, id: &ValueId) -> Option<&ValidationBinding> {
        BTreeMap::get(self, id)
    }
    fn len(&self) -> usize {
        BTreeMap::len(self)
    }
    fn iter(&self) -> Entries<'_> {
        Entries::Old(BTreeMap::iter(self))
    }
}
impl Read for Scope {
    fn get(&self, id: &ValueId) -> Option<&ValidationBinding> {
        tree::get(self.root.as_deref(), id)
    }
    fn len(&self) -> usize {
        self.root.as_ref().map_or(0, |node| node.count)
    }
    fn iter(&self) -> Entries<'_> {
        Entries::Checkpoint(TreeIter::new(self.root.as_deref()))
    }
}

pub(in crate::hir::validation) enum Target<'a> {
    Map(&'a mut BTreeMap<ValueId, ValidationBinding>),
    Checkpoint(&'a mut Scope),
}
pub(in crate::hir::validation) trait Write: Read {
    fn publication_target(&mut self) -> Target<'_>;
    fn get_mut(&mut self, id: &ValueId) -> Result<Option<&mut ValidationBinding>, Diagnostic>;
    fn insert(&mut self, id: ValueId, value: ValidationBinding) -> Result<(), Diagnostic>;
    fn old_mut(&mut self) -> Option<&mut BTreeMap<ValueId, ValidationBinding>> {
        None
    }
    fn take_owned(&mut self) -> Owned;
    fn replace_owned(&mut self, value: Owned);
}
impl Write for BTreeMap<ValueId, ValidationBinding> {
    fn publication_target(&mut self) -> Target<'_> {
        Target::Map(self)
    }
    fn get_mut(&mut self, id: &ValueId) -> Result<Option<&mut ValidationBinding>, Diagnostic> {
        Ok(BTreeMap::get_mut(self, id))
    }
    fn insert(&mut self, id: ValueId, value: ValidationBinding) -> Result<(), Diagnostic> {
        drop(BTreeMap::insert(self, id, value));
        Ok(())
    }
    fn old_mut(&mut self) -> Option<&mut BTreeMap<ValueId, ValidationBinding>> {
        Some(self)
    }
    fn take_owned(&mut self) -> Owned {
        Owned::Old(std::mem::take(self))
    }
    fn replace_owned(&mut self, value: Owned) {
        let Owned::Old(value) = value else {
            panic!("legacy scope backend changed")
        };
        *self = value;
    }
}
impl Write for Scope {
    fn publication_target(&mut self) -> Target<'_> {
        Target::Checkpoint(self)
    }
    fn get_mut(&mut self, id: &ValueId) -> Result<Option<&mut ValidationBinding>, Diagnostic> {
        Scope::get_mut(self, id)
    }
    fn insert(&mut self, id: ValueId, value: ValidationBinding) -> Result<(), Diagnostic> {
        Scope::insert(self, id, value)
    }
    fn take_owned(&mut self) -> Owned {
        Owned::Checkpoint(std::mem::take(self))
    }
    fn replace_owned(&mut self, value: Owned) {
        let Owned::Checkpoint(value) = value else {
            panic!("checkpoint scope backend changed")
        };
        *self = value;
    }
}
#[derive(Clone)]
pub(in crate::hir::validation) enum Owned {
    Old(BTreeMap<ValueId, ValidationBinding>),
    Checkpoint(Scope),
}
impl Read for Owned {
    fn get(&self, id: &ValueId) -> Option<&ValidationBinding> {
        match self {
            Self::Old(value) => value.get(id),
            Self::Checkpoint(value) => value.get(id),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Old(value) => value.len(),
            Self::Checkpoint(value) => value.len(),
        }
    }
    fn iter(&self) -> Entries<'_> {
        match self {
            Self::Old(value) => Entries::Old(value.iter()),
            Self::Checkpoint(value) => value.iter(),
        }
    }
}
impl Write for Owned {
    fn publication_target(&mut self) -> Target<'_> {
        match self {
            Self::Old(value) => Target::Map(value),
            Self::Checkpoint(value) => Target::Checkpoint(value),
        }
    }
    fn get_mut(&mut self, id: &ValueId) -> Result<Option<&mut ValidationBinding>, Diagnostic> {
        match self {
            Self::Old(value) => Ok(value.get_mut(id)),
            Self::Checkpoint(value) => value.get_mut(id),
        }
    }
    fn insert(&mut self, id: ValueId, binding: ValidationBinding) -> Result<(), Diagnostic> {
        match self {
            Self::Old(value) => {
                drop(value.insert(id, binding));
                Ok(())
            }
            Self::Checkpoint(value) => value.insert(id, binding),
        }
    }
    fn old_mut(&mut self) -> Option<&mut BTreeMap<ValueId, ValidationBinding>> {
        match self {
            Self::Old(value) => Some(value),
            Self::Checkpoint(_) => None,
        }
    }
    fn take_owned(&mut self) -> Owned {
        match self {
            Self::Old(value) => Owned::Old(std::mem::take(value)),
            Self::Checkpoint(value) => Owned::Checkpoint(std::mem::take(value)),
        }
    }
    fn replace_owned(&mut self, value: Owned) {
        *self = value;
    }
}
impl PartialEq for Owned {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl Eq for Owned {}
impl std::fmt::Debug for Owned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
#[derive(Clone)]
pub(in crate::hir::validation) enum IdSnapshot {
    Old(Vec<ValueId>),
    Checkpoint(Scope),
}
pub(in crate::hir::validation) enum IdIter<'a> {
    Old(std::slice::Iter<'a, ValueId>),
    Checkpoint(Keys<'a>),
}
impl<'a> Iterator for IdIter<'a> {
    type Item = &'a ValueId;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Old(value) => value.next(),
            Self::Checkpoint(value) => value.next(),
        }
    }
}
pub(in crate::hir::validation) trait Ids {
    fn ids(&self) -> IdIter<'_>;
}
impl Ids for Vec<ValueId> {
    fn ids(&self) -> IdIter<'_> {
        IdIter::Old(self.iter())
    }
}
impl Ids for [ValueId] {
    fn ids(&self) -> IdIter<'_> {
        IdIter::Old(self.iter())
    }
}
impl Ids for IdSnapshot {
    fn ids(&self) -> IdIter<'_> {
        match self {
            Self::Old(value) => IdIter::Old(value.iter()),
            Self::Checkpoint(value) => IdIter::Checkpoint(value.keys()),
        }
    }
}
impl Scope {
    pub(in crate::hir::validation) fn ids(&self) -> IdSnapshot {
        IdSnapshot::Checkpoint(self.clone())
    }
}
impl Owned {
    pub(in crate::hir::validation) fn ids(&self) -> IdSnapshot {
        match self {
            Self::Old(value) => IdSnapshot::Old(value.keys().cloned().collect()),
            Self::Checkpoint(value) => value.ids(),
        }
    }
}
