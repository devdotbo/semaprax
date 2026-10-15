//! Full saved actual receiving body at64be: guards, selection and fresh profile.
//! It retains the independently saved outward empty-cost map on frontendNone.
use super::super::{observe_acquired, ModuleFailure, Parts};
use crate::bounded_output::{identity_reservation::IdentityReservation, staged};
use crate::hir::identity_resolution::Failure;
use crate::project::incremental::{paid_resolution::SelectiveFailure, FrontendPass};
use crate::workspace_graph::expected_projection::borrowed_leaf::Input;
use std::collections::BTreeMap;

pub(super) fn resolve_module<'s>(
    frontend: Option<&FrontendPass>,
    path: &str,
    synthetic: &'s Input<'_>,
    private: Option<crate::source_verify::private_names::PrivateNames<'s>>,
) -> Result<Parts, ModuleFailure> {
    // Every non-hit module acquires exactly once, on the same Rc<Budget> the
    // caller's resolver-cost interval measures; no fresh quota is created.
    let mut owner = if frontend.is_some_and(FrontendPass::retains_checked_hir) {
        staged!(SelectiveResolution, IdentityReservation::acquire())
    } else {
        staged!(FreshResolution, IdentityReservation::acquire())
    }
    .map_err(ModuleFailure::Setup)?;
    #[cfg(test)]
    observe_acquired(&owner);
    // Both routes execute inside the same linked-source authentication scope.
    // Evaluating a selective Result before these guards would reject retained
    // imported wrapper bodies before the guards could authenticate them.
    crate::vec_ops::with_authenticated_linked_source(|| {
        crate::box_ops::with_authenticated_linked_source(|| {
            let selective = staged!(
                SelectiveResolution,
                frontend.and_then(|cache| {
                    synthetic
                        .owned()
                        .and_then(|input| {
                            cache.resolve_functions_paid(path, input, private, &mut owner)
                        })
                        .or_else(|| {
                            synthetic.borrowed().and_then(|view| {
                                cache.resolve_borrowed_cold_source_paid(
                                    path, view, private, &mut owner,
                                )
                            })
                        })
                })
            );
            if let Some(selective) = selective {
                selective.map(|resolved| resolved.into_parts())
            } else {
                staged!(FreshResolution, {
                    synthetic
                        .resolve_paid(private, &mut owner)
                        .map(|(resolved, work)| {
                            let selected = work.static_text();
                            (resolved, BTreeMap::new(), 0, selected)
                        })
                        .map_err(SelectiveFailure::Resolve)
                })
            }
        })
    })
    .map_err(|failure| match failure {
        SelectiveFailure::Project(report) => ModuleFailure::Report(report),
        SelectiveFailure::Resolve(Failure::Source(report)) => ModuleFailure::Report(report),
        SelectiveFailure::Resolve(Failure::Semantic(error)) => ModuleFailure::Report(vec![error]),
        SelectiveFailure::Resolve(Failure::Identity(_)) => ModuleFailure::Report(owner.publish()),
    })
}
