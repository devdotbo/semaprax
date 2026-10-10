//! Direct typed-HIR materializations that precede skeleton path construction.
use super::*;

pub(super) fn call_root(
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<CallSkeletonState>, Diagnostic> {
    work.budget
        .reserve_array::<CallSkeletonState>(work.function, 1, "initial call state slots")?;
    Ok(vec![(empty_expr_path(), Vec::new())])
}
pub(super) fn push_projection(
    place: &mut CleanupPlace,
    field: &DeclarationId,
    phase: &str,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    work.budget.reserve_vec(
        work.function,
        &mut place.projections,
        1,
        "owned projection slots",
    )?;
    let field = work.clone_owned(field, phase)?;
    place.projections.push(field);
    Ok(())
}
/// Retain the legacy uncharged observation's exact sequence and work count;
/// shared insertion reserves the actual RC payload before converting it.
pub(super) fn push_observation(
    observations: &mut Observations,
    observation: SkeletonObservation,
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
    if work.budget.physical.is_some() && observations.prefix.is_some() {
        return Err(replay_error(
            work.function,
            "selected lexical replay requires flat observations",
        ));
    }
    work.budget
        .reserve_vec(work.function, &mut observations.items, 1, phase)?;
    work.budget
        .reserve_bytes(work.function, physical_clone::observation_rc_bytes(), phase)?;
    observations.push(observation.into());
    Ok(())
}

impl ClonePayload for crate::hir::ResolvedParam {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.name.len().checked_add(self.ty.clone_payload_bytes()?)
    }
}
fn digits(mut index: usize) -> usize {
    let mut count = 1;
    while index >= 10 {
        index /= 10;
        count += 1;
    }
    count
}
fn parameter_id(
    operation: &str,
    index: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<crate::hir::ValueId, Diagnostic> {
    if work.budget.physical.is_none() {
        return Ok(crate::hir::ValueId::intrinsic_parameter(operation, index));
    }
    let len = operation
        .len()
        .checked_add(7)
        .and_then(|n| n.checked_add(digits(index)))
        .ok_or_else(|| replay_error(work.function, "intrinsic parameter identity overflow"))?;
    // A temporary exact String overlaps the new Arc<str> allocation. Round its
    // two-counter header + trailing text to usize alignment before allocation.
    let alignment = std::mem::align_of::<usize>();
    let arc = len
        .checked_add(2 * std::mem::size_of::<usize>())
        .and_then(|n| n.checked_add(alignment - 1))
        .map(|n| n / alignment * alignment);
    let bytes = arc.and_then(|n| n.checked_add(len)).unwrap_or(usize::MAX);
    work.budget
        .reserve_bytes(work.function, bytes, "intrinsic parameter identity")?;
    Ok(crate::hir::ValueId::intrinsic_parameter_exact(
        operation, index,
    ))
}
pub(super) fn static_params(
    operation: &str,
    types: &[ResolvedType],
    names: &[&str],
    ownership: impl Fn(usize, &ResolvedType) -> OwnershipMode,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    work.budget.reserve_array::<crate::hir::ResolvedParam>(
        work.function,
        types.len().min(names.len()),
        "intrinsic parameter slots",
    )?;
    let mut params = Vec::with_capacity(types.len().min(names.len()));
    for (index, (ty, name)) in types.iter().zip(names).enumerate() {
        let id = parameter_id(operation, index, work)?;
        work.budget
            .reserve_bytes(work.function, name.len(), "intrinsic parameter name")?;
        let ty = work.clone_payload(ty, "intrinsic parameter type")?;
        params.push(crate::hir::ResolvedParam {
            id,
            name: (*name).to_owned(),
            ownership: ownership(index, &ty),
            ty,
            span: crate::ast::Span::default(),
        });
    }
    Ok(params)
}

/// The first selected backing profile does not admit nominal/callable provider
/// construction. Refuse before catalog helpers can create uncharged types,
/// names or identities; ordinary replay retains the existing provider path.
pub(super) fn unsupported_provider(work: &mut SkeletonWork<'_, '_>) -> Result<(), Diagnostic> {
    if work.budget.physical.is_some() {
        Err(replay_error(
            work.function,
            "selected lexical replay parameter provider is outside the metered primitive profile",
        ))
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intrinsic_parameter_exact_preserves_unicode_and_decimal_identity_bytes() {
        for operation in ["core.bytes.zeroed", "unicode.é.控制"] {
            for index in [0usize, 9, 10, 999, usize::MAX] {
                assert_eq!(
                    crate::hir::ValueId::intrinsic_parameter(operation, index),
                    crate::hir::ValueId::intrinsic_parameter_exact(operation, index)
                );
            }
        }
    }
    #[test]
    fn parameter_clone_payload_counts_type_and_name_but_shares_value_identity() {
        let parameter = crate::hir::ResolvedParam {
            id: crate::hir::ValueId::intrinsic_parameter("core.bytes.zeroed", 0),
            name: "requested".to_owned(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::Usize,
            span: crate::ast::Span::default(),
        };
        assert_eq!(parameter.clone_payload_bytes(), Some("requested".len()));
        let params = vec![parameter.clone(), parameter];
        assert_eq!(
            params.clone_payload_bytes(),
            Some(2 * std::mem::size_of::<crate::hir::ResolvedParam>() + 2 * "requested".len())
        );
    }
}
