//! Full ordinary shape lifecycle predicates on the existing held payer.
//! The summary collector retains the complete old lifecycle validation branches.
use super::inventory_metadata as metadata;
use super::*;
pub(super) mod equal;
pub(super) mod native;
pub(super) mod outcome;
pub(super) mod step;
pub(super) mod variants;

#[cfg(test)]
mod tests;
