//! Unit-test projection of compiler-checked primitive module bodies.
//! Standalone resolution includes builtin nominal declarations, whereas the
//! authenticated workspace primitive context exposes only the authored module.
//! This adapter exercises replay internals; it grants no source authentication
//! and deliberately supplies neither full-program nor renewal authority.
use crate::cleanup::ReplayContext;
use crate::hir::*;

pub(super) struct Context<'a>(&'a ResolvedProgram);
impl<'a> Context<'a> {
    pub(super) fn new(program: &'a ResolvedProgram) -> Self {
        assert!(program.function_templates.is_empty());
        assert!(program.function_instances.is_empty());
        assert!(program.permits.is_empty());
        assert!(program.agents.is_empty());
        for function in &program.functions {
            assert!(function.effects.is_empty());
            assert!(function.yields.is_none());
            assert!(primitive(&function.return_type));
            assert!(function.params.iter().all(|p| primitive(&p.ty)));
            let mut pending = vec![&function.body];
            pending.extend(&function.requires);
            pending.extend(&function.ensures);
            while let Some(expression) = pending.pop() {
                assert!(primitive(&expression.ty) || expression.ty == ResolvedType::Never);
                pending.extend(crate::hir::lexical_exit::ChildCursor::new(expression));
            }
        }
        Self(program)
    }
}
fn primitive(ty: &ResolvedType) -> bool {
    crate::hir::is_scalar_resolved_type(ty)
        || matches!(
            ty,
            ResolvedType::Bytes
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::ArrayU8(_)
        )
}
impl ReplayContext for Context<'_> {
    fn declarations(&self) -> &DeclarationIndex {
        &self.0.declarations
    }
    fn types(&self) -> crate::hir::items::View<'_, ResolvedTypeDeclaration> {
        crate::hir::items::View::slice(&[])
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &[]
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        crate::hir::items::View::slice(&self.0.functions)
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &[]
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &[]
    }
}
