//! Receiving directories own their actual Vecs through every partial frontier.
use super::generated_roots::GeneratedRoots;
use crate::ast::{Function, TypeDeclaration};
use crate::bounded_output::key_directory::KeyDirectory;

pub(crate) struct ReceivingRoots<'a> {
    pub(crate) types: Option<KeyDirectory<'a, TypeDeclaration>>,
    pub(crate) functions: Option<KeyDirectory<'a, Function>>,
    iterative: bool,
}
impl<'a> ReceivingRoots<'a> {
    pub(crate) fn new(types: KeyDirectory<'a, TypeDeclaration>, iterative: bool) -> Self {
        Self {
            types: Some(types),
            functions: None,
            iterative,
        }
    }
    pub(crate) fn types(&self) -> &[TypeDeclaration] {
        self.types.as_deref().unwrap_or(&[])
    }
    pub(crate) fn functions(&self) -> &[Function] {
        self.functions.as_deref().unwrap_or(&[])
    }
    pub(crate) fn into_plain(mut self) -> GeneratedRoots {
        let types = self
            .types
            .take()
            .map_or_else(Vec::new, KeyDirectory::into_vec);
        let functions = self
            .functions
            .take()
            .map_or_else(Vec::new, KeyDirectory::into_vec);
        if self.iterative {
            GeneratedRoots::iterative(types, functions)
        } else {
            GeneratedRoots::plain(types, functions)
        }
    }
}
impl Drop for ReceivingRoots<'_> {
    fn drop(&mut self) {
        let types = self
            .types
            .take()
            .map_or_else(Vec::new, KeyDirectory::into_vec);
        let functions = self
            .functions
            .take()
            .map_or_else(Vec::new, KeyDirectory::into_vec);
        // Exactly the original recursive owner and prepaid worklist; no new
        // directory or additional allowance is acquired while dropping.
        if self.iterative {
            drop(GeneratedRoots::iterative(types, functions));
        } else {
            drop(GeneratedRoots::plain(types, functions));
        }
    }
}

pub(crate) enum GeneratedOwner<'a> {
    Plain(GeneratedRoots),
    Receiving(ReceivingRoots<'a>),
}
impl GeneratedOwner<'_> {
    pub(crate) fn types(&self) -> &[TypeDeclaration] {
        match self {
            Self::Plain(roots) => &roots.types,
            Self::Receiving(roots) => roots.types(),
        }
    }
    pub(crate) fn functions(&self) -> &[Function] {
        match self {
            Self::Plain(roots) => &roots.functions,
            Self::Receiving(roots) => roots.functions(),
        }
    }
    pub(crate) fn into_plain(self) -> GeneratedRoots {
        match self {
            Self::Plain(roots) => roots,
            Self::Receiving(roots) => roots.into_plain(),
        }
    }
}
