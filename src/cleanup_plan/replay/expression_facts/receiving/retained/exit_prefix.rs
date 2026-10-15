//! Cold malformed-owner proof only; valid exits initialize no new carrier.
use super::*;

pub(super) struct Probe<'a> {
    pub(super) owner: BlockId,
    pub(super) current: BlockId,
    pub(super) exit: crate::cleanup_plan::ExitTargetId,
    pub(super) previous: Option<&'a crate::cleanup_plan::CleanupBlock>,
    pub(super) repeated: bool,
}
// Full initialized and receiving probe overlap, get-result option, plus the simultaneously received previous/seen
// pattern bindings and comparison booleans. No old Directory/header credit.
pub(super) const CONTROL_BYTES: usize = 2 * std::mem::size_of::<Probe<'static>>()
    + std::mem::size_of::<Option<&crate::cleanup_plan::CleanupBlock>>()
    + std::mem::size_of::<&crate::cleanup_plan::CleanupBlock>()
    + std::mem::size_of::<crate::cleanup_plan::ExitTargetId>()
    + 3 * std::mem::size_of::<bool>();
// A conservative source-derived ceiling, paid before every new proof read:
// five initializers, owner comparison/index conversion (2), indexed node/key/
// child access (3), previous assignment/binding (2), id comparison (1), variant
// discriminator/binding/value comparison (3), conjunction/repeated assignment
// and final decision (3): nineteen one-step operations, plus one for
// the optional-presence branch. Existing numeric/comparator prices stay fixed.
pub(super) const PROBE_UNITS: usize = 20;
