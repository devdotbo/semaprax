//! Actual compact Result receivers at the unchanged HIR payment frontiers.
use super::{
    Diagnostic, ExprSkeletonPath, Observations, ReplayBudget, ResolvedFunction, SkeletonWork,
};
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::mem::size_of;

pub(in crate::cleanup_plan::replay) const HIR_SELECTION_CONTROLS: usize = size_of::<Result<(), ByteFailure>>()
    + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>() + size_of::<&str>()
    + 2 * size_of::<usize>() // price and length
    + 4 * size_of::<bool>() // single predicate pair and first-work continuation initializer/slot
    + size_of::<&std::borrow::Cow<'_, [ExprSkeletonPath]>>()
    + size_of::<&[ExprSkeletonPath]>();

pub(in crate::cleanup_plan::replay) const HIR_MOVE_CONTROLS: usize = 2 * size_of::<Result<(), ByteFailure>>()
    + size_of::<Observations>() // existing named result receives a move, no aggregate clone
    + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>() + 2 * size_of::<usize>() // price and direct zero index
    + size_of::<&mut SkeletonWork<'_, '_>>() // retained logical work call
    + size_of::<&std::borrow::Cow<'_, [ExprSkeletonPath]>>() // direct index receiver
    + 2 * size_of::<&[ExprSkeletonPath]>() // dereference and index receivers
    + size_of::<&ExprSkeletonPath>();

// These compare both complete real bank expressions. No stand-in IR layout,
// lowered floor, changed cap, or omitted transport contributes to the delta.
const _: () = assert!(
    super::HIR_SELECTION_CONTROLS - HIR_SELECTION_CONTROLS
        == size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>()
);
const _: () = assert!(
    super::HIR_MOVE_CONTROLS - HIR_MOVE_CONTROLS
        == 2 * (size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>())
);

impl SkeletonWork<'_, '_> {
    pub(in crate::cleanup_plan::replay) fn charge_compact(
        &mut self,
        units: usize,
        phase: &str,
    ) -> Result<(), ByteFailure> {
        self.budget
            .charge_compact_skeleton(self.function, units, phase)
    }
}

macro_rules! hir_selection {
    ($work:ident) => {
        $crate::cleanup_plan::replay::physical_budget::compact_try!($work
            .budget
            .reserve_compact_bytes(
                $work.function,
                $crate::cleanup_plan::replay::final_output::compact::HIR_SELECTION_CONTROLS,
                "HIR single-suffix selection controls",
            ))
    };
}
pub(in crate::cleanup_plan::replay) use hir_selection;

macro_rules! hir_move {
    ($work:ident) => {
        $crate::cleanup_plan::replay::physical_budget::compact_try!($work
            .budget
            .reserve_compact_bytes(
                $work.function,
                $crate::cleanup_plan::replay::final_output::compact::HIR_MOVE_CONTROLS,
                "HIR single-suffix move controls",
            ))
    };
}
pub(in crate::cleanup_plan::replay) use hir_move;

macro_rules! hir_work {
    ($work:ident) => {
        $crate::cleanup_plan::replay::physical_budget::compact_try!(
            $work.charge_compact(1, "skeleton prefix clone")
        )
    };
}
pub(in crate::cleanup_plan::replay) use hir_work;
