//! Minimal selected replay declarations rebuilt from current authenticated
//! primitive functions. No generic/layout/provenance cache is carried forward.
use super::*;
use crate::cleanup_plan::ReplayPhysicalReservation;
impl DeclarationIndex {
    pub(crate) fn primitive_replay<'a>(
        functions: impl Iterator<Item = (&'a ResolvedFunction, IdentityOrigin)>,
        physical: &mut ReplayPhysicalReservation,
    ) -> Result<Self, crate::diagnostic::Diagnostic> {
        let fail = || {
            crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "primitive replay declaration reconstruction refused",
            )
        };
        let mut index = Self::default();
        let mut work = crate::cache_codec::MAX_NODES;
        for (function, origin) in functions {
            work = work
                .checked_sub(function.params.len().checked_add(1).ok_or_else(fail)?)
                .ok_or_else(fail)?;
            if index.declarations.contains_key(&function.id)
                || index.functions_by_name.contains_key(&function.name)
            {
                return Err(fail());
            }
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
