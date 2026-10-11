//! Place type and ownership resolution, relocated verbatim from the validator
//! root so the root stays within its recorded module size.
use super::*;

impl HirValidator<'_> {
    pub(super) fn resolve_place(
        &self,
        place: &Place,
        binding: &ValidationBinding,
    ) -> Result<(ResolvedType, OwnershipMode), Diagnostic> {
        let mut ty = binding.ty.clone();
        let mut ownership = binding.ownership;
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => {
                    ty = self.field_type_for_type(&ty, field)?;
                    ownership = self.expected_ownership(&ty, ownership)?;
                }
                PlaceProjection::VariantField { .. } => {
                    return Err(hir_error(
                        "variant-field projections are not valid before variant HIR lands",
                    ));
                }
            }
        }
        Ok((ty, ownership))
    }
}
