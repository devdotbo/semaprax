//! Paid inspection of a complete already-derived shape, with deferred semantic
//! text. Shape construction is a separate owner and is not credited here.
use super::*;
use crate::cleanup::{
    CleanupShapeProfile, FieldLivenessShape, BYTES_DROP_LIFECYCLE_ID, MAX_CLEANUP_OWNED_LEAVES,
    MAX_CLEANUP_SHAPE_DEPTH, MAX_CLEANUP_VISITED_FIELDS, STRING_DROP_LIFECYCLE_ID,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Leaves,
    Depth,
    Fields,
    DepthLimit,
    LeavesLimit,
    FieldsLimit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Inspected {
    pub(super) profile: CleanupShapeProfile,
    pub(super) has_variant: bool,
    failure: Option<Failure>,
}
impl Inspected {
    /// Call only at the old final profile-publication boundary. Earlier
    /// inventory validation retains its original first-error priority.
    pub(super) fn finish(self, payer: &mut Payer<'_>) -> Result<CleanupShapeProfile, Diagnostic> {
        inventory_text::current(payer)?;
        let Some(failure) = self.failure else {
            return Ok(self.profile);
        };
        let message = match failure {
            Failure::Leaves => {
                inventory_text::raw(payer, format_args!("cleanup owned-leaf count overflowed"))
            }
            Failure::Depth => {
                inventory_text::raw(payer, format_args!("cleanup record depth overflowed"))
            }
            Failure::Fields => inventory_text::raw(
                payer,
                format_args!("cleanup visited-field count overflowed"),
            ),
            Failure::DepthLimit => inventory_text::raw(
                payer,
                format_args!(
                    "cleanup shape exceeds the {MAX_CLEANUP_SHAPE_DEPTH} record-depth limit"
                ),
            ),
            Failure::LeavesLimit => inventory_text::raw(
                payer,
                format_args!(
                    "cleanup shape exceeds the {MAX_CLEANUP_OWNED_LEAVES} owned-leaf limit"
                ),
            ),
            Failure::FieldsLimit => inventory_text::raw(
                payer,
                format_args!(
                    "cleanup shape exceeds the {MAX_CLEANUP_VISITED_FIELDS} visited-field limit"
                ),
            ),
        }?;
        Err(Diagnostic::io("SPX-H006", message))
    }
}

pub(super) fn inspect(
    shape: &FieldLivenessShape,
    payer: &mut Payer<'_>,
) -> Result<Inspected, Diagnostic> {
    inventory_text::current(payer)?;
    let function = payer.function;
    payer
        .budget
        .charge(function, 1, "inventory shape profile root")?;
    let mut inspected = Inspected {
        profile: CleanupShapeProfile::default(),
        has_variant: false,
        failure: None,
    };
    let mut pending = Vec::new();
    payer.grow(&mut pending, 1)?;
    pending.push((shape, 0usize));
    while let Some((shape, record_depth)) = pending.pop() {
        payer
            .budget
            .charge(function, 1, "inventory shape profile frame")?;
        match shape {
            FieldLivenessShape::NoDrop => {}
            FieldLivenessShape::Leaf { lifecycle, .. } => {
                let Some(leaves) = inspected.profile.owned_leaves.checked_add(1) else {
                    inspected.failure = Some(Failure::Leaves);
                    return Ok(inspected);
                };
                inspected.profile.owned_leaves = leaves;
                // Preserve short-circuit order and count only comparisons
                // which the original nested-bytes predicate actually reaches.
                if record_depth >= 2 {
                    inspected.profile.has_nested_owned_bytes |= inventory_text::same_bytes(
                        lifecycle.as_str(),
                        BYTES_DROP_LIFECYCLE_ID,
                        payer,
                    )? || inventory_text::same_bytes(
                        lifecycle.as_str(),
                        STRING_DROP_LIFECYCLE_ID,
                        payer,
                    )?;
                }
            }
            FieldLivenessShape::Record { fields, .. } => {
                let Some(child_depth) = record_depth.checked_add(1) else {
                    inspected.failure = Some(Failure::Depth);
                    return Ok(inspected);
                };
                inspected.profile.maximum_record_depth =
                    inspected.profile.maximum_record_depth.max(child_depth);
                let Some(visited) = inspected.profile.visited_fields.checked_add(fields.len())
                else {
                    inspected.failure = Some(Failure::Fields);
                    return Ok(inspected);
                };
                inspected.profile.visited_fields = visited;
                payer.grow(&mut pending, fields.len())?;
                payer.budget.charge(
                    function,
                    fields.len(),
                    "inventory profile field scheduling",
                )?;
                pending.extend(fields.iter().rev().map(|field| (&field.shape, child_depth)));
            }
            FieldLivenessShape::Variant { cases, .. } => {
                inspected.has_variant = true;
                let mut field_count = 0usize;
                for case in cases {
                    payer
                        .budget
                        .charge(function, 1, "inventory profile case count")?;
                    let Some(next) = field_count.checked_add(case.fields.len()) else {
                        inspected.failure = Some(Failure::Fields);
                        return Ok(inspected);
                    };
                    field_count = next;
                }
                let Some(visited) = inspected.profile.visited_fields.checked_add(field_count)
                else {
                    inspected.failure = Some(Failure::Fields);
                    return Ok(inspected);
                };
                inspected.profile.visited_fields = visited;
                payer.grow(&mut pending, field_count)?;
                payer.budget.charge(
                    function,
                    field_count,
                    "inventory profile variant field scheduling",
                )?;
                for case in cases.iter().rev() {
                    payer
                        .budget
                        .charge(function, 1, "inventory profile case scheduling")?;
                    pending.extend(
                        case.fields
                            .iter()
                            .rev()
                            .map(|field| (&field.shape, record_depth)),
                    );
                }
            }
        }
    }
    // All three limits use the original nested-bytes condition and priority.
    // Store only a fixed discriminant; do not publish/allocate an error early.
    if inspected.profile.has_nested_owned_bytes {
        inspected.failure = if inspected.profile.maximum_record_depth > MAX_CLEANUP_SHAPE_DEPTH {
            Some(Failure::DepthLimit)
        } else if inspected.profile.owned_leaves > MAX_CLEANUP_OWNED_LEAVES {
            Some(Failure::LeavesLimit)
        } else if inspected.profile.visited_fields > MAX_CLEANUP_VISITED_FIELDS {
            Some(Failure::FieldsLimit)
        } else {
            None
        };
    }
    Ok(inspected)
}

#[cfg(test)]
mod tests;
