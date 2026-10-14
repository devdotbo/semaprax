//! Source-shaped disposal frontier acquired on the same actual caller owner.
//! This scratch is storage only; neither source admission nor a type proof.
use super::*;
use std::mem::size_of;
#[derive(Default)]
pub(in crate::hir) struct Workspace {
    pub(super) scratch: Option<Vec<ResolvedType>>,
    pub(super) building: bool,
}
// Published canonical indexes own no legacy recursive field Vec. Their clones
// share payload owners, each of which already owns its own disposal scratch.
impl Clone for Workspace {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl PartialEq for Workspace {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Workspace {}
impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("private disposal storage")
    }
}
fn frontier<E>(root: &Type, reserve: &mut impl FnMut(usize) -> Result<(), E>) -> Result<usize, E> {
    fn push<'a, E>(
        stack: &mut Vec<&'a Type>,
        ty: &'a Type,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        if stack.len() == stack.capacity() {
            let cap = stack.capacity().saturating_mul(2).max(4);
            reserve(
                size_of::<Vec<&Type>>().saturating_add(cap.saturating_mul(size_of::<&Type>())),
            )?;
            let mut next = Vec::with_capacity(cap);
            assert_eq!(next.capacity(), cap);
            next.extend(stack.drain(..));
            *stack = next;
        }
        stack.push(ty);
        Ok(())
    }
    if !matches!(root,Type::Named{arguments,..} if !arguments.is_empty())
        && !matches!(root, Type::Function { .. })
    {
        return Ok(0);
    }
    let mut stack = Vec::new();
    push(&mut stack, root, reserve)?;
    let mut maximum = 0;
    while let Some(ty) = stack.pop() {
        match ty {
            Type::Named { arguments, .. } => {
                for child in arguments {
                    push(&mut stack, child, reserve)?;
                }
            }
            Type::Function { parameters, result } => {
                for child in parameters {
                    push(&mut stack, child, reserve)?;
                }
                push(&mut stack, result, reserve)?;
            }
            _ => {}
        }
        maximum = maximum.max(stack.len());
    }
    Ok(maximum)
}
impl Workspace {
    pub(super) fn acquire<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        // Source types resolve shape-for-shape (parameters become scalar
        // metadata leaves); inherited fields reuse those same type shapes.
        // Traverse every declared field, not a nominal/prelude name filter.
        let mut capacity = 0;
        for declaration in program.types().iter().chain(prelude) {
            match &declaration.kind {
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        capacity = capacity.max(frontier(&field.ty, reserve)?);
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        for field in &case.fields {
                            capacity = capacity.max(frontier(&field.ty, reserve)?);
                        }
                    }
                }
                TypeDeclarationKind::Resource { .. } => {}
            }
        }
        reserve(
            size_of::<Self>()
                .saturating_add(size_of::<Result<Self, E>>())
                .saturating_add(capacity.saturating_mul(size_of::<ResolvedType>())),
        )?;
        let scratch = Vec::with_capacity(capacity);
        assert_eq!(scratch.capacity(), capacity);
        Ok(Self {
            scratch: Some(scratch),
            building: true,
        })
    }
}
