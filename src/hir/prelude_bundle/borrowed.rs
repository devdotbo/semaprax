//! Fixed-base borrowed views for existing infallible Index APIs. These methods
//! allocate nothing and grant no work budget; callers that already meter work
//! use the separate fallible overlay. Source verification remains independent.
use super::*;

impl BasePreludeBundle {
    pub(crate) fn declarations_slice(&self) -> &[Declaration] {
        self.declarations()
    }
    pub(crate) fn types_slice(&self) -> &[ResolvedTypeDeclaration] {
        self.types()
    }
    #[cfg(test)]
    pub(crate) fn declaration_by_text(&self, text: &str) -> Option<&Declaration> {
        self.declarations()
            .iter()
            .find(|entry| entry.id.as_str() == text)
    }
    #[cfg(test)]
    pub(crate) fn type_id(&self, name: &str) -> Option<&DeclarationId> {
        self.types()
            .iter()
            .find(|ty| ty.name == name)
            .map(|ty| &ty.id)
    }
    #[cfg(test)]
    pub(crate) fn type_parameters(
        &self,
        owner: &DeclarationId,
    ) -> Option<&[ResolvedTypeParameterDeclaration]> {
        self.types()
            .iter()
            .find(|ty| ty.id == *owner)
            .map(|ty| ty.type_parameters.as_slice())
    }
    #[cfg(test)]
    pub(crate) fn variant_cases(
        &self,
        owner: &DeclarationId,
    ) -> Option<&[ResolvedVariantCaseDeclaration]> {
        let ty = self.types().iter().find(|ty| ty.id == *owner)?;
        let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
            return None;
        };
        Some(cases.as_slice())
    }
    #[cfg(test)]
    pub(crate) fn case_fields(&self, owner: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        for ty in self.types() {
            let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
                continue;
            };
            if let Some(case) = cases.iter().find(|case| case.id == *owner) {
                return Some(&case.fields);
            }
        }
        None
    }
    #[cfg(test)]
    fn named_id(
        &self,
        owner: &DeclarationId,
        name: &str,
        kind: DeclarationKind,
    ) -> Option<&DeclarationId> {
        self.declarations()
            .iter()
            .find(|entry| {
                entry.kind == kind && entry.owner.as_ref() == Some(owner) && entry.name == name
            })
            .map(|entry| &entry.id)
    }
    #[cfg(test)]
    pub(crate) fn case_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.named_id(owner, name, DeclarationKind::VariantCase)
    }
    #[cfg(test)]
    pub(crate) fn field_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.named_id(owner, name, DeclarationKind::CaseField)
    }
}
