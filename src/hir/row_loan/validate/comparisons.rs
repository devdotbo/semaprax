//! Variable identity comparisons debit the held replay Work before activity.
use super::*;
impl Work {
    pub(super) fn identities(&mut self, left: &str, right: &str) -> Result<(), Diagnostic> {
        self.units(left.len().min(right.len()).saturating_add(1))
    }
    pub(super) fn types(
        &mut self,
        left: &ResolvedType,
        right: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        // Count both immutable trees; their combined identifier payload covers
        // any prefix visited by the unchanged derived equality implementation.
        self.type_tree(left)?;
        self.type_tree(right)
    }
    fn type_tree(&mut self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        // Flat types need no scratch. Expanded types use the same paid growth
        // receiver, avoiding a new RowLoan-only depth limit or Rust recursion.
        self.units(2)?; // census plus unchanged equality traversal
        match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if arguments.is_empty() => {
                return self.units(declaration.as_str().len());
            }
            ResolvedType::TypeParameter { owner, .. } => return self.units(owner.as_str().len()),
            ResolvedType::Nominal { .. } | ResolvedType::Function { .. } => {}
            _ => return Ok(()),
        }
        let mut pending = new_vec()?;
        push(&mut pending, ty)?;
        while let Some(ty) = pending.pop() {
            match ty {
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    self.units(declaration.as_str().len())?;
                    for argument in arguments {
                        self.units(2)?;
                        push(&mut pending, argument)?;
                    }
                }
                ResolvedType::TypeParameter { owner, .. } => self.units(owner.as_str().len())?,
                ResolvedType::Function { parameters, result } => {
                    for parameter in parameters {
                        self.units(2)?;
                        push(&mut pending, parameter)?;
                    }
                    self.units(2)?;
                    push(&mut pending, result.as_ref())?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn places(&mut self, left: &Place, right: &Place) -> Result<(), Diagnostic> {
        self.identities(left.root.as_str(), right.root.as_str())?;
        for place in [left, right] {
            for projection in &place.projections {
                self.units(2)?; // census plus unchanged equality traversal
                match projection {
                    PlaceProjection::Field(field) => self.units(field.as_str().len())?,
                    PlaceProjection::VariantField { case, field } => {
                        self.units(case.as_str().len())?;
                        self.units(field.as_str().len())?;
                    }
                }
            }
        }
        Ok(())
    }
    fn keys<'a>(
        &mut self,
        keys: impl Iterator<Item = &'a DeclarationId>,
        query: &DeclarationId,
        copies: usize,
    ) -> Result<(), Diagnostic> {
        for key in keys {
            self.charge()?; // this prepayment census itself
            self.units(
                key.as_str()
                    .len()
                    .min(query.as_str().len())
                    .saturating_add(1)
                    .saturating_mul(copies),
            )?;
        }
        Ok(())
    }
    pub(super) fn layout(
        &mut self,
        index: &DeclarationIndex,
        ty: &ResolvedType,
        field_reader: bool,
    ) -> Result<(), Diagnostic> {
        self.charge()?;
        let ResolvedType::Nominal { declaration, .. } = ty else {
            return Ok(());
        };
        self.keys(index.declarations.keys(), declaration, 1)?;
        self.keys(index.type_parameters.keys(), declaration, 1)?;
        // One get below supplies borrowed census data, then the classifier get.
        self.keys(index.record_fields.keys(), declaration, 2)?;
        if let Some(fields) = index.record_fields.get(declaration) {
            for field in fields {
                self.charge()?;
                // field() repeats declaration lookup after layout().
                self.keys(
                    index.declarations.keys(),
                    &field.id,
                    if field_reader { 3 } else { 1 },
                )?;
                if field_reader {
                    self.units(field.id.as_str().len().saturating_add(1))?; // selected field equality
                    self.keys(index.declarations.keys(), &field.id, 1)?; // census get below
                    if let Some(fact) = index.declarations.get(&field.id) {
                        self.units(fact.name.len().min(field.name.len()).saturating_add(1))?;
                        if let Some(owner) = &fact.owner {
                            self.identities(owner.as_str(), declaration.as_str())?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn prefix_comparison(
        &mut self,
        index: &DeclarationIndex,
        comparison: crate::loan_plan::traversal::row_metered::Comparison<'_>,
    ) -> Result<(), Diagnostic> {
        use crate::loan_plan::traversal::row_metered::Comparison;
        match comparison {
            Comparison::Text(left, right) => self.identities(left, right),
            Comparison::Type(left, right) => self.types(left, right),
            Comparison::Value(left, right) => self.identities(left.as_str(), right.as_str()),
            Comparison::Binding(left, right) => {
                self.charge()?;
                self.identities(left.id.as_str(), right.id.as_str())?;
                self.identities(&left.name, &right.name)?;
                self.types(&left.ty, &right.ty)
            }
            Comparison::Element(element) => self.layout(index, element, false),
            Comparison::Visit => self.charge(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn type_and_projection_work_covers_census_and_unchanged_equality() {
        let mut flat = Work(0);
        flat.types(&ResolvedType::I64, &ResolvedType::I64).unwrap();
        assert_eq!(flat.0, 4); // two nodes, each census plus equality
        let ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("core.vec"),
            arguments: vec![ResolvedType::I64],
        };
        let mut expanded = Work(0);
        expanded.types(&ty, &ty).unwrap();
        assert_eq!(expanded.0, 2 * (2 + 8 + 2));
        let place = Place {
            root: ValueId::new("root".to_owned()),
            projections: vec![PlaceProjection::Field(DeclarationId::new("field"))],
        };
        let mut projections = Work(0);
        projections.places(&place, &place).unwrap();
        assert_eq!(projections.0, 5 + 2 * (2 + 5));
        let mut short = Work(MAX_ROW_LOAN_WORK - 3);
        assert!(short.types(&ResolvedType::I64, &ResolvedType::I64).is_err());
        assert_eq!(short.0, MAX_ROW_LOAN_WORK + 1);
        assert!(short.units(0).unwrap_err().message.is_empty());
    }
}
