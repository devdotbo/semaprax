//! Storage custody for one actual resolved-core invocation. This owner grants
//! no source authority, allowance, cached verdict, or cross-attempt reuse.
use super::ByteHelperProofs;
use crate::ast::Program;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::diagnostic::Diagnostic;
use crate::loop_calls::byte_writer::SourceBacking;
use crate::workspace_graph::AuthoredDeclaration;
use std::collections::BTreeMap;
use std::mem::size_of;

fn error() -> Diagnostic {
    Diagnostic::io(
        "SPX-G171",
        "byte helper backing lost its exact core source or payer",
    )
}

pub(in crate::workspace_graph) struct CoreProviderProofs<'a> {
    pub(in crate::workspace_graph) base_prelude: crate::hir::prelude_bundle::Invocation<
        crate::bounded_output::identity_reservation::Refused,
    >,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    ledger: ClassifierLedger,
    backing: Option<SourceBacking<'a>>,
    carrier_charged: bool,
    poisoned: bool,
}
impl<'a> CoreProviderProofs<'a> {
    pub(in crate::workspace_graph) fn new(
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
    ) -> Result<Self, Diagnostic> {
        // Existing SourceProofs/ProviderProofs charges cover the proof payload
        // when acquired. Pay this complete owner plus the additional guard
        // control once. The empty owner still occupies its full header while
        // a phase is live; moving backing does not discount that header.
        let control = size_of::<Self>()
            .checked_add(size_of::<Phase<'_, 'a>>())
            .and_then(|n| n.checked_sub(size_of::<ByteHelperProofs<'a>>()))
            .and_then(|n| {
                n.checked_add(size_of::<
                    crate::hir::prelude_bundle::Invocation<
                        crate::bounded_output::identity_reservation::Refused,
                    >,
                >())
            })
            .ok_or_else(error)?;
        if crate::bounded_output::active_remaining().is_none()
            || !crate::bounded_output::reserve_active_required(0)
            || !crate::bounded_output::reserve_active_required(control)
        {
            return Err(error());
        }
        Ok(Self {
            base_prelude: crate::hir::prelude_bundle::Invocation::new(),
            authored,
            programs,
            ledger: ClassifierLedger::capture(),
            backing: Some(SourceBacking::empty()),
            carrier_charged: false,
            poisoned: false,
        })
    }
    pub(in crate::workspace_graph) fn bind_base_owner(&mut self) {
        let current = !self.poisoned && self.ledger.current();
        self.base_prelude.bind_owner(current);
        if !current {
            self.poisoned = true;
        }
    }
    fn bind(&mut self) -> Result<(), Diagnostic> {
        if self.poisoned
            || !self.ledger.current()
            || !crate::bounded_output::reserve_active_required(0)
        {
            self.poisoned = true;
            return Err(error());
        }
        Ok(())
    }
}

/// Standalone calls retain their fresh original proof scope. Only the actual
/// core path receives a bound owner, at the original local-proof position.
pub(in crate::workspace_graph::expected_projection) struct Phase<'o, 'a> {
    owner: Option<&'o mut CoreProviderProofs<'a>>,
    proofs: Option<ByteHelperProofs<'a>>,
    completed: bool,
}
impl<'o, 'a> Phase<'o, 'a> {
    pub(in crate::workspace_graph::expected_projection) fn new(
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
        owner: Option<&'o mut CoreProviderProofs<'a>>,
    ) -> Result<Self, Diagnostic> {
        let proofs = if let Some(core) = owner.as_deref() {
            if !std::ptr::eq(core.authored, authored) || !std::ptr::eq(core.programs, programs) {
                // The mutable branch below records poison on the same owner.
                None
            } else {
                Some(())
            }
        } else {
            Some(())
        };
        let mut owner = owner;
        if let Some(core) = owner.as_deref_mut() {
            core.bind()?;
            if proofs.is_none() {
                core.poisoned = true;
                return Err(error());
            }
            let Some(backing) = core.backing.take() else {
                core.poisoned = true;
                return Err(error());
            };
            let proofs = match ByteHelperProofs::from_backing(
                core.authored,
                core.programs,
                backing,
                core.carrier_charged,
            ) {
                Ok(proofs) => proofs,
                Err(error) => {
                    core.poisoned = true;
                    return Err(error);
                }
            };
            Ok(Self {
                owner,
                proofs: Some(proofs),
                completed: false,
            })
        } else {
            Ok(Self {
                owner: None,
                proofs: Some(ByteHelperProofs::new(authored, programs)),
                completed: false,
            })
        }
    }
    pub(in crate::workspace_graph::expected_projection) fn get(
        &mut self,
    ) -> Result<&mut ByteHelperProofs<'a>, Diagnostic> {
        if let Some(core) = self.owner.as_deref_mut() {
            core.bind()?;
        }
        self.proofs.as_mut().ok_or_else(error)
    }
    pub(in crate::workspace_graph::expected_projection) fn complete(
        &mut self,
    ) -> Result<(), Diagnostic> {
        if let Some(core) = self.owner.as_deref_mut() {
            core.bind()?;
        }
        self.completed = true;
        Ok(())
    }
}
impl Drop for Phase<'_, '_> {
    fn drop(&mut self) {
        let Some(core) = self.owner.as_deref_mut() else {
            return;
        };
        let Some(proofs) = self.proofs.take() else {
            core.poisoned = true;
            return;
        };
        let (backing, carrier_charged, failed) = proofs.into_parts();
        core.carrier_charged |= carrier_charged;
        core.poisoned |= failed || !self.completed;
        debug_assert!(core.backing.is_none());
        core.backing = Some(backing);
    }
}

#[cfg(test)]
#[path = "core_proofs/tests.rs"]
mod tests;
