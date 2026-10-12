//! Borrowed alias discovery followed by one exact projection materialization.
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

use super::{charge, error, Diagnostic, Place, PlaceProjection, ValueId, WorkCounter};

pub(super) fn resolve(
    aliases: &BTreeMap<ValueId, Place>,
    place: Place,
    work: &mut WorkCounter,
) -> Result<Place, Diagnostic> {
    if let Some(payer) = work.allocation {
        payer.reserve(0).map_err(|_| super::storage::marker())?;
    }
    if crate::bounded_output::active_refused() {
        return Err(error(
            "loan alias projection exceeds its active allocation budget",
        ));
    }
    let mut current = &place.root;
    let mut seen = BTreeSet::new();
    let mut count = place.projections.len();
    let mut text_bytes = 0usize;
    while let Some(alias) = aliases.get(current) {
        charge(work)?;
        if !super::storage::set_insert(&mut seen, current, work.allocation)? {
            return Err(error("loan alias provenance contains a cycle"));
        }
        count = count
            .checked_add(alias.projections.len())
            .ok_or_else(|| error("loan alias projection allocation overflows"))?;
        for projection in &alias.projections {
            charge(work)?;
            let bytes = match projection {
                PlaceProjection::Field(field) => field.as_str().len(),
                PlaceProjection::VariantField { case, field } => case
                    .as_str()
                    .len()
                    .checked_add(field.as_str().len())
                    .ok_or_else(|| error("loan alias projection allocation overflows"))?,
            };
            text_bytes = text_bytes
                .checked_add(bytes)
                .ok_or_else(|| error("loan alias projection allocation overflows"))?;
        }
        current = &alias.root;
    }
    if seen.is_empty() {
        return Ok(place);
    }
    let root = current.clone();
    if count == 0 {
        return Ok(Place {
            root,
            projections: Vec::new(),
        });
    }
    // The existing construction envelope remains unchanged. This actual new
    // materialization is additionally prepaid on the same active ledger;
    // neither the earlier envelope nor dropped storage is refunded.
    let bytes = count
        .checked_mul(size_of::<PlaceProjection>())
        .and_then(|bytes| bytes.checked_add(size_of::<Vec<PlaceProjection>>()))
        .and_then(|bytes| bytes.checked_add(text_bytes))
        .ok_or_else(|| error("loan alias projection allocation overflows"))?;
    if let Some(payer) = work.allocation {
        payer.reserve(bytes).map_err(|_| super::storage::marker())?;
    } else if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(error(
            "loan alias projection exceeds its active allocation budget",
        ));
    }
    // Already prepaid above: retain that original conservative clone envelope,
    // including its directory/header terms, without a second array debit.
    let mut projections = Vec::new();
    if let Some(payer) = work.allocation {
        let requested = count
            .checked_mul(size_of::<PlaceProjection>())
            .ok_or_else(|| {
                payer.arithmetic();
                super::storage::marker()
            })?;
        if projections.try_reserve_exact(count).is_err() {
            payer.allocation(requested);
            return Err(super::storage::marker());
        }
        if projections.capacity() != count {
            let excess = projections
                .capacity()
                .saturating_sub(count)
                .checked_mul(size_of::<PlaceProjection>())
                .unwrap_or(usize::MAX);
            let _ = payer.capacity(requested, excess);
            return Err(super::storage::marker());
        }
    } else {
        projections = Vec::with_capacity(count);
    }
    // Prefixes encountered near the original root precede the original
    // projections after repeated prepending. Build the reverse once, then
    // reverse the complete vector without allocating or cloning it again.
    projections.extend(place.projections.into_iter().rev());
    current = &place.root;
    while let Some(alias) = aliases.get(current) {
        charge(work)?;
        projections.extend(alias.projections.iter().rev().cloned());
        current = &alias.root;
    }
    projections.reverse();
    Ok(Place { root, projections })
}

#[cfg(test)]
pub(super) fn original(
    aliases: &BTreeMap<ValueId, Place>,
    place: Place,
    work: &mut WorkCounter,
) -> Result<Place, Diagnostic> {
    original_resolve_origin(aliases, place, work)
}

#[cfg(test)]
fn original_resolve_origin(
    aliases: &BTreeMap<ValueId, Place>,
    mut place: Place,
    work: &mut WorkCounter,
) -> Result<Place, Diagnostic> {
    let mut seen = BTreeSet::new();
    while let Some(alias) = aliases.get(&place.root) {
        charge(work)?;
        if !seen.insert(place.root.clone()) {
            return Err(error("loan alias provenance contains a cycle"));
        }
        let mut projections = alias.projections.clone();
        projections.extend(place.projections);
        place = Place {
            root: alias.root.clone(),
            projections,
        };
    }
    Ok(place)
}

#[cfg(test)]
mod tests;
