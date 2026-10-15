//! Private consumed receipt of one finished closed scalar lexical Session.
//! It is not Clone, not public, not serialized and has no constructor outside
//! the receiver. It names the exact live module object, and it is the only
//! value that can select this family's Graph schema. Plan schemas, cached
//! Arcs, copied inventories and detached equal HIR cannot construct it.
use super::*;

pub(in crate::workspace_graph::source_schema) struct Received<'a> {
    module: &'a WorkspaceResolvedModule,
    flags: census::Flags,
}

impl<'a> Received<'a> {
    /// Only the receiver seals, after `finish_paid` has returned the same
    /// ledger it held for acquisition, authentication and every member replay.
    pub(super) fn seal(module: &'a WorkspaceResolvedModule, flags: census::Flags) -> Self {
        Self { module, flags }
    }

    /// Consume the receipt for exactly its own module. This runs after the
    /// ledger is released, so it does no per-member work: the receipt holds a
    /// shared `'a` borrow of the replayed module for its whole life, and the
    /// module's function carrier (owned `Vec` or immutable checked `Arc`
    /// range) has no interior mutability. Pointer equality with that object
    /// is therefore the exact replayed member sequence.
    ///
    /// The shared base-lattice mapping receives only census-derived flags; the
    /// census has excluded every match, template, record pattern, generic
    /// record and Option-try node, so those predicates are fixed `false`.
    pub(in crate::workspace_graph::source_schema) fn select(
        self,
        module: &WorkspaceResolvedModule,
    ) -> Option<&'static str> {
        if !std::ptr::eq(self.module, module) {
            return None;
        }
        let flags = self.flags;
        Some(crate::graph::scalar_base_lattice(
            || flags.usize,
            || false,
            || flags.body_loop,
            false,
            || false,
            || false,
            || false,
        ))
    }
}
