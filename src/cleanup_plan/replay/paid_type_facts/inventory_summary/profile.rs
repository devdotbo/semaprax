//! Streaming preorder projection of the complete independent shape inspector.
//! Semantic profile failures are retained until the old finished-root barrier.
use super::*;
use crate::cleanup::{
    CleanupShapeProfile, MAX_CLEANUP_OWNED_LEAVES, MAX_CLEANUP_SHAPE_DEPTH,
    MAX_CLEANUP_VISITED_FIELDS,
};
#[derive(Clone, Copy)]
enum Failure {
    Leaves,
    Depth,
    Fields,
}
#[derive(Default)]
pub(super) struct Tracker {
    profile: CleanupShapeProfile,
    failure: Option<Failure>,
    pub(super) variant: bool,
}
impl Tracker {
    pub(super) fn record(
        &mut self,
        depth: usize,
        fields: usize,
        payer: &mut Payer<'_>,
    ) -> Result<usize, Diagnostic> {
        metadata::step(payer, 1)?;
        let Some(next) = depth.checked_add(1) else {
            self.failure.get_or_insert(Failure::Depth);
            return Ok(depth);
        };
        if self.failure.is_none() {
            self.profile.maximum_record_depth = self.profile.maximum_record_depth.max(next);
            self.fields(fields);
        }
        Ok(next)
    }
    fn fields(&mut self, count: usize) {
        if self.failure.is_none() {
            match self.profile.visited_fields.checked_add(count) {
                Some(next) => self.profile.visited_fields = next,
                None => self.failure = Some(Failure::Fields),
            }
        }
    }
    pub(super) fn variant(
        &mut self,
        cases: &[crate::hir::ResolvedVariantCaseDeclaration],
        payer: &mut Payer<'_>,
    ) -> Result<(), Diagnostic> {
        self.variant_counts(cases.iter().map(|case| case.fields.len()), payer)
    }
    fn variant_counts(
        &mut self,
        counts: impl Iterator<Item = usize>,
        payer: &mut Payer<'_>,
    ) -> Result<(), Diagnostic> {
        metadata::step(payer, 1)?;
        self.variant = true;
        let mut count = 0usize;
        for fields in counts {
            metadata::step(payer, 1)?;
            if let Some(next) = count.checked_add(fields) {
                count = next;
            } else {
                self.failure.get_or_insert(Failure::Fields);
                return Ok(());
            }
        }
        self.fields(count);
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn variant_shape(
        &mut self,
        cases: &[crate::cleanup::VariantCaseLiveness],
        payer: &mut Payer<'_>,
    ) -> Result<(), Diagnostic> {
        self.variant_counts(cases.iter().map(|case| case.fields.len()), payer)
    }
    pub(super) fn leaf(
        &mut self,
        id: &str,
        depth: usize,
        payer: &mut Payer<'_>,
    ) -> Result<(), Diagnostic> {
        metadata::step(payer, 1)?;
        if self.failure.is_some() {
            return Ok(());
        }
        let Some(next) = self.profile.owned_leaves.checked_add(1) else {
            self.failure = Some(Failure::Leaves);
            return Ok(());
        };
        self.profile.owned_leaves = next;
        if depth >= 2 {
            self.profile.has_nested_owned_bytes |=
                inventory_text::same_bytes(id, crate::cleanup::BYTES_DROP_LIFECYCLE_ID, payer)?
                    || inventory_text::same_bytes(
                        id,
                        crate::cleanup::STRING_DROP_LIFECYCLE_ID,
                        payer,
                    )?;
        }
        Ok(())
    }
    pub(super) fn finish(
        self,
        payer: &mut Payer<'_>,
    ) -> Result<(CleanupShapeProfile, bool), Diagnostic> {
        metadata::step(payer, 1)?;
        if let Some(failure) = self.failure {
            return Err(error(match failure {
                Failure::Leaves => "cleanup owned-leaf count overflowed",
                Failure::Depth => "cleanup record depth overflowed",
                Failure::Fields => "cleanup visited-field count overflowed",
            }));
        }
        if self.profile.has_nested_owned_bytes {
            let message = if self.profile.maximum_record_depth > MAX_CLEANUP_SHAPE_DEPTH {
                Some(inventory_text::raw(
                    payer,
                    format_args!(
                        "cleanup shape exceeds the {MAX_CLEANUP_SHAPE_DEPTH} record-depth limit"
                    ),
                )?)
            } else if self.profile.owned_leaves > MAX_CLEANUP_OWNED_LEAVES {
                Some(inventory_text::raw(
                    payer,
                    format_args!(
                        "cleanup shape exceeds the {MAX_CLEANUP_OWNED_LEAVES} owned-leaf limit"
                    ),
                )?)
            } else if self.profile.visited_fields > MAX_CLEANUP_VISITED_FIELDS {
                Some(inventory_text::raw(payer,format_args!("cleanup shape exceeds the {MAX_CLEANUP_VISITED_FIELDS} visited-field limit"))?)
            } else {
                None
            };
            if let Some(message) = message {
                return Err(Diagnostic::io("SPX-H006", message));
            }
        }
        Ok((self.profile, self.variant))
    }
}
