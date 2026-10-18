//! Actual new singleton slots; old producer/Result tariffs are never credited.
use super::*;
use std::mem::size_of;

// One newly distinct signature-work site and one receiving-byte site. These
// narrow, sequential primitive frames belong to this exact id_inner activation.
// The existing direct-root request acquires them before either can form.
type Primitive<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
pub(super) const PRIMITIVES: usize =
    2 * (2 * size_of::<Primitive<'static>>() + 2 * size_of::<Result<(), Diagnostic>>());

// Actual borrowed OnePassLabels selector, its constant-pattern predicate and
// the conditional reserve operand. PUBLICATION pays before the selector reads.
pub(super) const SELECT: usize = 2 * size_of::<&lexical_successors::OnePassLabels<'static>>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<usize>();

// The result's owned label/child receiving pair and the later ExitOne owned
// pattern pair. This is a per-published-frame packet, never a future-loop bound.
// Old OnePass target/row/Result costs remain in the complete original invoice.
pub(super) const FIELDS: usize = 2 * size_of::<(Label, SharedWeighted)>()
    + 2 * size_of::<Label>()
    + 2 * size_of::<SharedWeighted>();

// The exact real query type owns its actual fields. Fresh &Rc query receivers
// and the existing scalar key copy remain; iterator/remove alone disappeared.
pub(super) const RECEIVING: usize =
    lexical_successors::frame_loop::borrowed_single::INLINE_RECEIVING;

#[cfg(test)]
pub(super) mod census;
