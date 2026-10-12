//! Exact finite iteration capacity, separate from type, ownership and cleanup authority.
//! The first profile has one fresh record and one direct owned-record discard.
//! The checked counter step dominates every normal/Continue path of its sole arm.
//! No hint attached to AST/HIR supplies this fact.
mod resolved;
mod source;
pub(crate) use resolved::inspect as resolved;
pub(crate) use source::{inspect as source, permits as source_permits};
const LIMIT: u32 = crate::byte_data_capacity::MAX_BYTES_COPY_SITES;
const DEPTH: usize = crate::cache_codec::MAX_DEPTH;

#[cfg(test)]
mod tests;
