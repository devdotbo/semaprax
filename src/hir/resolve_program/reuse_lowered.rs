//! Only the resolver's pre-rebuild phase may use this clone. All three canonical
//! attachments are independently rebuilt before final validation/publication.
use super::*;

pub(super) fn clone_for_rebuild(function: &ResolvedFunction) -> ResolvedFunction {
    ResolvedFunction {
        id: function.id.clone(),
        name: function.name.clone(),
        params: function.params.clone(),
        result_id: function.result_id.clone(),
        return_type: function.return_type.clone(),
        effects: function.effects.clone(),
        yields: function.yields.clone(),
        requires: function.requires.clone(),
        ensures: function.ensures.clone(),
        body: function.body.clone(),
        cleanup: CleanupInventory::unresolved(),
        cleanup_plan: CleanupPlan::unresolved(),
        loan_plan: LoanPlan::unresolved(),
        span: function.span,
    }
}

#[cfg(test)]
mod tests;
