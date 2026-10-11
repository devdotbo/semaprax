//! Closed read-only use domain after construction. Ordinary HIR validation
//! still authenticates every live place, move and loan; this census only
//! refuses uses outside the domain the fused executor is required to implement.
use super::*;

const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;

pub(super) struct Owners<'a> {
    pub(super) function: &'a str,
    pub(super) vector: &'a ResolvedBinding,
    pub(super) counter: &'a ResolvedBinding,
    pub(super) item: &'a ResolvedBinding,
    pub(super) element: &'a T,
}

impl Owners<'_> {
    /// A suffix binding cannot reuse a construction value identity.
    fn fresh(&self, binding: &ResolvedBinding) -> Result<(), Refusal> {
        if binding.id == self.vector.id || binding.id == self.counter.id || binding.id == self.item.id
        {
            return Err(Refusal::Forged("suffix rebinds a construction value"));
        }
        Ok(())
    }
}

fn statement_allowed(statement: &S) -> Option<(&ResolvedBinding, &E)> {
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
            Some((binding, value))
        }
        S::Assign {
            binding,
            field: None,
            value,
            ..
        } if crate::hir::is_scalar_resolved_type(&binding.ty) && binding.ownership == O::Value => {
            Some((binding, value))
        }
        _ => None,
    }
}

struct Census<'a> {
    frames: [Option<crate::hir::lexical_exit::ChildCursor<'a>>; DEPTH],
    len: usize,
    work: usize,
}

impl<'a> Census<'a> {
    fn debit(&mut self) -> Result<(), Refusal> {
        self.work = self
            .work
            .checked_sub(1)
            .ok_or(Refusal::Suffix("suffix census work bound"))?;
        Ok(())
    }

    fn expression(
        &mut self,
        program: &ResolvedProgram,
        owners: &Owners<'_>,
        root: &'a E,
    ) -> Result<(), Refusal> {
        self.frames[0] = Some(crate::hir::lexical_exit::ChildCursor::new(root));
        self.len = 1;
        while self.len != 0 {
            self.debit()?;
            let (started, expression) = self.frames[self.len - 1]
                .as_ref()
                .map(|frame| (frame.is_started(), frame.expression()))
                .ok_or(Refusal::Suffix("suffix census frame"))?;
            if !started {
                self.node(program, owners, expression)?;
            }
            let frame = self.frames[self.len - 1]
                .as_mut()
                .ok_or(Refusal::Suffix("suffix census frame"))?;
            if let Some(child) = frame.next() {
                *self
                    .frames
                    .get_mut(self.len)
                    .ok_or(Refusal::Suffix("suffix census depth bound"))? =
                    Some(crate::hir::lexical_exit::ChildCursor::new(child));
                self.len += 1;
            } else {
                self.frames[self.len - 1] = None;
                self.len -= 1;
            }
        }
        Ok(())
    }

    fn node(&mut self, program: &ResolvedProgram, owners: &Owners<'_>, e: &'a E) -> Result<(), Refusal> {
        if let Some(refusal) = super::shape::vec_constructor(e) {
            return Err(match refusal {
                Refusal::Shape(_) => Refusal::Suffix("Vec constructor allocates after construction"),
                forged => forged,
            });
        }
        let scalar = crate::hir::is_scalar_resolved_type(&e.ty) && e.ownership == O::Value;
        let view = matches!(e.ty, T::Str | T::SliceU8) && e.ownership == O::Borrow;
        match &e.kind {
            K::Int(_)
            | K::Usize(_)
            | K::Uint8(_)
            | K::Bool(_)
            | K::Unary { .. }
            | K::Binary { .. }
            | K::If { .. } => Ok(()),
            K::Project { .. } | K::Place(_) if scalar => Ok(()),
            K::Match {
                mode: crate::hir::ResolvedMatchMode::Value,
                scrutinee,
                arms,
            } if matches!(&scrutinee.ty, T::Nominal { declaration, arguments }
                    if declaration.as_str() == crate::prelude::OPTION_ID
                        && arguments.as_slice() == [T::U8])
                && arms.len() == 2
                && arms.iter().all(|arm| arm.guard.is_none()) =>
            {
                Ok(())
            }
            K::Place(place)
                if place.projections.is_empty()
                    && (view
                        || matches!(&e.ty, T::Nominal { declaration, .. }
                            if declaration.as_str() == crate::prelude::VEC_ID)
                            && matches!(e.ownership, O::Own | O::Borrow)) =>
            {
                Ok(())
            }
            K::BorrowPlace { operation, .. }
                if matches!(
                    operation.as_str(),
                    crate::byte_ops::STR_AS_BYTES_ID | crate::byte_ops::STRING_AS_STR_ID
                ) =>
            {
                Ok(())
            }
            K::VecFieldRead { .. } if view || scalar => Ok(()),
            K::Block { statements, .. } => {
                // A lowered traversal is accepted only after its complete
                // independent freeze replay; otherwise loops refuse.
                let traversal = crate::loan_plan::traversal::inspect_borrowed(
                    program,
                    e,
                    owners.element,
                    &mut self.work,
                )
                .ok_or(Refusal::Suffix("borrowed traversal replay refused"))?
                .is_some();
                for statement in statements {
                    self.debit()?;
                    match statement {
                        S::While { .. } | S::LexicalWhile { .. } if traversal => {}
                        other => {
                            let (binding, _) = statement_allowed(other)
                                .ok_or(Refusal::Suffix("owner mutation or unsupported statement"))?;
                            owners.fresh(binding)?;
                        }
                    }
                }
                Ok(())
            }
            K::Call {
                callee,
                instance: None,
                ..
            } if callee.as_str() != owners.function
                && matches!(
                    callee.as_str(),
                    crate::vec_ops::LEN_ID
                        | crate::vec_ops::GET_ID
                        | crate::vec_ops::SORT_OWNED_ID
                        | crate::byte_ops::LEN_ID
                        | crate::byte_ops::GET_ID
                        | crate::string_ops::I64_FROM_USIZE_ID
                        | crate::str_ops::LEN_BYTES_ID
                ) =>
            {
                Ok(())
            }
            K::Call { .. } => Err(Refusal::Suffix("call outside the closed builtin domain")),
            _ => Err(Refusal::Suffix("expression outside the closed use domain")),
        }
    }
}

pub(super) fn validate<'a>(
    program: &ResolvedProgram,
    owners: &Owners<'_>,
    suffix: &'a [S],
    tail: &'a E,
) -> Result<(), Refusal> {
    // Actual census layout only; flat statement scans debit the work counter.
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Census<'_>>()) {
        return Err(Refusal::Scratch);
    }
    let mut census = Census {
        frames: [None; DEPTH],
        len: 0,
        work: crate::cache_codec::MAX_NODES,
    };
    for statement in suffix {
        census.debit()?;
        let (binding, value) = statement_allowed(statement)
            .ok_or(Refusal::Suffix("owner mutation or unsupported statement"))?;
        owners.fresh(binding)?;
        census.expression(program, owners, value)?;
    }
    census.expression(program, owners, tail)
}

#[cfg(test)]
pub(super) fn scratch_bytes() -> usize {
    std::mem::size_of::<Census<'_>>()
}
