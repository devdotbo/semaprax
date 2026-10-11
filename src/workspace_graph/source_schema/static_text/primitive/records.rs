//! Independent source layout/current namespace comparison before record index construction.
use super::*;
use crate::workspace_graph::static_text::primitive::records::Records;
#[cfg(test)]
pub(super) fn authenticate(
    module: &WorkspaceResolvedModule,
    source: Option<&Records>,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    index: &mut hir::DeclarationIndex,
    physical: &mut crate::cleanup_plan::ReplayPhysicalReservation,
) -> Result<(), Vec<Diagnostic>> {
    let expected = source.map_or(&[][..], |source| source.records.as_slice());
    if module.types.len() != expected.len() {
        return Err(fail());
    }
    let origin = |explicit| {
        if explicit {
            hir::IdentityOrigin::Explicit
        } else {
            hir::IdentityOrigin::Automatic
        }
    };
    for (source, current) in expected.iter().zip(&module.types) {
        if current != &source.declaration {
            return Err(fail());
        }
        let declaration = facts.get(current.id.as_str()).ok_or_else(fail)?;
        if declaration.kind != hir::DeclarationKind::Record
            || declaration.owner.is_some()
            || declaration.origin != origin(source.explicit)
            || declaration.path.as_deref() != Some(module.path.as_str())
            || declaration.module.as_deref() != Some(module.module.as_str())
        {
            return Err(fail());
        }
        let hir::ResolvedTypeDeclarationKind::Record { fields } = &current.kind else {
            return Err(fail());
        };
        if fields.len() != source.field_explicit.len() {
            return Err(fail());
        }
        for (field, explicit) in fields.iter().zip(&source.field_explicit) {
            let fact = facts.get(field.id.as_str()).ok_or_else(fail)?;
            if fact.kind != hir::DeclarationKind::Field
                || fact.owner.as_deref() != Some(current.id.as_str())
                || fact.origin != origin(*explicit)
                || fact.path.as_deref() != Some(module.path.as_str())
                || fact.module.as_deref() != Some(module.module.as_str())
            {
                return Err(fail());
            }
        }
        index
            .append_flat_record_replay(
                current,
                origin(source.explicit),
                |ordinal| source.field_explicit.get(ordinal).copied().map(origin),
                physical,
            )
            .map_err(|e| vec![e])?;
    }
    Ok(())
}

pub(super) mod prepared;
pub(super) use prepared::authenticate as authenticate_prepared;

#[cfg(test)]
mod tests;
