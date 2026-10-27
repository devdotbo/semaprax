//! Actual append probe backing retained with the receiving immutable storage.
//! Entries are private discovery metadata, never admission or plan authority.
use super::*;
#[derive(Default)]
pub(super) struct Scratch {
    pub(super) local: Vec<(LoanId, usize)>,
    // Source occurrence until full validation, then the derived origin ordinal.
    pub(super) liveness: Vec<(LiveKey, usize)>,
    pub(super) ids: Vec<(IdKey, LoanId, usize)>,
}
