//! Exact retained declaration and owner identity checks.
use super::*;
pub(super) fn require_retained_shape_fact<'a>(
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    module: &WorkspaceResolvedModule,
    id: &'a str,
    kind: hir::DeclarationKind,
    owner: Option<&str>,
    seen: &mut BTreeSet<&'a str>,
) -> Result<(), Vec<Diagnostic>> {
    let fact = facts.get(id);
    if !fact.is_some_and(|fact| {
        fact.kind == kind
            && fact.owner.as_deref() == owner
            && fact.path.as_deref() == Some(module.path.as_str())
            && fact.module.as_deref() == Some(module.module.as_str())
    }) || !seen.insert(id)
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "retained workspace declaration shape disagrees with authored identity facts",
        )]);
    }
    Ok(())
}

/// A nonescaping text view of the exact root selected by the original walk.
/// Its empty result keeps the old clone's refused/floor behavior; it grants no
/// declaration or provider authority. Both shipping receivers keep their bodies.
pub(super) struct TopLevelDeclaration<'a>(&'a str);

impl TopLevelDeclaration<'_> {
    pub(super) fn as_str(&self) -> &str {
        self.0
    }
}

// Corresponding old result/receiving slots remain priced as DeclarationId.
// These are replacement roles, never a credit to any other allocation. No
// forecast, original text debit, frame tariff or cap is reduced.
const _: () = {
    use std::mem::{align_of, size_of};
    assert!(size_of::<TopLevelDeclaration<'_>>() <= size_of::<hir::DeclarationId>());
    assert!(align_of::<TopLevelDeclaration<'_>>() <= align_of::<hir::DeclarationId>());
    assert!(size_of::<&TopLevelDeclaration<'_>>() == size_of::<&hir::DeclarationId>());
};

pub(super) fn top_level_declaration<'a>(
    index: &'a hir::DeclarationIndex,
    declaration: &'a hir::Declaration,
) -> TopLevelDeclaration<'a> {
    let mut current = declaration;
    while let Some(owner) = &current.owner {
        let Some(parent) = index.declaration(owner) else {
            break;
        };
        current = parent;
    }
    TopLevelDeclaration(crate::bounded_output::borrowed_clone_view(
        current.id.as_str(),
    ))
}

#[cfg(test)]
pub(in crate::workspace_graph) mod before785;
#[cfg(test)]
mod before_caller785;
#[cfg(test)]
mod before_validation785;
#[cfg(test)]
mod source_controls;
#[cfg(test)]
mod tests;
