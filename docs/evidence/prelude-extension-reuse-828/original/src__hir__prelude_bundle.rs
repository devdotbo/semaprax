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

#[path = "prelude_bundle/borrowed.rs"]
mod borrowed;
#[path = "prelude_bundle/fixed_work.rs"]
mod fixed_work;
#[path = "prelude_bundle/invocation.rs"]
mod invocation;
#[cfg(test)]
#[path = "prelude_bundle/materialize.rs"]
mod materialize;
#[cfg(test)]
#[path = "prelude_bundle/merge.rs"]
mod merge;
#[cfg(test)]
#[path = "prelude_bundle/overlay.rs"]
mod overlay;
#[path = "prelude_bundle/paid.rs"]
mod paid;
#[path = "prelude_bundle/producer.rs"]
mod producer;
#[path = "prelude_bundle/work_model.rs"]
mod work_model;
pub(crate) use fixed_work::{FixedFailure, FixedSchemaPayer, WorkReceipt, WorkRefusal};
#[cfg(test)]
pub(crate) use invocation::ProducerFailure;
pub(crate) use invocation::{Invocation, InvocationFailure};
#[cfg(test)]
pub(crate) use merge::merge_by_key;
#[cfg(test)]
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

#[derive(Debug, Eq, PartialEq)]
struct Contents {
    // Unique sorted IDs. All names/IDs and every payload remain actually owned.
    declarations: Vec<Declaration>,
    // Producer order is Option, Result, independently of declaration lookup order.
    types: Vec<ResolvedTypeDeclaration>,
}

/// A real owner, never a process cache. Modules may borrow a view or retain one
/// explicit lease. Only the producer can create the contents.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BasePreludeBundle {
    held: Arc<Contents>,
}
// Pure compatibility Clone for already-owned immutable index facts, matching
// DeclarationId/NominalPayload Clone: no new backing, ambient ledger or source
// authority. This does not pay a receiving Index carrier or authenticate a
// sticky account. Qualified acquisition paths use retain with their payer.
impl Clone for BasePreludeBundle {
    fn clone(&self) -> Self {
        Self {
            held: Arc::clone(&self.held),
        }
    }
}
impl BasePreludeBundle {
    pub(crate) fn acquire<P: Payer>(payer: &mut P) -> Result<Self, Failure<P::Error>> {
        // The initializer and receiving carriers coexist with the paid shared
        // owner. Acquire these actual layouts before constructing either.
        let carriers = std::mem::size_of::<Contents>()
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Result<Self, Failure<P::Error>>>()))
            .ok_or(Failure::Layout)?;
        payer.bytes(carriers).map_err(Failure::Payer)?;
        let contents = producer::contents(payer)?;
        payer
            .bytes(paid::shared_bytes::<Contents>())
            .map_err(Failure::Payer)?;
        Ok(Self {
            held: Arc::new(contents),
        })
    }

    #[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn overlay<'a>(&'a self, local: &'a crate::hir::DeclarationIndex) -> Overlay<'a> {
        Overlay::new(self, local)
    }

    #[cfg(test)]
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
#[path = "prelude_bundle/original.rs"]
mod original;
#[cfg(test)]
#[path = "prelude_bundle/tests.rs"]
mod tests;
