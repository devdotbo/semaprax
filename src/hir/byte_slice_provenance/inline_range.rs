//! Keep the exact inline Bytes view producer and owned-storage root.
use super::*;
/// Scope/availability and Own-mode admission are independently rechecked by
/// HIR validation. This projection re-derives the view operation and storage
/// type from the retained body/declaration inventory, never a Slice type alone.
pub(super) fn origin(
    source: &ResolvedExpr,
    roots: &BTreeMap<ValueId, ResolvedType>,
) -> Option<ByteSliceProvenance> {
    let ResolvedExprKind::BorrowPlace { operation, place } = &source.kind else {
        return None;
    };
    if operation.as_str() != crate::byte_ops::BYTES_AS_SLICE_ID
        || !place.projections.is_empty()
        || source.ty != ResolvedType::SliceU8
        || source.ownership != OwnershipMode::Borrow
        || roots.get(&place.root) != Some(&ResolvedType::Bytes)
    {
        return None;
    }
    Some(ByteSliceProvenance {
        vector_field: None,
        root: place.root.clone(),
        projections: Vec::new(),
        projected_type: ResolvedType::Bytes,
        root_kind: ByteSliceRootKind::OwnedBytes,
        root_length: ByteSliceExtent::ValueLength,
        offset: ByteSliceExtent::Constant(0),
        length: ByteSliceExtent::ValueLength,
        producer: Some(source.id.clone()),
        ranges: Vec::new(),
    })
}
