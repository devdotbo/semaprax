//! Exact inline owned-Bytes range origins, independent of cached Slice types.
use super::*;
impl HirValidator<'_> {
    pub(super) fn byte_range_origin<'b>(
        &self,
        source: &'b ResolvedExpr,
        scope: &BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<(&'b Place, Place), Diagnostic> {
        match &source.kind {
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                let origin = self
                    .byte_slice_aliases
                    .get(&place.root)
                    .cloned()
                    .ok_or_else(|| {
                        hir_error_at_span(
                            source.span,
                            "byte range source lacks authenticated slice provenance",
                        )
                    })?;
                Ok((place, origin))
            }
            ResolvedExprKind::BorrowPlace { operation, place }
                if operation.as_str() == crate::byte_ops::BYTES_AS_SLICE_ID
                    && place.projections.is_empty()
                    && source.ty == ResolvedType::SliceU8
                    && source.ownership == OwnershipMode::Borrow =>
            {
                let binding = scope.get(&place.root).ok_or_else(|| {
                    hir_error_at_span(source.span, "inline byte range owner is out of scope")
                })?;
                if binding.ty != ResolvedType::Bytes
                    || binding.ownership != OwnershipMode::Own
                    || Self::place_availability(binding, &[]) != Availability::Available
                {
                    return Err(hir_error_at_span(
                        source.span,
                        "inline byte range requires one available owned Bytes root",
                    ));
                }
                self.validate_byte_view_place(
                    crate::byte_ops::ByteOp::BytesAsSlice,
                    place,
                    source.span,
                    scope,
                )?;
                Ok((place, place.clone()))
            }
            _ => Err(hir_error_at_span(
                source.span,
                "byte range source must be a named slice or exact owned-Bytes view",
            )),
        }
    }
}

#[cfg(test)]
mod tests;
