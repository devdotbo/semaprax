//! Agent declaration lowering with caller-supplied identity construction.
//!
//! Every agent, type-role, operation and model-wait helper identity is asked
//! of the caller's callback with the borrowed source spelling, in the original
//! left-to-right field order, so no temporary `String` precedes the shared
//! backing. The callback decides how that backing is paid for; it grants no
//! admission authority and a callback error is returned unchanged.

use crate::ast::{AgentDeclaration, AgentOperationKind, AgentOperationRole, AgentTypeRole};
use crate::diagnostic::Diagnostic;

use super::super::{
    DeclarationId, ResolvedAgentDeclaration, ResolvedAgentModelWaitBinding,
    ResolvedAgentOperationKind, ResolvedAgentOperationRole, ResolvedAgentOperationRoleKind,
    ResolvedAgentTypeRole, ResolvedAgentTypeRoleKind,
};

/// Resolver outcome with the caller's identity error kept distinct from the
/// ordinary semantic diagnostic.
#[derive(Debug)]
pub(in crate::hir) enum AgentIdentityFailure<E> {
    Semantic(Diagnostic),
    Identity(E),
}

impl<E> From<Diagnostic> for AgentIdentityFailure<E> {
    fn from(error: Diagnostic) -> Self {
        Self::Semantic(error)
    }
}

pub(in crate::hir) fn resolve_agent_declaration_with<E>(
    declaration: &AgentDeclaration,
    module: &str,
    mut identity: impl FnMut(&str) -> Result<DeclarationId, E>,
) -> Result<ResolvedAgentDeclaration, E> {
    let stable_id = identity(declaration.stable_id.as_str())?;
    let name = declaration.name.clone();
    let mut types = Vec::with_capacity(declaration.types.len());
    for role in &declaration.types {
        types.push(ResolvedAgentTypeRole {
            role: type_role(&role.role),
            stable_id: identity(role.stable_id.as_str())?,
        });
    }
    let mut operations = Vec::with_capacity(declaration.operations.len());
    for operation in &declaration.operations {
        operations.push(ResolvedAgentOperationRole {
            role: operation_role(&operation.role),
            kind: operation_kind(&operation.kind),
            stable_id: identity(operation.stable_id.as_str())?,
            embedded: operation.embedded_function_index.is_some(),
        });
    }
    let runtime_v1_json = declaration.runtime_v1_json.clone();
    let model_wait = declaration
        .model_wait
        .as_ref()
        .map(|binding| {
            identity(binding.helper_id.as_str())
                .map(|helper_id| Box::new(ResolvedAgentModelWaitBinding { helper_id }))
        })
        .transpose()?;
    Ok(ResolvedAgentDeclaration {
        stable_id,
        name,
        types,
        operations,
        source_association: None,
        runtime_v1_json,
        model_wait,
    }
    .bind_source_association(module))
}

fn type_role(role: &AgentTypeRole) -> ResolvedAgentTypeRoleKind {
    match role {
        AgentTypeRole::Task => ResolvedAgentTypeRoleKind::Task,
        AgentTypeRole::State => ResolvedAgentTypeRoleKind::State,
        AgentTypeRole::Observation => ResolvedAgentTypeRoleKind::Observation,
        AgentTypeRole::Proposal => ResolvedAgentTypeRoleKind::Proposal,
        AgentTypeRole::Outcome => ResolvedAgentTypeRoleKind::Outcome,
        AgentTypeRole::Result => ResolvedAgentTypeRoleKind::Result,
    }
}

fn operation_role(role: &AgentOperationRole) -> ResolvedAgentOperationRoleKind {
    match role {
        AgentOperationRole::Initialize => ResolvedAgentOperationRoleKind::Initialize,
        AgentOperationRole::Observe => ResolvedAgentOperationRoleKind::Observe,
        AgentOperationRole::Propose => ResolvedAgentOperationRoleKind::Propose,
        AgentOperationRole::Authorize => ResolvedAgentOperationRoleKind::Authorize,
        AgentOperationRole::Execute => ResolvedAgentOperationRoleKind::Execute,
        AgentOperationRole::Reduce => ResolvedAgentOperationRoleKind::Reduce,
    }
}

fn operation_kind(kind: &AgentOperationKind) -> ResolvedAgentOperationKind {
    match kind {
        AgentOperationKind::Deterministic => ResolvedAgentOperationKind::Deterministic,
        AgentOperationKind::Model => ResolvedAgentOperationKind::Model,
        AgentOperationKind::Effect => ResolvedAgentOperationKind::Effect,
    }
}

#[cfg(test)]
pub(in crate::hir) mod fixture;

#[cfg(test)]
mod tests;
