// src/workspace_graph/custody.rs
//! Source custody: the one routine that turns moved sources and restored
//! programs into `SourceMetadata` and `CurrentParsedSource` owners, for both
//! the ordinary project route and the cached route.
//!
//! Everything is prepaid from the original active bounded_output ledger by
//! `SourceCustody::plan`, before any owner is allocated and inside the
//! caller's consumed snapshot. `SelectionPlan::complete` runs after the
//! original dependency restore and re-authenticates the same ledger. There
//! are no credits, refunds or new limits, and any mismatch fails closed.
use std::mem::size_of;
use std::sync::Arc;

use super::current_source::{
    arc_carrier_bytes, CurrentParsedSource, CurrentParsedSourceError, SourceMetadata,
};
use super::dependency_pruning::PrunedDependencyFunctions;
use super::WorkspaceSource;
use crate::ast::Program;
use crate::bounded_output::prepaid::{PrepaidAcquisition, PrepaidError};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum CustodyError {
    /// The ledger refused, or was not the charged one. The original first
    /// refusal stays recorded on the ledger.
    Prepaid(PrepaidError),
    /// A byte or element count overflowed `usize`.
    Overflow,
    /// Sources, programs and the plan disagree on module count.
    CountMismatch,
    /// Slot, path allocation or restored function count differs from the plan.
    PlanMismatch { slot: usize },
    /// The program path differs from the source path.
    PathInconsistent { slot: usize },
    /// A vector's actual capacity differs from the capacity that was paid.
    CapacityDrift,
}

impl From<PrepaidError> for CustodyError {
    fn from(error: PrepaidError) -> Self {
        Self::Prepaid(error)
    }
}

/// Planned selection for one module, fixed before the original restore.
struct PlannedSlot {
    slot: usize,
    path_ptr: usize,
    path_len: usize,
    restored_len: usize,
    program_path_ptr: usize,
    program_module_ptr: usize,
    ordinals: Option<Vec<usize>>,
}

/// Prepaid selection plan. Holds the paid, still unauthenticated construction
/// acquisition for the final deferred step.
#[must_use]
pub(super) struct SelectionPlan {
    slots: Vec<PlannedSlot>,
    construct: PrepaidAcquisition,
}

/// Owners of every current source: metadata for file facts, parsed sources
/// for graph construction. Both vectors share the same `SourceMetadata` Arcs.
pub(crate) struct SourceCustody {
    metadata: Vec<Arc<SourceMetadata>>,
    parsed: super::current_source::ParsedSources,
}

fn mul(count: usize, each: usize) -> Result<usize, CustodyError> {
    count.checked_mul(each).ok_or(CustodyError::Overflow)
}

fn add(left: usize, right: usize) -> Result<usize, CustodyError> {
    left.checked_add(right).ok_or(CustodyError::Overflow)
}

fn exact<T>(capacity: usize) -> Result<Vec<T>, CustodyError> {
    let vec = Vec::with_capacity(capacity);
    if vec.capacity() == capacity {
        Ok(vec)
    } else {
        Err(CustodyError::CapacityDrift)
    }
}

impl SourceCustody {
    /// Plans the selection from the original pruning record and the still
    /// pruned programs. Must run inside the original ledger after its
    /// successful pre-bound charge and before `pruned.restore(..)`.
    pub(super) fn plan(
        sources: &[WorkspaceSource],
        programs: &[Program],
        pruned: &PrunedDependencyFunctions,
    ) -> Result<SelectionPlan, CustodyError> {
        let count = sources.len();
        if programs.len() != count {
            return Err(CustodyError::CountMismatch);
        }
        let mut ordinal_bytes = 0usize;
        for (slot, program) in programs.iter().enumerate() {
            if pruned.removed_len(slot) > 0 {
                ordinal_bytes = add(
                    ordinal_bytes,
                    mul(program.functions.len(), size_of::<usize>())?,
                )?;
            }
        }
        let plan_bytes = add(mul(count, size_of::<PlannedSlot>())?, ordinal_bytes)?;
        let per_module = add(
            add(
                arc_carrier_bytes::<SourceMetadata>(),
                arc_carrier_bytes::<CurrentParsedSource>(),
            ),
            add(
                size_of::<Arc<SourceMetadata>>(),
                size_of::<Arc<CurrentParsedSource>>(),
            ),
        )?;
        let construct_bytes = add(
            mul(count, per_module)?,
            arc_carrier_bytes::<Vec<Arc<CurrentParsedSource>>>(),
        )?;

        if !crate::bounded_output::reserve_active_required(
            size_of::<SelectionPlan>()
                .checked_add(size_of::<SourceCustody>())
                .ok_or(CustodyError::Overflow)?,
        ) {
            return Err(CustodyError::Prepaid(PrepaidError::Refused));
        }
        let plan_paid = PrepaidAcquisition::acquire(plan_bytes)?;
        let construct = PrepaidAcquisition::acquire(construct_bytes)?;
        let mut spend = plan_paid.authenticate()?;

        spend.take(mul(count, size_of::<PlannedSlot>())?)?;
        let mut slots = exact::<PlannedSlot>(count)?;
        for (slot, (source, program)) in sources.iter().zip(programs).enumerate() {
            let retained = program.functions.len();
            let removed = pruned.removed_len(slot);
            let ordinals = if removed > 0 {
                spend.take(mul(retained, size_of::<usize>())?)?;
                let mut out = exact::<usize>(retained)?;
                pruned.retained_positions(slot, retained, &mut out);
                if out.len() != retained || out.capacity() != retained {
                    return Err(CustodyError::CapacityDrift);
                }
                Some(out)
            } else {
                None
            };
            slots.push(PlannedSlot {
                slot,
                path_ptr: source.path.as_ptr() as usize,
                path_len: source.path.len(),
                restored_len: add(retained, removed)?,
                program_path_ptr: program.path.as_ptr() as usize,
                program_module_ptr: program.module.as_ptr() as usize,
                ordinals,
            });
        }
        spend.finish()?;
        Ok(SelectionPlan { slots, construct })
    }
}

impl SelectionPlan {
    /// Builds the custody from the actual moved sources and the programs after
    /// the original restore. Only the exact moved source and Program slots
    /// from this invocation can finish the plan.
    pub(super) fn complete(
        self,
        sources: Vec<WorkspaceSource>,
        programs: Vec<Program>,
    ) -> Result<SourceCustody, CustodyError> {
        let count = self.slots.len();
        if sources.len() != count || programs.len() != count {
            return Err(CustodyError::CountMismatch);
        }
        let mut spend = self.construct.authenticate_deferred()?;
        let mut reserve = |bytes: usize| spend.take(bytes);

        let mut metadata_dir = {
            reserve(mul(count, size_of::<Arc<SourceMetadata>>())?)?;
            exact::<Arc<SourceMetadata>>(count)?
        };
        let mut parsed_dir = {
            reserve(mul(count, size_of::<Arc<CurrentParsedSource>>())?)?;
            exact::<Arc<CurrentParsedSource>>(count)?
        };

        let mut planned = self.slots.into_iter();
        for (slot, (source, program)) in sources.into_iter().zip(programs).enumerate() {
            let mut plan = planned.next().ok_or(CustodyError::CountMismatch)?;
            if plan.slot != slot
                || plan.path_ptr != source.path.as_ptr() as usize
                || plan.path_len != source.path.len()
                || plan.restored_len != program.functions.len()
                || plan.program_path_ptr != program.path.as_ptr() as usize
                || plan.program_module_ptr != program.module.as_ptr() as usize
            {
                return Err(CustodyError::PlanMismatch { slot });
            }
            let metadata = SourceMetadata::new(source, &mut reserve)?;
            let parsed = CurrentParsedSource::new(
                Arc::clone(&metadata),
                program,
                plan.ordinals.take(),
                &mut reserve,
            )
            .map_err(|error| match error {
                CurrentParsedSourceError::Reserve(error) => CustodyError::Prepaid(error),
                CurrentParsedSourceError::PathInconsistent => {
                    CustodyError::PathInconsistent { slot }
                }
            })?;
            metadata_dir.push(metadata);
            parsed_dir.push(parsed);
        }
        reserve(arc_carrier_bytes::<Vec<Arc<CurrentParsedSource>>>())?;
        let parsed_dir = Arc::new(parsed_dir);
        spend.finish()?;
        Ok(SourceCustody {
            metadata: metadata_dir,
            parsed: parsed_dir,
        })
    }
}

impl SourceCustody {
    pub(crate) fn len(&self) -> usize {
        self.parsed.len()
    }

    /// Metadata Arcs for file facts. They never reach an AST.
    pub(crate) fn metadata(&self) -> &[Arc<SourceMetadata>] {
        &self.metadata
    }

    pub(crate) fn parsed(&self) -> &super::current_source::ParsedSources {
        &self.parsed
    }

    #[cfg(test)]
    pub(crate) fn metadata_at(&self, slot: usize) -> Option<&Arc<SourceMetadata>> {
        self.metadata.get(slot)
    }

    #[cfg(test)]
    pub(crate) fn parsed_at(&self, slot: usize) -> Option<&Arc<CurrentParsedSource>> {
        self.parsed.get(slot)
    }

    /// The actual restored programs, for Agent and law extraction.
    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<Arc<SourceMetadata>>,
        super::current_source::ParsedSources,
    ) {
        (self.metadata, self.parsed)
    }

    #[cfg(test)]
    pub(crate) fn programs(&self) -> impl Iterator<Item = &Program> {
        self.parsed.iter().map(|parsed| parsed.program())
    }
}

#[cfg(test)]
mod tests;
