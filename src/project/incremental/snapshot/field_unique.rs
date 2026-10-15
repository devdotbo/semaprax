//! A cached field-uniqueness descriptor is authenticated against a new
//! resolution of the cached synthetic AST, so a valid descriptor for another
//! same-typed field cannot acquire source authority through the cache. The
//! ordinary Project replay then proves that AST came from canonical source.
use super::*;

fn uses(function: &crate::hir::ResolvedFunction) -> bool {
    std::iter::once(&function.body)
        .chain(&function.requires)
        .chain(&function.ensures)
        .any(|root| {
            let mut found = false;
            crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                found |= callee
                    .as_str()
                    .starts_with(crate::vec_ops::field_unique::PREFIX);
            });
            found
        })
}

pub(super) fn replay_source(
    source: &str,
    synthetic: &Program,
    retained: &crate::hir::ResolvedProgram,
) -> Result<()> {
    if !retained.functions.iter().any(uses) && !source.contains(crate::vec_ops::field_unique::NAME)
    {
        return Ok(());
    }
    let fresh = crate::hir::resolve(synthetic).map_err(|_| {
        invalid("semantic snapshot field-uniqueness source cannot be independently resolved")
    })?;
    if fresh.functions.len() != retained.functions.len() {
        return Err(invalid(
            "semantic snapshot field-uniqueness function inventory disagrees",
        ));
    }
    for function in &fresh.functions {
        let expected = retained
            .functions
            .iter()
            .find(|item| item.id == function.id)
            .ok_or_else(|| invalid("semantic snapshot field-uniqueness function is absent"))?;
        if (uses(function) || uses(expected))
            && (function.body != expected.body
                || function.requires != expected.requires
                || function.ensures != expected.ensures)
        {
            return Err(invalid(
                "semantic snapshot field-uniqueness HIR disagrees with canonical source",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "field_unique/tests.rs"]
mod tests;
