//! UNVALIDATED test-only precursor; compiled only under `cfg(test)`. It is
//! kept only for static refusal gates (strategy, capacity, legacy, stale,
//! noncanonical plans); no executable host gate counts its bytes.
//!
//! It rebuilds `Selected` from the private `construction_fusion` descriptor,
//! the carrier mapping and a caller-supplied capacity summary. None of these is
//! the root `SourceProof`, the typed source/HIR join or complete selected
//! validation, and a rescaled 32-row HIR carries no count authority. It
//! therefore produces a `PrecursorModule` for executor-mechanics gates only,
//! never an `Artifact`. It still runs every executor-side check, including the
//! canonical cleanup and loan plan authentication.
use super::facts::{self, Selected};
use super::{Grant, Refusal};
use crate::byte_data_capacity::{
    AllocationStrategy, ProgramCapacitySummary, MAX_BYTES_COPY_SITES,
    MAX_OWNED_BYTE_PAYLOAD_BYTES,
};
use crate::construction_fusion::{self, native_provenance::Mapping, Descriptor};
use crate::{ast, hir};

pub(super) struct Precursor<'a> {
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
    descriptor: Descriptor<'a>,
    mapping: Mapping<'a>,
}

/// Unpublishable lowered bytes plus the host cross-check grant.
pub(super) struct PrecursorModule {
    pub(super) bytes: Vec<u8>,
    pub(super) grant: Grant,
}

pub(super) fn inspect<'a>(
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
    capacity: &ProgramCapacitySummary,
) -> Result<Precursor<'a>, Refusal> {
    if capacity.strategy() != AllocationStrategy::FusedConstructionV1 {
        return Err(Refusal::Strategy);
    }
    let descriptor = construction_fusion::derive(source, program).ok_or(Refusal::Join)?;
    let mapping =
        construction_fusion::native_provenance::derive(source, program).ok_or(Refusal::Carrier)?;
    if !descriptor.still_held(source, program) || !mapping.still_held(source, program) {
        return Err(Refusal::Stale);
    }
    let rows = u32::try_from(descriptor.count()).map_err(|_| Refusal::Layout)?;
    let width = u64::try_from(descriptor.width()).map_err(|_| Refusal::Layout)?;
    let extent = u64::from(rows)
        .checked_mul(width)
        .ok_or(Refusal::Layout)?;
    if extent != descriptor.payload_bytes() {
        return Err(Refusal::Layout);
    }
    let summary = capacity
        .function(descriptor.typed().function.id.as_str())
        .ok_or(Refusal::Capacity)?;
    if summary.fused_backing_events != 1
        || summary.bytes_copy_sites == 0
        || summary.bytes_copy_sites > MAX_BYTES_COPY_SITES
        || summary.owned_byte_payload_bytes < extent
        || summary.owned_byte_payload_bytes > MAX_OWNED_BYTE_PAYLOAD_BYTES
    {
        return Err(Refusal::Capacity);
    }
    let precursor = Precursor {
        source,
        program,
        descriptor,
        mapping,
    };
    facts::check(&precursor.selected()?, source, program)?;
    Ok(precursor)
}

impl Precursor<'_> {
    fn selected(&self) -> Result<Selected<'_>, Refusal> {
        let typed = self.descriptor.typed();
        let byte_field = self.descriptor.byte_field();
        Ok(Selected {
            source: self.source,
            program: self.program,
            function: typed.function,
            record: typed.record,
            vector: typed.vector,
            counter: typed.counter,
            item: typed.item,
            condition: typed.condition,
            body: typed.body,
            constructor: typed.constructor,
            push: typed.push,
            step: typed.step,
            producer: self.descriptor.byte_site(),
            byte_field,
            suffix: typed.suffix,
            tail: typed.tail,
            rows: u32::try_from(self.descriptor.count()).map_err(|_| Refusal::Layout)?,
            width: u64::try_from(self.descriptor.width()).map_err(|_| Refusal::Layout)?,
            order: facts::roles(typed.record, byte_field)?,
            mapping: Some(&self.mapping),
        })
    }
    pub(super) fn rows(&self) -> usize {
        self.descriptor.count()
    }
    pub(super) fn width(&self) -> usize {
        self.descriptor.width()
    }
    pub(super) fn order(&self) -> Result<[&'static str; 3], Refusal> {
        Ok(self.selected()?.order)
    }
    /// Recheck against the exact objects, then lower. Not publishable.
    pub(super) fn module(
        &self,
        source: &ast::Program,
        program: &hir::ResolvedProgram,
    ) -> Result<PrecursorModule, Refusal> {
        let selected = self.selected()?;
        facts::check(&selected, source, program)?;
        let (bytes, grant) = super::module(&selected)?;
        Ok(PrecursorModule { bytes, grant })
    }
}
