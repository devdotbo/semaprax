//! Minimal selected replay declarations rebuilt from current authenticated
//! primitive functions. No generic/layout/provenance cache is carried forward.
use super::*;
mod flat_record;
mod helper_signature;
#[cfg(test)]
mod option_u8;
mod vector;
use crate::cleanup_plan::ReplayPhysicalReservation;
// Selected constructors refuse with the Copy literal; no Diagnostic or String
// is allocated on refusal. Only test wrappers render it explicitly.
use crate::cleanup_plan::SelectedReplayRefused as Refused;
impl DeclarationIndex {
    #[cfg(test)]
    pub(crate) fn primitive_replay<'a>(
        functions: impl Iterator<Item = (&'a ResolvedFunction, IdentityOrigin)>,
        physical: &mut ReplayPhysicalReservation,
    ) -> Result<Self, crate::diagnostic::Diagnostic> {
        Self::primitive_replay_metered(functions, physical, &mut |_| Ok(()))
            .map_err(Refused::into_legacy_diagnostic)
    }
    pub(crate) fn primitive_replay_metered<'a>(
        functions: impl Iterator<Item = (&'a ResolvedFunction, IdentityOrigin)>,
        physical: &mut ReplayPhysicalReservation,
        debit: &mut dyn FnMut(usize) -> Result<(), Refused>,
    ) -> Result<Self, Refused> {
        let fail = || Refused::constructor("primitive replay declaration reconstruction refused");
        let mut index = Self::default();
        let mut work = crate::cache_codec::MAX_NODES;
        for (function, origin) in functions {
            work = work
                .checked_sub(function.params.len().checked_add(1).ok_or_else(fail)?)
                .ok_or_else(fail)?;
            // Scan exact retained keys before the tree operations. The second
            // byte debit covers their search/insertion comparisons (each key
            // can be compared at most once by each tree operation).
            for key in index.declarations.keys() {
                debit(
                    key.as_str()
                        .len()
                        .checked_add(function.id.as_str().len())
                        .and_then(|n| n.checked_mul(3))
                        .ok_or_else(fail)?,
                )?;
                if key == &function.id {
                    return Err(fail());
                }
            }
            for key in index.functions_by_name.keys() {
                debit(
                    key.len()
                        .checked_add(function.name.len())
                        .and_then(|n| n.checked_mul(3))
                        .ok_or_else(fail)?,
                )?;
                if key == &function.name {
                    return Err(fail());
                }
            }
            debit(
                function
                    .id
                    .as_str()
                    .len()
                    .checked_mul(4)
                    .and_then(|n| {
                        function
                            .name
                            .len()
                            .checked_mul(2)
                            .and_then(|m| n.checked_add(m))
                    })
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            let node = crate::cleanup_plan::replay_btree_node_upper::<DeclarationId, Declaration>()
                .ok_or_else(fail)?;
            let names = crate::cleanup_plan::replay_btree_node_upper::<String, DeclarationId>()
                .ok_or_else(fail)?;
            let strings = function
                .id
                .as_str()
                .len()
                .checked_mul(3)
                .and_then(|n| {
                    function
                        .name
                        .len()
                        .checked_mul(2)
                        .and_then(|m| n.checked_add(m))
                })
                .ok_or_else(fail)?;
            physical
                .reserve(
                    node.checked_add(names)
                        .and_then(|n| n.checked_add(strings))
                        .ok_or_else(fail)?,
                )
                .map_err(|_| fail())?;
            // Parameter/root identities must belong to this exact source owner.
            physical
                .reserve(function.id.as_str().len())
                .map_err(|_| fail())?;
            debit(
                function
                    .result_id
                    .as_str()
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(function.body.id.as_str().len()))
                    .and_then(|n| n.checked_add(function.id.as_str().len().checked_mul(2)?))
                    .ok_or_else(fail)?,
            )?;
            for parameter in &function.params {
                debit(
                    parameter
                        .id
                        .as_str()
                        .len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(function.id.as_str().len()))
                        .and_then(|n| n.checked_add(32))
                        .ok_or_else(fail)?,
                )?;
            }
            let owner = FunctionExecutionId::Monomorphic(function.id.clone());
            if !function.result_id.matches_result(&owner)
                || !function.body.id.matches(&owner, "body")
                || function
                    .params
                    .iter()
                    .enumerate()
                    .any(|(position, parameter)| !parameter.id.matches_parameter(&owner, position))
            {
                return Err(fail());
            }
            index.declarations.insert(
                function.id.clone(),
                Declaration {
                    id: function.id.clone(),
                    name: function.name.clone(),
                    kind: DeclarationKind::Function,
                    identity_origin: origin,
                    owner: None,
                },
            );
            index
                .functions_by_name
                .insert(function.name.clone(), function.id.clone());
        }
        Ok(index)
    }
}

impl DeclarationIndex {
    /// Imported declarations are identified by persistent ID; their original
    /// names never enter the caller's authored alias namespace.
    pub(crate) fn append_scalar_helper_replay_metered(
        &mut self,
        function: &ResolvedFunction,
        physical: &mut ReplayPhysicalReservation,
        debit: &mut dyn FnMut(usize) -> Result<(), Refused>,
    ) -> Result<(), Refused> {
        let fail = || Refused::constructor("scalar helper declaration reconstruction refused");
        for key in self.declarations.keys() {
            debit(
                key.as_str()
                    .len()
                    .checked_add(function.id.as_str().len())
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(fail)?,
            )?;
            if key == &function.id {
                return Err(fail());
            }
        }
        debit(function.params.len().checked_add(1).ok_or_else(fail)?)?;
        if !helper_signature::admitted(self, function, debit)? {
            return Err(fail());
        }
        let node = crate::cleanup_plan::replay_btree_node_upper::<DeclarationId, Declaration>()
            .ok_or_else(fail)?;
        physical
            .reserve(
                node.checked_add(function.id.as_str().len().checked_mul(3).ok_or_else(fail)?)
                    .and_then(|n| n.checked_add(function.name.len()))
                    .ok_or_else(fail)?,
            )
            .map_err(|_| fail())?;
        debit(
            function
                .id
                .as_str()
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(function.name.len()))
                .and_then(|n| n.checked_add(function.result_id.as_str().len().checked_mul(2)?))
                .and_then(|n| n.checked_add(function.body.id.as_str().len()))
                .ok_or_else(fail)?,
        )?;
        for parameter in &function.params {
            debit(
                parameter
                    .id
                    .as_str()
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(function.id.as_str().len()))
                    .and_then(|n| n.checked_add(32))
                    .ok_or_else(fail)?,
            )?;
        }
        let owner = FunctionExecutionId::Monomorphic(function.id.clone());
        if !function.result_id.matches_result(&owner)
            || !function.body.id.matches(&owner, "body")
            || function
                .params
                .iter()
                .enumerate()
                .any(|(i, p)| !p.id.matches_parameter(&owner, i))
        {
            return Err(fail());
        }
        self.declarations.insert(
            function.id.clone(),
            Declaration {
                id: function.id.clone(),
                name: function.name.clone(),
                kind: DeclarationKind::Function,
                identity_origin: IdentityOrigin::Explicit,
                owner: None,
            },
        );
        Ok(())
    }
}
