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

// Complete original vector banks with only their real compact payment Result changed.
pub(in crate::cleanup_plan::replay) const VECTOR_QUERY: usize = size_of::<Result<(), ByteFailure>>()
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<&ReplayBudget>() // selected caller/method
    + 2 * size_of::<&Vec<ExprSkeletonPath>>() // capacity caller/method
    + 10 * size_of::<Option<usize>>() // five checked results and their matches
    + 28 * size_of::<usize>() // price, arithmetic self/RHS/results and held outputs
    + 5 * size_of::<bool>() // selected/negation, bound, equality, saved-backing test
    // On false selection the original consuming loop is acquired AFTER the
    // first borrowed item. This additional iterator/next frame is prepaid here;
    // it is unused when the mutable input frame returns the vector directly.
    + 2 * size_of::<std::vec::IntoIter<ExprSkeletonPath>>()
    + 2 * size_of::<Option<ExprSkeletonPath>>();

const _: () = assert!(
    crate::cleanup_plan::replay::sequence_vector::QUERY - VECTOR_QUERY
        == size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>()
);

pub(in crate::cleanup_plan::replay) const VECTOR_FRAME: usize = size_of::<Result<(), ByteFailure>>()
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>() + size_of::<usize>()
    + 2 * size_of::<std::slice::IterMut<'static, ExprSkeletonPath>>()
    + 2 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + size_of::<&mut [ExprSkeletonPath]>()
    + 2 * size_of::<Option<&mut ExprSkeletonPath>>()
    + size_of::<&mut ExprSkeletonPath>()
    + 2 * size_of::<Observations>() // Default return and scratch receiver
    + 2 * size_of::<Vec<std::rc::Rc<crate::cleanup_plan::replay::SkeletonObservation>>>() // Vec::default return/field
    + size_of::<Option<std::rc::Rc<crate::cleanup_plan::replay::skeleton_work::ObservationDag>>>();

const _: () = assert!(
    crate::cleanup_plan::replay::sequence_vector::FRAME - VECTOR_FRAME
        == size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>()
);
