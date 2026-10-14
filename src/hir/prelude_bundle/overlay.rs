//! Borrowed lookup overlay; it creates no merged BTreeMap or hidden owned HIR.
use super::*;
use crate::hir::{DeclarationIndex, ResolvedVariantCaseDeclaration};
use std::collections::btree_map::Values;
use std::iter::Peekable;

pub(crate) struct Overlay<'a> {
    base: &'a BasePreludeBundle,
    local: &'a DeclarationIndex,
}
impl<'a> Overlay<'a> {
    pub(super) fn new(base: &'a BasePreludeBundle, local: &'a DeclarationIndex) -> Self {
        Self { base, local }
    }
    pub(crate) fn declaration<P: Payer>(
        &self,
        id: &DeclarationId,
        payer: &mut P,
    ) -> Result<Option<&'a Declaration>, Failure<P::Error>> {
        if let Some(found) = self.base.declaration(id.as_str(), payer)? {
            return Ok(Some(found));
        }
        for declaration in self.local.declarations.values() {
            compare(declaration.id.as_str(), id.as_str(), payer)?;
            if declaration.id == *id {
                return Ok(Some(declaration));
            }
        }
        Ok(None)
    }
    pub(crate) fn type_id<P: Payer>(
        &self,
        name: &str,
        payer: &mut P,
    ) -> Result<Option<&'a DeclarationId>, Failure<P::Error>> {
        for ty in self.base.types() {
            compare(&ty.name, name, payer)?;
            if ty.name == name {
                return Ok(Some(&ty.id));
            }
        }
        for (candidate, id) in &self.local.types_by_name {
            compare(candidate, name, payer)?;
            if candidate == name {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }
    fn named<P: Payer>(
        &self,
        owner: &DeclarationId,
        name: &str,
        kind: DeclarationKind,
        payer: &mut P,
    ) -> Result<Option<&'a DeclarationId>, Failure<P::Error>> {
        for declaration in self.base.declarations() {
            payer.work(1).map_err(Failure::Payer)?;
            if declaration.kind != kind {
                continue;
            }
            if let Some(candidate) = &declaration.owner {
                compare(candidate.as_str(), owner.as_str(), payer)?;
                compare(&declaration.name, name, payer)?;
                if candidate == owner && declaration.name == name {
                    return Ok(Some(&declaration.id));
                }
            }
        }
        let local = if kind == DeclarationKind::VariantCase {
            &self.local.cases_by_owner_name
        } else {
            &self.local.fields_by_owner_name
        };
        for ((candidate, candidate_name), id) in local {
            compare(candidate.as_str(), owner.as_str(), payer)?;
            compare(candidate_name, name, payer)?;
            if candidate == owner && candidate_name == name {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }
    pub(crate) fn case_id<P: Payer>(
        &self,
        owner: &DeclarationId,
        name: &str,
        payer: &mut P,
    ) -> Result<Option<&'a DeclarationId>, Failure<P::Error>> {
        self.named(owner, name, DeclarationKind::VariantCase, payer)
    }
    pub(crate) fn field_id<P: Payer>(
        &self,
        owner: &DeclarationId,
        name: &str,
        payer: &mut P,
    ) -> Result<Option<&'a DeclarationId>, Failure<P::Error>> {
        self.named(owner, name, DeclarationKind::CaseField, payer)
    }
    pub(crate) fn type_parameters<P: Payer>(
        &self,
        owner: &DeclarationId,
        payer: &mut P,
    ) -> Result<Option<&'a [ResolvedTypeParameterDeclaration]>, Failure<P::Error>> {
        for ty in self.base.types() {
            compare(ty.id.as_str(), owner.as_str(), payer)?;
            if ty.id == *owner {
                return Ok(Some(ty.type_parameters.as_slice()));
            }
        }
        for (id, parameters) in &self.local.type_parameters {
            compare(id.as_str(), owner.as_str(), payer)?;
            if id == owner {
                return Ok(Some(parameters.as_slice()));
            }
        }
        Ok(None)
    }
    pub(crate) fn variant_cases<P: Payer>(
        &self,
        owner: &DeclarationId,
        payer: &mut P,
    ) -> Result<Option<&'a [ResolvedVariantCaseDeclaration]>, Failure<P::Error>> {
        for ty in self.base.types() {
            compare(ty.id.as_str(), owner.as_str(), payer)?;
            if ty.id == *owner {
                let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
                    return Err(Failure::Schema);
                };
                return Ok(Some(cases.as_slice()));
            }
        }
        for (id, cases) in &self.local.variant_cases {
            compare(id.as_str(), owner.as_str(), payer)?;
            if id == owner {
                return Ok(Some(cases.as_slice()));
            }
        }
        Ok(None)
    }
    pub(crate) fn case_fields<P: Payer>(
        &self,
        owner: &DeclarationId,
        payer: &mut P,
    ) -> Result<Option<&'a [ResolvedFieldDeclaration]>, Failure<P::Error>> {
        for ty in self.base.types() {
            let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
                return Err(Failure::Schema);
            };
            for case in cases.iter() {
                compare(case.id.as_str(), owner.as_str(), payer)?;
                if case.id == *owner {
                    return Ok(Some(&case.fields));
                }
            }
        }
        // The local index keeps its own canonical coordinate validation. Charge
        // its complete key comparison bound before invoking that unchanged API.
        for id in self.local.case_fields.keys() {
            compare(id.as_str(), owner.as_str(), payer)?;
        }
        Ok(self.local.case_fields(owner))
    }
    pub(crate) fn declarations<'p, P: Payer>(&self, payer: &'p mut P) -> Declarations<'a, 'p, P> {
        Declarations {
            base: self.base.declarations(),
            local: self.local.declarations.values().peekable(),
            payer,
            failed: false,
        }
    }
}

fn compare<P: Payer>(left: &str, right: &str, payer: &mut P) -> Result<(), Failure<P::Error>> {
    payer
        .work(
            left.len()
                .checked_add(right.len())
                .ok_or(Failure::Layout)?
                .checked_add(1)
                .ok_or(Failure::Layout)?,
        )
        .map_err(Failure::Payer)
}

pub(crate) struct Declarations<'a, 'p, P: Payer> {
    base: &'a [Declaration],
    local: Peekable<Values<'a, DeclarationId, Declaration>>,
    payer: &'p mut P,
    failed: bool,
}
impl<'a, P: Payer> Iterator for Declarations<'a, '_, P> {
    type Item = Result<&'a Declaration, Failure<P::Error>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let base = self.base.first();
        let local = self.local.peek().copied();
        let result = match (base, local) {
            (None, None) => return None,
            (Some(base), Some(local)) => {
                if let Err(error) = compare(base.id.as_str(), local.id.as_str(), self.payer) {
                    self.failed = true;
                    return Some(Err(error));
                }
                match base.id.cmp(&local.id) {
                    std::cmp::Ordering::Greater => self.local.next().unwrap(),
                    std::cmp::Ordering::Equal => {
                        self.local.next();
                        self.base = &self.base[1..];
                        base
                    }
                    std::cmp::Ordering::Less => {
                        self.base = &self.base[1..];
                        base
                    }
                }
            }
            (Some(base), None) => {
                if let Err(error) = self.payer.work(1) {
                    self.failed = true;
                    return Some(Err(Failure::Payer(error)));
                }
                self.base = &self.base[1..];
                base
            }
            (None, Some(_)) => {
                if let Err(error) = self.payer.work(1) {
                    self.failed = true;
                    return Some(Err(Failure::Payer(error)));
                }
                self.local.next().unwrap()
            }
        };
        Some(Ok(result))
    }
}
