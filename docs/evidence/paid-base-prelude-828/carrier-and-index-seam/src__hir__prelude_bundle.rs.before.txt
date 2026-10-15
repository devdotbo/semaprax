//! One paid immutable base-prelude owner, awaiting ordinary resolver integration.
//! Metadata is borrowed from the existing checked producer. This module grants
//! no source admission, cache authority, or right to remove the current forecast.
use crate::ast::Span;
use crate::hir::{
    Declaration, DeclarationId, DeclarationKind, IdentityOrigin, NominalPayload,
    ResolvedFieldDeclaration, ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
    ResolvedTypeParameterDeclaration, ResolvedVariantCaseDeclaration,
};
use crate::prelude::base_schema::{Variant, BASE};
use std::sync::Arc;

mod overlay;
mod paid;
mod producer;
pub(crate) use overlay::Overlay;

/// The existing invocation supplies both accounts and retains its first sticky
/// refusal. A zero-byte/work request must also observe an already failed account.
pub(crate) trait Payer {
    type Error;
    fn bytes(&mut self, requested: usize) -> Result<(), Self::Error>;
    fn work(&mut self, requested: usize) -> Result<(), Self::Error>;
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Failure<E> {
    Payer(E),
    Layout,
    Allocation,
    Capacity,
    Schema,
}

struct Contents {
    // Unique sorted IDs. All names/IDs and every payload remain actually owned.
    declarations: Vec<Declaration>,
    // Producer order is Option, Result, independently of declaration lookup order.
    types: Vec<ResolvedTypeDeclaration>,
}

/// A real owner, never a process cache. Modules may borrow a view or retain one
/// explicit lease. Only the producer can create the contents.
pub(crate) struct BasePreludeBundle {
    held: Arc<Contents>,
}
impl BasePreludeBundle {
    pub(crate) fn acquire<P: Payer>(payer: &mut P) -> Result<Self, Failure<P::Error>> {
        let contents = producer::contents(payer)?;
        payer
            .bytes(paid::shared_bytes::<Contents>())
            .map_err(Failure::Payer)?;
        Ok(Self {
            held: Arc::new(contents),
        })
    }

    pub(crate) fn retain<P: Payer>(&self, payer: &mut P) -> Result<Self, Failure<P::Error>> {
        // No new heap/control block: the original paid owner remains alive.
        payer.bytes(0).map_err(Failure::Payer)?;
        payer.work(1).map_err(Failure::Payer)?;
        Ok(Self {
            held: Arc::clone(&self.held),
        })
    }

    pub(crate) fn types(&self) -> &[ResolvedTypeDeclaration] {
        &self.held.types
    }

    pub(crate) fn declarations(&self) -> &[Declaration] {
        &self.held.declarations
    }

    pub(crate) fn overlay<'a>(&'a self, local: &'a crate::hir::DeclarationIndex) -> Overlay<'a> {
        Overlay::new(self, local)
    }

    pub(super) fn declaration<P: Payer>(
        &self,
        text: &str,
        payer: &mut P,
    ) -> Result<Option<&Declaration>, Failure<P::Error>> {
        for declaration in &self.held.declarations {
            payer
                .work(
                    text.len()
                        .checked_add(declaration.id.as_str().len())
                        .ok_or(Failure::Layout)?,
                )
                .map_err(Failure::Payer)?;
            match declaration.id.as_str().cmp(text) {
                std::cmp::Ordering::Equal => return Ok(Some(declaration)),
                std::cmp::Ordering::Greater => break,
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(None)
    }
}

impl Drop for Contents {
    fn drop(&mut self) {
        // The outer owner never recursively drops an unbounded declaration list.
        // Last shared nominal-payload owners use #827's iterative disposer; base
        // schema leaves are only TypeParameter, never a nested nominal graph.
        while let Some(ty) = self.types.pop() {
            drop(ty);
        }
        while let Some(declaration) = self.declarations.pop() {
            drop(declaration);
        }
    }
}

#[cfg(test)]
mod original;
#[cfg(test)]
mod tests;
