//! Closed read-only suffix; ordinary HIR validation independently checks live places and loans.
use super::*;
const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;
struct Census<'a> {
    frames: [Option<crate::hir::lexical_exit::ChildCursor<'a>>; DEPTH],
    len: usize,
    work: usize,
}
impl<'a> Census<'a> {
    fn expression(&mut self, program: &ResolvedProgram, root: &'a E, element: &T) -> Option<()> {
        self.frames[0] = Some(crate::hir::lexical_exit::ChildCursor::new(root));
        self.len = 1;
        while self.len != 0 {
            self.work = self.work.checked_sub(1)?;
            let frame = self.frames[self.len - 1].as_mut()?;
            if !frame.is_started() {
                let e = frame.expression();
                match &e.kind {
                    K::Int(_)
                    | K::Usize(_)
                    | K::Uint8(_)
                    | K::Bool(_)
                    | K::Unary { .. }
                    | K::Binary { .. }
                    | K::If { .. } => {}
                    K::Project { .. }
                        if crate::hir::is_scalar_resolved_type(&e.ty)
                            && e.ownership == O::Value => {}
                    K::Match {
                        mode: crate::hir::ResolvedMatchMode::Value,
                        scrutinee,
                        arms,
                    } if matches!(&scrutinee.ty, T::Nominal { declaration, arguments } if declaration.as_str()==crate::prelude::OPTION_ID && arguments.as_slice()==[T::U8])
                        && arms.len() == 2
                        && arms.iter().all(|arm| arm.guard.is_none()) => {}
                    K::Place(_)
                        if crate::hir::is_scalar_resolved_type(&e.ty)
                            && e.ownership == O::Value => {}
                    K::Place(place)
                        if place.projections.is_empty()
                            && (matches!(e.ty, T::Str | T::SliceU8)
                                && e.ownership == O::Borrow
                                || matches!(&e.ty,T::Nominal{declaration,..} if declaration.as_str()==crate::prelude::VEC_ID)
                                    && matches!(e.ownership, O::Own | O::Borrow)) => {}
                    K::BorrowPlace { operation, .. }
                        if matches!(
                            operation.as_str(),
                            crate::byte_ops::STR_AS_BYTES_ID | crate::byte_ops::STRING_AS_STR_ID
                        ) => {}
                    K::VecFieldRead { .. }
                        if (matches!(e.ty, T::Str | T::SliceU8) && e.ownership == O::Borrow)
                            || (crate::hir::is_scalar_resolved_type(&e.ty)
                                && e.ownership == O::Value) => {}
                    K::Block { statements, .. } => {
                        // A lowered traversal is accepted only after its complete independent freeze replay.
                        let traversal = crate::loan_plan::traversal::inspect_borrowed(
                            program,
                            e,
                            element,
                            &mut self.work,
                        )?
                        .is_some();
                        for statement in statements {
                            self.work = self.work.checked_sub(1)?;
                            match statement {
                                S::Let {
                                    binding, mutable, ..
                                } if !*mutable
                                    || (crate::hir::is_scalar_resolved_type(&binding.ty)
                                        && binding.ownership == O::Value) => {}
                                S::Assign {
                                    binding,
                                    field: None,
                                    ..
                                } if crate::hir::is_scalar_resolved_type(&binding.ty)
                                    && binding.ownership == O::Value => {}
                                S::While { .. } | S::LexicalWhile { .. } if traversal => {}
                                _ => return None,
                            }
                        }
                    }
                    K::Call {
                        callee,
                        instance: None,
                        ..
                    } if matches!(
                        callee.as_str(),
                        crate::vec_ops::LEN_ID
                            | crate::vec_ops::GET_ID
                            | crate::vec_ops::SORT_OWNED_ID
                            | crate::byte_ops::LEN_ID
                            | crate::byte_ops::GET_ID
                            | crate::string_ops::I64_FROM_USIZE_ID
                            | crate::str_ops::LEN_BYTES_ID
                    ) => {}
                    _ => return None,
                }
            }
            if let Some(child) = frame.next() {
                *self.frames.get_mut(self.len)? =
                    Some(crate::hir::lexical_exit::ChildCursor::new(child));
                self.len += 1;
            } else {
                self.frames[self.len - 1] = None;
                self.len -= 1;
            }
        }
        Some(())
    }
}
pub(super) fn validate<'a>(
    program: &ResolvedProgram,
    suffix: &'a [S],
    tail: &'a E,
    element: &T,
) -> Option<()> {
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Census<'_>>()) {
        return None;
    }
    let mut census = Census {
        frames: [None; DEPTH],
        len: 0,
        work: crate::cache_codec::MAX_NODES,
    };
    for statement in suffix {
        match statement {
            S::Let {
                binding,
                mutable,
                value,
                ..
            } if !*mutable
                || (crate::hir::is_scalar_resolved_type(&binding.ty)
                    && binding.ownership == O::Value) =>
            {
                census.expression(program, value, element)?
            }
            S::Assign {
                binding,
                field: None,
                value,
                ..
            } if crate::hir::is_scalar_resolved_type(&binding.ty)
                && binding.ownership == O::Value =>
            {
                census.expression(program, value, element)?
            }
            _ => return None,
        }
    }
    census.expression(program, tail, element)
}

#[cfg(test)]
pub(super) fn scratch_bytes() -> usize {
    std::mem::size_of::<Census<'_>>()
}
