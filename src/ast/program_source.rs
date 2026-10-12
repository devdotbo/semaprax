//! Internal compiler input. Borrowed declarations retain authored order and
//! identity; generated overlays are not verification or resolution authority.
use super::{
    AgentDeclaration, ExprKind, Function, InterfaceDeclaration, ModuleUse, Program,
    ProtocolDeclaration, ProtocolImplementation, SessionProtocolDeclaration, Type, TypeDeclaration,
};

/// Two contiguous declaration collections; no flattened reference directory.
/// Construction checks their combined length before an iterator is lent.
#[derive(Clone, Copy)]
pub(crate) struct DeclSlices<'a, T> {
    authored: &'a [T],
    generated: &'a [T],
    length: usize,
}
impl<'a, T> DeclSlices<'a, T> {
    pub(crate) fn one(authored: &'a [T]) -> Self {
        Self {
            authored,
            generated: &[],
            length: authored.len(),
        }
    }
    fn append(authored: &'a [T], generated: &'a [T]) -> Option<Self> {
        Some(Self {
            authored,
            generated,
            length: authored.len().checked_add(generated.len())?,
        })
    }
    pub(crate) fn len(&self) -> usize {
        self.length
    }
    pub(crate) fn first(&self) -> Option<&'a T> {
        self.get(0)
    }
    pub(crate) fn get(&self, index: usize) -> Option<&'a T> {
        if index < self.authored.len() {
            self.authored.get(index)
        } else {
            self.generated.get(index.checked_sub(self.authored.len())?)
        }
    }
    pub(crate) fn suffix(&self, start: usize) -> Option<Self> {
        if start > self.length {
            return None;
        }
        if start < self.authored.len() {
            Self::append(&self.authored[start..], self.generated)
        } else {
            Some(Self::one(&self.generated[start - self.authored.len()..]))
        }
    }
    pub(crate) fn iter(&self) -> DeclIter<'a, T> {
        DeclIter {
            authored: self.authored.iter(),
            generated: self.generated.iter(),
            remaining: self.length,
        }
    }
}
impl<T> std::ops::Index<usize> for DeclSlices<'_, T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("declaration index in bounds")
    }
}
pub(crate) struct DeclIter<'a, T> {
    authored: std::slice::Iter<'a, T>,
    generated: std::slice::Iter<'a, T>,
    remaining: usize,
}
impl<'a, T> Iterator for DeclIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.authored.next().or_else(|| self.generated.next());
        if next.is_some() {
            self.remaining -= 1;
        }
        next
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<T> DoubleEndedIterator for DeclIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let next = self
            .generated
            .next_back()
            .or_else(|| self.authored.next_back());
        if next.is_some() {
            self.remaining -= 1;
        }
        next
    }
}
impl<T> ExactSizeIterator for DeclIter<'_, T> {}
impl<T> std::iter::FusedIterator for DeclIter<'_, T> {}
impl<'a, T> IntoIterator for DeclSlices<'a, T> {
    type Item = &'a T;
    type IntoIter = DeclIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A concrete address distinguishes different presentations of the same AST.
/// The byte prevents distinct empty overlays from sharing a ZST identity.
pub(crate) struct OverlayIdentity {
    _nonzero_size: u8,
}
#[derive(Clone, Copy)]
pub(crate) enum Subject<'a> {
    Owned(&'a Program),
    Projected(&'a OverlayIdentity),
}
impl Subject<'_> {
    pub(crate) fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Owned(a), Self::Owned(b)) => std::ptr::eq(a, b),
            (Self::Projected(a), Self::Projected(b)) => std::ptr::eq(a, b),
            _ => false,
        }
    }
}
mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Program {}
    impl Sealed for super::SyntheticProgramView<'_> {}
}
/// Only the owned AST and compiler-owned presentation implement this input.
/// A view carries no verdict; every source pass and resolver still runs.
pub(crate) trait ProgramSource: sealed::Sealed {
    fn subject(&self) -> Subject<'_>;
    fn path(&self) -> &str;
    fn module(&self) -> &str;
    fn module_uses(&self) -> &[ModuleUse];
    fn permits(&self) -> &[String];
    fn types(&self) -> DeclSlices<'_, TypeDeclaration>;
    fn interfaces(&self) -> &[InterfaceDeclaration];
    fn protocols(&self) -> &[ProtocolDeclaration];
    fn implementations(&self) -> &[ProtocolImplementation];
    fn session_protocols(&self) -> &[SessionProtocolDeclaration];
    fn agents(&self) -> &[AgentDeclaration];
    fn functions(&self) -> DeclSlices<'_, Function>;
}
impl ProgramSource for Program {
    fn subject(&self) -> Subject<'_> {
        Subject::Owned(self)
    }
    fn path(&self) -> &str {
        &self.path
    }
    fn module(&self) -> &str {
        &self.module
    }
    fn module_uses(&self) -> &[ModuleUse] {
        &self.module_uses
    }
    fn permits(&self) -> &[String] {
        &self.permits
    }
    fn types(&self) -> DeclSlices<'_, TypeDeclaration> {
        DeclSlices::one(&self.types)
    }
    fn interfaces(&self) -> &[InterfaceDeclaration] {
        &self.interfaces
    }
    fn protocols(&self) -> &[ProtocolDeclaration] {
        &self.protocols
    }
    fn implementations(&self) -> &[ProtocolImplementation] {
        &self.implementations
    }
    fn session_protocols(&self) -> &[SessionProtocolDeclaration] {
        &self.session_protocols
    }
    fn agents(&self) -> &[AgentDeclaration] {
        &self.agents
    }
    fn functions(&self) -> DeclSlices<'_, Function> {
        DeclSlices::one(&self.functions)
    }
}

#[path = "program_source/generated_roots.rs"]
pub(crate) mod generated_roots;
use generated_roots::GeneratedRoots;

/// A producer-paid generated overlay after the exact authored slices.
/// Authored bodies and declaration backing are never cloned to lend this input.
pub(crate) struct SyntheticProgramView<'a> {
    authored: &'a Program,
    generated: GeneratedRoots,
    identity: OverlayIdentity,
}
impl<'a> SyntheticProgramView<'a> {
    pub(crate) fn into_overlay_owner(self) -> (&'a Program, GeneratedRoots) {
        (self.authored, self.generated)
    }

    pub(crate) fn with_generated_owner(
        authored: &'a Program,
        generated: GeneratedRoots,
    ) -> Option<Self> {
        authored.types.len().checked_add(generated.types.len())?;
        authored
            .functions
            .len()
            .checked_add(generated.functions.len())?;
        Some(Self {
            authored,
            generated,
            identity: OverlayIdentity { _nonzero_size: 0 },
        })
    }

    pub(crate) fn new(authored: &'a Program, generated_functions: Vec<Function>) -> Option<Self> {
        // Keep the leaf-main constructor closed. Imported prototypes use the
        // separately classified and paid imported-overlay producer.
        if generated_functions.iter().any(|function| {
            function.name != "main"
                || !function.params.is_empty()
                || !function.type_parameters.is_empty()
                || !function.requires.is_empty()
                || !function.ensures.is_empty()
                || function.return_type != Type::I64
                || function.yields.is_some()
                || function.follows.is_some()
                || !function.effects.is_empty()
                || !matches!(function.body.kind, ExprKind::Int(0))
        }) {
            return None;
        }
        authored
            .functions
            .len()
            .checked_add(generated_functions.len())?;
        Some(Self {
            authored,
            generated: GeneratedRoots::plain(Vec::new(), generated_functions),
            identity: OverlayIdentity { _nonzero_size: 0 },
        })
    }
}
impl ProgramSource for SyntheticProgramView<'_> {
    fn subject(&self) -> Subject<'_> {
        Subject::Projected(&self.identity)
    }
    fn path(&self) -> &str {
        &self.authored.path
    }
    fn module(&self) -> &str {
        &self.authored.module
    }
    // Project validation authenticates these sidecars before presentation.
    // This is exactly the old clone-and-clear projection, not a permission.
    fn module_uses(&self) -> &[ModuleUse] {
        &[]
    }
    fn permits(&self) -> &[String] {
        &self.authored.permits
    }
    fn types(&self) -> DeclSlices<'_, TypeDeclaration> {
        DeclSlices::append(&self.authored.types, &self.generated.types)
            .expect("constructor checked declaration count")
    }
    fn interfaces(&self) -> &[InterfaceDeclaration] {
        &self.authored.interfaces
    }
    fn protocols(&self) -> &[ProtocolDeclaration] {
        &self.authored.protocols
    }
    fn implementations(&self) -> &[ProtocolImplementation] {
        &[]
    }
    fn session_protocols(&self) -> &[SessionProtocolDeclaration] {
        &self.authored.session_protocols
    }
    fn agents(&self) -> &[AgentDeclaration] {
        &self.authored.agents
    }
    fn functions(&self) -> DeclSlices<'_, Function> {
        DeclSlices::append(&self.authored.functions, &self.generated.functions)
            .expect("constructor checked declaration count")
    }
}
