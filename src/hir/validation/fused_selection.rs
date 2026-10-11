//! Validator hooks for an explicit, joined `FusedConstructionV1` selection.
//!
//! With no selection, every hook is the unchanged ordinary rule. A selection
//! changes exactly two things: the joined loop is recognized by pointer
//! identity in place of the generic while-shape re-check (the same
//! replacement the 32-row finite construction proof gets), and capacity is
//! rebuilt under the selected strategy with that loop projected as a fused
//! construction. Every node of the loop still passes ordinary type,
//! ownership, loan, cleanup and status validation.
use super::*;

impl<'a> HirValidator<'a> {
    pub(super) fn with_selected(
        mut self,
        selected: Option<crate::hir::selected_fused_candidate::Selected<'a>>,
    ) -> Self {
        self.selected = selected;
        self
    }

    /// True only for the joined loop of a held selection over this program.
    pub(super) fn selected_loop(&self, condition: &ResolvedExpr, body: &ResolvedExpr) -> bool {
        self.selected
            .is_some_and(|selected| selected.selects(self.program, condition, body))
    }

    /// The target-neutral capacity projection rebuilt from this HIR.
    pub(super) fn analyze_capacity(&self) -> Result<(), Diagnostic> {
        match self.selected {
            None => analyze_byte_data_capacity(self.program).map(|_| ()),
            Some(selected) => {
                crate::hir::byte_capacity::analyze_selected_byte_data_capacity(
                    self.program,
                    selected,
                )
                .map(|_| ())
            }
        }
    }
}
