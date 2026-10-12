//! Explicit-owner siblings of the selective checked-module resolvers. The
//! caller acquires one identity report before either route; this module only
//! forwards that same owner. Legacy entries above remain unpaid compatibility.
use super::{capacity, FrontendPass, SelectiveResolvedModule, MAX_PROJECT_CHECKED_FUNCTIONS};
use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
use crate::diagnostic::Diagnostic;
use crate::hir::identity_resolution::Failure;

#[derive(Debug)]
pub(crate) enum SelectiveFailure {
    /// Project-bound refusal before any resolver work; legacy-owned Vec.
    Project(Vec<Diagnostic>),
    Resolve(Failure<Refused>),
}

fn module(
    (resolved, work): (
        crate::hir::ResolvedProgram,
        crate::hir::FunctionResolutionWork,
    ),
) -> SelectiveResolvedModule {
    SelectiveResolvedModule {
        reused_functions: work.reused(),
        static_text: work.static_text(),
        function_costs: work.costs(),
        resolved,
    }
}

impl FrontendPass {
    pub(crate) fn resolve_borrowed_cold_source_paid<'p>(
        &self,
        path: &str,
        synthetic: &'p dyn crate::ast::program_source::ProgramSource,
        private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
        owner: &mut IdentityReservation,
    ) -> Option<Result<SelectiveResolvedModule, SelectiveFailure>> {
        if !self.permits_borrowed_cold_source(path) {
            return None;
        }
        if synthetic.functions().len() > MAX_PROJECT_CHECKED_FUNCTIONS {
            return Some(Err(SelectiveFailure::Project(capacity(
                "checked function inventory exceeds its bound",
            ))));
        }
        Some(
            crate::hir::identity_resolution::resolve_source(synthetic, private, |bytes| {
                owner.reserve(bytes)
            })
            .map(module)
            .map_err(SelectiveFailure::Resolve),
        )
    }

    pub(crate) fn resolve_functions_paid<'p>(
        &self,
        path: &str,
        synthetic: &'p crate::ast::Program,
        private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
        owner: &mut IdentityReservation,
    ) -> Option<Result<SelectiveResolvedModule, SelectiveFailure>> {
        if !self.semantic {
            return None;
        }
        if synthetic.functions.len() > MAX_PROJECT_CHECKED_FUNCTIONS {
            return Some(Err(SelectiveFailure::Project(capacity(
                "checked function inventory exceeds its bound",
            ))));
        }
        let entry = self.checked.get(path);
        Some(
            crate::hir::identity_resolution::resolve_checked(
                synthetic,
                entry.map(|entry| &entry.synthetic),
                entry.map(|entry| &entry.resolved),
                entry.map(|entry| &entry.function_costs),
                private,
                |bytes| owner.reserve(bytes),
            )
            .map(module)
            .map_err(SelectiveFailure::Resolve),
        )
    }
}
