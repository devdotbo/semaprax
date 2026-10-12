//! Generated recursive roots retain the ordinary Program teardown owner.
//! The workspace producer pays that owner's worklist backing before copying
//! roots. Plain scalar/default overlays retain their allocation-free Drop.
use crate::ast::{Function, Program, TypeDeclaration};

pub(crate) struct GeneratedRoots {
    pub(crate) types: Vec<TypeDeclaration>,
    pub(crate) functions: Vec<Function>,
    iterative: bool,
}
impl GeneratedRoots {
    pub(crate) fn plain(types: Vec<TypeDeclaration>, functions: Vec<Function>) -> Self {
        Self { types, functions, iterative: false }
    }
    pub(crate) fn iterative(types: Vec<TypeDeclaration>, functions: Vec<Function>) -> Self {
        Self { types, functions, iterative: true }
    }
}
impl Drop for GeneratedRoots {
    fn drop(&mut self) {
        if !self.iterative { return; }
        // Empty metadata and sidecars acquire no backing. The exact existing
        // Program destructor takes every recursive root before drop glue runs,
        // on success and on partial construction/refusal alike.
        drop(Program {
            path: String::new(), module: String::new(), module_uses: Vec::new(),
            permits: Vec::new(), types: std::mem::take(&mut self.types),
            interfaces: Vec::new(), protocols: Vec::new(), implementations: Vec::new(),
            session_protocols: Vec::new(), agents: Vec::new(),
            functions: std::mem::take(&mut self.functions),
        });
    }
}
