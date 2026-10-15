//! One paid current-index snapshot of borrowed entries, no declaration cloning.
use crate::cleanup_plan::renewal_profile::ordinary::{Admission, Refusal, Scratch};
use crate::hir::renewal_probes::{Index, Work};
use crate::hir::{DeclarationId, DeclarationIndex};
#[derive(Debug)]
pub(crate) struct Probes<'a> {
    declarations: Vec<(&'a DeclarationId, &'a crate::hir::Declaration)>,
    parameters: Vec<(
        &'a DeclarationId,
        &'a Vec<crate::hir::ResolvedTypeParameterDeclaration>,
    )>,
    fields: Vec<(
        &'a DeclarationId,
        &'a Vec<crate::hir::ResolvedFieldDeclaration>,
    )>,
}
fn collect<'a, V>(
    map: &'a std::collections::BTreeMap<DeclarationId, V>,
    admission: &mut Admission<'_>,
) -> Result<Vec<(&'a DeclarationId, &'a V)>, Refusal> {
    let mut entries = Scratch::new(admission)?;
    for (key, value) in map {
        admission.units(1)?;
        entries.push((key, value), admission)?;
    }
    Ok(entries.into_vec())
}
fn get<'a, V>(
    entries: &[(&'a DeclarationId, &'a V)],
    key: &DeclarationId,
    work: &mut Work<'_>,
) -> Result<Option<&'a V>, ()> {
    let (mut start, mut end) = (0, entries.len());
    while start < end {
        let mid = start + (end - start) / 2;
        let n = key
            .as_str()
            .len()
            .min(entries[mid].0.as_str().len())
            .checked_add(1)
            .ok_or(())?;
        crate::hir::renewal_probes::units(work, n)?;
        match entries[mid].0.cmp(key) {
            std::cmp::Ordering::Less => start = mid + 1,
            std::cmp::Ordering::Greater => end = mid,
            std::cmp::Ordering::Equal => return Ok(Some(entries[mid].1)),
        }
    }
    Ok(None)
}
impl<'a> Probes<'a> {
    pub(crate) fn new(
        index: &'a DeclarationIndex,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.reserve(std::mem::size_of::<Self>())?;
        Ok(Self {
            declarations: collect(&index.declarations, admission)?,
            parameters: collect(&index.type_parameters, admission)?,
            fields: collect(&index.record_fields, admission)?,
        })
    }
}
#[cfg(test)]
impl Probes<'_> {
    pub(crate) fn backing_bytes(&self) -> usize {
        self.declarations.capacity()
            * std::mem::size_of::<(&DeclarationId, &crate::hir::Declaration)>()
            + self.parameters.capacity()
                * std::mem::size_of::<(
                    &DeclarationId,
                    &Vec<crate::hir::ResolvedTypeParameterDeclaration>,
                )>()
            + self.fields.capacity()
                * std::mem::size_of::<(&DeclarationId, &Vec<crate::hir::ResolvedFieldDeclaration>)>(
                )
    }
}
impl Index for Probes<'_> {
    fn declaration(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&crate::hir::Declaration>, ()> {
        get(&self.declarations, key, work)
    }
    fn parameters(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<crate::hir::ResolvedTypeParameterDeclaration>>, ()> {
        get(&self.parameters, key, work)
    }
    fn fields(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<crate::hir::ResolvedFieldDeclaration>>, ()> {
        get(&self.fields, key, work)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn current_map_counts(index: &DeclarationIndex) -> [usize; 3] {
    [
        index.declarations.len(),
        index.type_parameters.len(),
        index.record_fields.len(),
    ]
}
