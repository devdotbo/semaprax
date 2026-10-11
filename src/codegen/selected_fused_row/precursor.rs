//! UNVALIDATED private precursor input. Test-only.
//!
//! This borrows the old `construction_fusion` source/HIR descriptor and its
//! suffix provenance. Neither is the validated selected owner: no independent
//! `SourceProof` join, no selected capacity strategy and no selected HIR
//! validation stand behind it. It therefore yields a [`PrecursorUnit`] with no
//! artifact authority, exercises only counts that ordinary admission already
//! accepts, and supports no claim about the original 4,096-row program.
use super::{error, selection::Sites};
use crate::construction_fusion::{self, native_provenance};
use crate::diagnostic::Diagnostic;
use crate::{ast, hir};

pub(crate) struct UnvalidatedPrecursor<'a> {
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
    descriptor: construction_fusion::Descriptor<'a>,
    mapping: native_provenance::Mapping<'a>,
}

/// Emitted text of the unvalidated precursor route. Not an artifact.
pub(crate) struct PrecursorUnit {
    pub(crate) unvalidated_source: String,
    pub(crate) placement: [usize; 3],
}

impl<'a> UnvalidatedPrecursor<'a> {
    pub(crate) fn derive(
        source: &'a ast::Program,
        program: &'a hir::ResolvedProgram,
    ) -> Option<Self> {
        let descriptor = construction_fusion::derive(source, program)?;
        let mapping = native_provenance::derive(source, program)?;
        if !descriptor.still_held(source, program) || !mapping.still_held(source, program) {
            return None;
        }
        Some(Self {
            source,
            program,
            descriptor,
            mapping,
        })
    }
    /// Pointer identity with the supplied programs, then a fresh rederivation
    /// whose every selected site and carrier fact is the same borrowed node.
    pub(super) fn authenticate(
        &self,
        source: &ast::Program,
        program: &hir::ResolvedProgram,
    ) -> Result<(), Diagnostic> {
        if !std::ptr::eq(self.source, source) || !std::ptr::eq(self.program, program) {
            return Err(error(
                "precursor does not borrow the supplied source and typed programs",
            ));
        }
        let fresh = Self::derive(self.source, self.program)
            .ok_or_else(|| error("current source/HIR join no longer derives the selection"))?;
        let (left, right) = (self.descriptor.typed(), fresh.descriptor.typed());
        let same_sites = std::ptr::eq(left.function, right.function)
            && std::ptr::eq(left.record, right.record)
            && std::ptr::eq(left.vector, right.vector)
            && std::ptr::eq(left.counter, right.counter)
            && std::ptr::eq(left.item, right.item)
            && std::ptr::eq(left.condition, right.condition)
            && std::ptr::eq(left.body, right.body)
            && std::ptr::eq(left.constructor, right.constructor)
            && std::ptr::eq(left.push, right.push)
            && std::ptr::eq(left.step, right.step)
            && std::ptr::eq(left.suffix, right.suffix)
            && std::ptr::eq(left.tail, right.tail)
            && left.iterations == right.iterations
            && left.width == right.width
            && std::ptr::eq(self.descriptor.byte_site(), fresh.descriptor.byte_site());
        let facts = self.mapping.facts();
        let same_facts = facts.len() == fresh.mapping.facts().len()
            && facts.iter().zip(fresh.mapping.facts()).all(|(a, b)| {
                a.carrier() == b.carrier()
                    && std::ptr::eq(a.producer(), b.producer())
                    && std::ptr::eq(a.field(), b.field())
                    && a.vector() == b.vector()
            });
        if !same_sites || !same_facts {
            return Err(error("supplied precursor disagrees with the current rederivation"));
        }
        Ok(())
    }
    pub(super) fn sites(&self) -> Result<Sites<'_>, Diagnostic> {
        let typed = self.descriptor.typed();
        let hir::ResolvedExprKind::Block { statements, .. } = &typed.function.body.kind else {
            return Err(error("precursor function body is not a block"));
        };
        let Some(hir::ResolvedStatement::Let {
            value: allocation, ..
        }) = statements.first()
        else {
            return Err(error("precursor destination is absent"));
        };
        Ok(Sites {
            program: self.program,
            function: typed.function,
            record: typed.record,
            vector: typed.vector,
            counter: typed.counter,
            item: typed.item,
            allocation,
            condition: typed.condition,
            body: typed.body,
            constructor: typed.constructor,
            producer: self.descriptor.byte_site(),
            push: typed.push,
            step: typed.step,
            suffix: typed.suffix,
            tail: typed.tail,
            rows: typed.iterations,
            width: typed.width,
            carriers: &self.mapping,
        })
    }
}

/// Test-only unvalidated route: same authenticated lowering, no authority.
pub(crate) fn emit_unvalidated_precursor(
    source: &ast::Program,
    program: &hir::ResolvedProgram,
    precursor: &UnvalidatedPrecursor<'_>,
) -> Result<PrecursorUnit, Diagnostic> {
    precursor.authenticate(source, program)?;
    let sites = precursor.sites()?;
    let unit = super::emit::unit(&sites)?;
    Ok(PrecursorUnit {
        unvalidated_source: unit.source,
        placement: unit.placement,
    })
}
