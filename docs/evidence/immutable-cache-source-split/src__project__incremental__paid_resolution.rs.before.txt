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
        if !self.semantic {
            return None;
        }
        if synthetic.functions().len() > MAX_PROJECT_CHECKED_FUNCTIONS {
            return Some(Err(SelectiveFailure::Project(capacity(
                "checked function inventory exceeds its bound",
            ))));
        }
        Some(
            crate::hir::identity_resolution::resolve_source_checked(
                synthetic,
                self.checked.get(path).map(|entry| &entry.synthetic),
                self.checked.get(path).map(|entry| &entry.resolved),
                self.checked.get(path).map(|entry| &entry.function_costs),
                private,
                |bytes| owner.reserve(bytes),
            )
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

#[cfg(test)]
impl FrontendPass {
    /// A compiler-created previous candidate for actual receiver tests. The
    /// ordinary checked resolver still authenticates its complete environment.
    pub(crate) fn paid_receiver_candidate(
        synthetic: crate::ast::Program,
        resolved: crate::hir::ResolvedProgram,
        function_costs: std::collections::BTreeMap<String, usize>,
    ) -> Self {
        let checked = std::collections::BTreeMap::from([(
            synthetic.path.clone(),
            std::sync::Arc::new(super::CheckedModule {
                synthetic,
                resolved,
                function_costs,
                resolver_bytes: 0,
                retained_loan_bytes: 0,
            }),
        )]);
        Self {
            indexed_imports: Vec::new(),
            entries: std::collections::BTreeMap::new(),
            parsed: 0,
            reused: 0,
            canonicalizations: 0,
            parsed_bytes: 0,
            reused_bytes: 0,
            resolved: 0,
            retained_source_bytes: 0,
            ast_budget: 0,
            semantic: true,
            checked,
            next_checked: std::collections::BTreeMap::new(),
            checked_reused: 0,
            functions_resolved: 0,
            functions_reused: 0,
        }
    }
}
