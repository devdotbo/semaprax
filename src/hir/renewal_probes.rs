//! Work receivers for exact immutable declaration lookups; no new allowance.
use super::DeclarationId;
use std::collections::BTreeMap;
pub(crate) type Work<'a> = dyn FnMut(usize) -> bool + 'a;
pub(crate) fn units(work: &mut Work<'_>, count: usize) -> Result<(), ()> {
    if work(count) {
        Ok(())
    } else {
        Err(())
    }
}
pub(crate) fn lookup<'a, V>(
    map: &'a BTreeMap<DeclarationId, V>,
    query: &DeclarationId,
    work: &mut Work<'_>,
) -> Result<Option<&'a V>, ()> {
    // Every possible search comparison is paid before the actual get. The
    // borrowed key census itself receives one unit before visiting each key.
    for key in map.keys() {
        units(work, 1)?;
        let bytes = key
            .as_str()
            .len()
            .min(query.as_str().len())
            .checked_add(1)
            .ok_or(())?;
        units(work, bytes)?;
    }
    Ok(map.get(query))
}

/// Exact borrowed typed receivers; a paid phase inventory avoids repeated key censuses.
pub(crate) trait Index {
    fn declaration(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&super::Declaration>, ()>;
    fn parameters(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<super::ResolvedTypeParameterDeclaration>>, ()>;
    fn fields(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<super::ResolvedFieldDeclaration>>, ()>;
}
impl Index for super::DeclarationIndex {
    fn declaration(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&super::Declaration>, ()> {
        lookup(&self.declarations, key, work)
    }
    fn parameters(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<super::ResolvedTypeParameterDeclaration>>, ()> {
        lookup(&self.type_parameters, key, work).map(|rows| rows.map(|values| &**values))
    }
    fn fields(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<super::ResolvedFieldDeclaration>>, ()> {
        lookup(&self.record_fields, key, work).map(|rows| rows.map(|values| &**values))
    }
}

pub(crate) mod phase;
