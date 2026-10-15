//! Source-shaped disposal frontier acquired on the same actual caller owner.
//! This scratch is storage only; neither source admission nor a type proof.
use super::*;
use std::{
    mem::size_of,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};
type Scratch = Arc<Mutex<Vec<ResolvedType>>>;
#[derive(Default)]
pub(in crate::hir) struct Workspace {
    pub(super) scratch: Option<Scratch>,
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
            next.append(stack);
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
/// Construction-only Vec guard. The original Vec allocation and order are
/// unchanged; all local recursive values share the one prepaid rollback frontier.
pub(super) struct PendingVec<T: nominal_payload::NominalElement> {
    values: Vec<T>,
    scratch: Option<Scratch>,
}
impl<T: nominal_payload::NominalElement> PendingVec<T> {
    pub(super) fn into_vec(mut self) -> Vec<T> {
        std::mem::take(&mut self.values)
    }
}
impl<T: nominal_payload::NominalElement> Deref for PendingVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.values
    }
}
impl<T: nominal_payload::NominalElement> DerefMut for PendingVec<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.values
    }
}
impl<T: nominal_payload::NominalElement> Drop for PendingVec<T> {
    fn drop(&mut self) {
        if let Some(scratch) = &self.scratch {
            let mut scratch = scratch.lock().expect("construction disposal lock");
            for value in &mut self.values {
                value.drain_types(&mut |ty| nominal_payload::dispose(ty, &mut scratch));
            }
        }
    }
}
pub(super) struct PendingType {
    value: Option<ResolvedType>,
    scratch: Option<Scratch>,
}
impl PendingType {
    pub(super) fn into_type(mut self) -> ResolvedType {
        self.value.take().expect("present construction type")
    }
}
impl Drop for PendingType {
    fn drop(&mut self) {
        if let (Some(ty), Some(scratch)) = (self.value.take(), &self.scratch) {
            nominal_payload::dispose(ty, &mut scratch.lock().expect("construction disposal lock"));
        }
    }
}
pub(super) struct PendingTypes {
    values: Vec<ResolvedType>,
    scratch: Option<Scratch>,
}
impl Deref for PendingTypes {
    type Target = Vec<ResolvedType>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
impl DerefMut for PendingTypes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}
impl Drop for PendingTypes {
    fn drop(&mut self) {
        if let Some(s) = &self.scratch {
            let mut s = s.lock().expect("construction disposal lock");
            for ty in self.values.drain(..) {
                nominal_payload::dispose(ty, &mut s);
            }
        }
    }
}
impl Workspace {
    pub(super) fn types(&self) -> PendingTypes {
        PendingTypes {
            values: Vec::new(),
            scratch: self.scratch.clone(),
        }
    }
    pub(super) fn fields<T: nominal_payload::NominalElement>(
        &self,
        capacity: usize,
    ) -> PendingVec<T> {
        PendingVec {
            values: Vec::with_capacity(capacity),
            scratch: self.scratch.clone(),
        }
    }
    pub(super) fn resolved_type(&self, value: ResolvedType) -> PendingType {
        PendingType {
            value: Some(value),
            scratch: self.scratch.clone(),
        }
    }
    pub(super) fn finish(&mut self) {
        self.building = false;
        // Every published nonempty sequence now has its own immutable paid
        // owner. Never apply a source frontier to later host-mutated values.
        self.scratch = None;
    }
    pub(super) fn acquire<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        // Replacing Vec map values enlarges actual BTree nodes. Acquire a
        // complete current node upper bound per possible insertion in each
        // changed map, in addition to the unchanged caller prebound. There is
        // no credit for the old map or for omitted payload clones.
        fn node<V>() -> usize {
            crate::cleanup_plan::replay_btree_node_upper::<DeclarationId, V>()
                .expect("fixed nominal map layout")
        }
        for declaration in program.types().iter().chain(prelude) {
            reserve(node::<NominalPayload<ResolvedTypeParameterDeclaration>>())?;
            match &declaration.kind {
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => {
                    reserve(node::<NominalPayload<ResolvedFieldDeclaration>>())?;
                }
                TypeDeclarationKind::Variant { cases } => {
                    reserve(node::<NominalPayload<ResolvedVariantCaseDeclaration>>())?;
                    for _ in cases {
                        reserve(node::<CaseFields>())?;
                    }
                }
                TypeDeclarationKind::Resource { .. } => {}
            }
        }
        for _ in program.functions() {
            reserve(node::<NominalPayload<ResolvedTypeParameterDeclaration>>())?;
        }
        for declaration in program.types() {
            if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
                // The complete old producer rebuilds this class field entry.
                reserve(node::<NominalPayload<ResolvedFieldDeclaration>>())?;
                for _ in methods {
                    reserve(node::<NominalPayload<ResolvedTypeParameterDeclaration>>())?;
                }
            }
        }
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
        #[repr(C)]
        struct ArcCarrier<T> {
            strong: usize,
            weak: usize,
            value: T,
        }
        // At most one outer cases guard, one fields guard and one resolved
        // field type coexist. Price both initialization and receiving slots.
        let guards = 2 * size_of::<PendingVec<ResolvedVariantCaseDeclaration>>()
            + 2 * size_of::<PendingVec<ResolvedFieldDeclaration>>()
            + 2 * size_of::<PendingType>()
            + 2 * size_of::<PendingTypes>();
        // The pthread std Mutex stores its native object in a lazy Box.
        // Fund that exact native Layout before its first lock, then initialize
        // it here so rollback never performs that allocation after refusal.
        #[cfg(all(
            unix,
            not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "freebsd",
                target_os = "openbsd",
                target_os = "dragonfly",
                target_os = "fuchsia"
            ))
        ))]
        let mutex_backing = size_of::<libc::pthread_mutex_t>();
        #[cfg(not(all(
            unix,
            not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "freebsd",
                target_os = "openbsd",
                target_os = "dragonfly",
                target_os = "fuchsia"
            ))
        )))]
        let mutex_backing = 0;
        reserve(
            size_of::<ArcCarrier<Mutex<Vec<ResolvedType>>>>()
                .saturating_add(mutex_backing)
                .saturating_add(guards)
                .saturating_add(size_of::<Self>())
                .saturating_add(size_of::<Result<Self, E>>())
                .saturating_add(capacity.saturating_mul(size_of::<ResolvedType>())),
        )?;
        let scratch = Vec::with_capacity(capacity);
        assert_eq!(scratch.capacity(), capacity);
        let scratch = Arc::new(Mutex::new(scratch));
        drop(
            scratch
                .lock()
                .expect("initialize prepaid construction disposal lock"),
        );
        Ok(Self {
            scratch: Some(scratch),
            building: true,
        })
    }
}
