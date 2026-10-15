//! Payloads borrow the exact held HIR; map keys and traversal remain independent.
use super::*;
#[derive(Clone, Copy, Debug)]
pub(super) enum Arguments<'hir> {
    Slice(&'hir [ResolvedExpr]),
    Range([&'hir ResolvedExpr; 3]),
}
impl<'hir> Arguments<'hir> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Slice(args) => args.len(),
            Self::Range(_) => 3,
        }
    }
    pub(super) fn get(&self, index: usize) -> Option<&'hir ExpressionId> {
        match self {
            Self::Slice(args) => args.get(index).map(|arg| &arg.id),
            Self::Range(args) => args.get(index).map(|arg| &arg.id),
        }
    }
    pub(super) fn iter(&self) -> ArgumentIds<'hir> {
        ArgumentIds {
            arguments: *self,
            index: 0,
        }
    }
}
pub(super) struct ArgumentIds<'hir> {
    arguments: Arguments<'hir>,
    index: usize,
}
impl<'hir> Iterator for ArgumentIds<'hir> {
    type Item = &'hir ExpressionId;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.arguments.get(self.index)?;
        self.index += 1;
        Some(next)
    }
}
impl<'hir> IntoIterator for &Arguments<'hir> {
    type Item = &'hir ExpressionId;
    type IntoIter = ArgumentIds<'hir>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl PartialEq for Arguments<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Types<'hir> {
    Slice(&'hir [ResolvedType]),
    One(&'hir ResolvedType),
}
impl Types<'_> {
    pub(super) fn as_slice(&self) -> &[ResolvedType] {
        match self {
            Self::Slice(types) => types,
            Self::One(ty) => std::slice::from_ref(*ty),
        }
    }
}
impl std::ops::Deref for Types<'_> {
    type Target = [ResolvedType];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
impl PartialEq for Types<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
