//! Private consumed receipt of one finished closed scalar lexical Session.
//! It is not Clone, not public, not serialized and has no constructor outside
//! the receiver. It names the exact live module object and its exact ordered
//! member view, and it is the only value that can select this family's Graph
//! schema. Plan schemas, cached Arcs, copied inventories and detached equal
//! HIR cannot construct it.
use super::*;

pub(in crate::workspace_graph::source_schema) struct Received<'a> {
    module: &'a WorkspaceResolvedModule,
    members: crate::hir::items::View<'a, ResolvedFunction>,
    flags: census::Flags,
}

impl<'a> Received<'a> {
    /// Only the receiver seals, after `finish_paid` has returned the same
    /// ledger it held for acquisition, authentication and every member replay.
    pub(super) fn seal(module: &'a WorkspaceResolvedModule, flags: census::Flags) -> Self {
        Self {
            module,
            members: crate::hir::items::ItemSource::view(&module.functions),
            flags,
        }
    }

    /// Consume the receipt for exactly its own module and current members.
    /// The shared base-lattice mapping receives only census-derived flags; the
    /// census has excluded every match, template, record pattern, generic
    /// record and Option-try node, so those predicates are fixed `false`.
    pub(in crate::workspace_graph::source_schema) fn select(
        self,
        module: &WorkspaceResolvedModule,
    ) -> Option<&'static str> {
        if !std::ptr::eq(self.module, module) || self.members.len() != module.functions.len() {
            return None;
        }
        for (held, current) in self.members.iter().zip(&module.functions) {
            if !std::ptr::eq(held, current) {
                return None;
            }
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
