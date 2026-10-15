//! Compiler-only partial roots have one exact paid inline disposal frontier.
use super::*;
use std::ops::{Deref, DerefMut};
// Actual extension fields are T or Iter<T>/List<T>. Reject a changed schema
// before constructing a deeper tree; this is not a user-source depth quota.
fn dispose(ty: ResolvedType) {
    let mut pending: Option<ResolvedType> = Some(ty);
    while let Some(ty) = pending.take() {
        match ty {
            ResolvedType::Nominal { mut arguments, .. } => {
                assert!(
                    arguments.len() <= 1,
                    "fixed compiler extension disposal frontier"
                );
                pending = arguments.pop();
            }
            ResolvedType::Function { .. } => unreachable!("not an extension field schema"),
            _ => {}
        }
    }
}
pub(super) struct Type(pub(super) Option<ResolvedType>);
impl Type {
    pub(super) fn take(mut self) -> ResolvedType {
        self.0.take().expect("staged compiler type")
    }
}
impl Drop for Type {
    fn drop(&mut self) {
        if let Some(ty) = self.0.take() {
            dispose(ty);
        }
    }
}
pub(super) struct Fields(pub(super) Vec<ResolvedFieldDeclaration>);
impl Fields {
    pub(super) fn take(mut self) -> Vec<ResolvedFieldDeclaration> {
        std::mem::take(&mut self.0)
    }
}
impl Deref for Fields {
    type Target = Vec<ResolvedFieldDeclaration>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Fields {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for Fields {
    fn drop(&mut self) {
        for f in &mut self.0 {
            dispose(std::mem::replace(&mut f.ty, ResolvedType::Unit));
        }
    }
}
pub(super) struct Cases(pub(super) Vec<ResolvedVariantCaseDeclaration>);
impl Deref for Cases {
    type Target = Vec<ResolvedVariantCaseDeclaration>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Cases {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for Cases {
    fn drop(&mut self) {
        for c in &mut self.0 {
            for f in &mut c.fields {
                dispose(std::mem::replace(&mut f.ty, ResolvedType::Unit));
            }
        }
    }
}
