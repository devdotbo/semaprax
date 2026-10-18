//! Actual new singleton slots; old producer/Result tariffs are never credited.
use super::*;
use std::mem::size_of;

// One newly distinct signature-work site and one receiving-byte site. These
// narrow, sequential primitive frames belong to this exact id_inner activation.
// The existing direct-root request acquires them before either can form.
type Primitive<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
type FrameGrowth<'a, 'hir> = (
    &'a mut ReplayBudget,
    &'a ResolvedFunction,
    &'a mut Vec<CanonicalFrame<'hir>>,
    usize,
    &'a str,
);
// Historical owned-One candidate had eight additional growth frames.
// Keep that full invoice independently, without treating it as live credit.
#[cfg(test)]
pub(super) const OWNED_SCHEDULE_PRIMITIVES: usize =
    8 * (2 * size_of::<FrameGrowth<'static, 'static>>() + 2 * size_of::<Result<(), Diagnostic>>());
// Actual shared lexical continuation leaves only two scheduling expansions:
// original vector and new One, hence two additional growth primitive sites.
pub(super) const SCHEDULE_PRIMITIVES: usize =
    2 * (2 * size_of::<FrameGrowth<'static, 'static>>() + 2 * size_of::<Result<(), Diagnostic>>());
// Four vector publication expansions each have one real new receiving payment.
// All four call/formal/Result frames are acquired before any can form.
pub(super) const VECTOR_PRIMITIVES: usize =
    4 * (2 * size_of::<Primitive<'static>>() + 2 * size_of::<Result<(), Diagnostic>>());
pub(super) const VECTOR_TRANSPORT: usize =
    2 * size_of::<u64>() + 2 * size_of::<Vec<(Label, SharedWeighted)>>();
pub(super) const PRIMITIVES: usize = 2
    * (2 * size_of::<Primitive<'static>>() + 2 * size_of::<Result<(), Diagnostic>>())
    + SCHEDULE_PRIMITIVES
    + VECTOR_PRIMITIVES;

// Actual borrowed OnePassLabels selector, its constant-pattern predicate and
// the conditional reserve operand. PUBLICATION pays before the selector reads.
pub(super) const SELECT: usize = 2 * size_of::<&lexical_successors::OnePassLabels<'static>>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<usize>();

// Label references point into the immutable automaton held for 'hir. No
// temporary Label/Rc clone is constructed or destroyed on a signature hit.
// Both original state and child Rc owners retain their exact old Drop points.
// A miss clones into the final owned signature; all clone actual/formal
// reference and owned return/receiving fields are prepaid here, independently
// of the conservatively retained original publication-clone fees.
pub(super) const FIELDS: usize = 2 * size_of::<(&Label, SharedWeighted)>()
    + 2 * size_of::<&Label>()
    + 2 * size_of::<SharedWeighted>()
    + size_of::<&SharedWeighted>()
    + 2 * size_of::<&Label>()
    + 2 * size_of::<Label>();

// The exact real query type owns its actual fields. Fresh &Rc query receivers
// and the existing scalar key copy remain; iterator/remove alone disappeared.
pub(super) const RECEIVING: usize =
    lexical_successors::frame_loop::borrowed_single::INLINE_RECEIVING;

#[cfg(test)]
pub(super) mod census;
