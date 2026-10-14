//! Authority-held typed reporting route. Original Vec APIs stay independently compiled.
use super::*;
use crate::workspace_report::WorkspaceReportFailure;
mod prepare;
use prepare::prepare_parsed_with_limit_typed as prepare_parsed_with_limit;

pub fn generate_evidence(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceOperationsEvidenceArtifacts, WorkspaceReportFailure> {
    generate_evidence_with_hook(root, proposal_path, |_| {})
}

pub fn evidence(root: &Path, proposal_path: &Path) -> Result<String, WorkspaceReportFailure> {
    generate_evidence(root, proposal_path)
        .map(SemanticWorkspaceOperationsEvidenceArtifacts::into_operations_evidence)
}

pub fn verify(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, WorkspaceReportFailure> {
    verify_with_hook(root, proposal_path, evidence_path, |_| {})
}

fn generate_evidence_with_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(OperationsEvidencePoint),
) -> Result<SemanticWorkspaceOperationsEvidenceArtifacts, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_operations_proposal(proposal_path).and_then(|source| {
        hook(OperationsEvidencePoint::ProposalOwned);
        parse_proposal(&source)
    });
    let (mut authority, proposal) = locked
        .authenticate_operations(proposal)
        .map_err(map_base_operations_builder_limit)?;
    let result = (|| {
        let storage = (
            authority.manifest_bytes(),
            authority.retained_generations(),
            authority.staging_attempts(),
        );
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources()?;
        let base = semantic_workspace::reporting::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        hook(OperationsEvidencePoint::AfterOperationsReplay);
        let derivation = render_derivation(&prepared)?;
        let prepared = prepared.into_evidence_input(derivation, storage)?;
        let change_artifacts =
            semantic_workspace_change::render_prepared_artifacts(&prepared.change)?;
        hook(OperationsEvidencePoint::ChangeArtifactsRendered);
        let artifacts = evidence_artifact::render_evidence(&prepared, &change_artifacts)?;
        hook(OperationsEvidencePoint::EvidenceRendered);
        Ok(artifacts)
    })();
    authority.finish_reporting(result)
}

fn verify_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(OperationsEvidencePoint),
) -> Result<String, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let input = read_operations_proposal(proposal_path).and_then(|proposal_source| {
        hook(OperationsEvidencePoint::ProposalOwned);
        let evidence_source = evidence_verification::read_evidence(evidence_path)?;
        let submitted = evidence_verification::parse_evidence(&evidence_source)?;
        hook(OperationsEvidencePoint::EvidenceOwned);
        let proposal = parse_proposal(&proposal_source)?;
        Ok((proposal, evidence_source, submitted))
    });
    let (mut authority, (proposal, evidence_source, submitted)) = locked
        .authenticate_operations(input)
        .map_err(map_base_operations_builder_limit)?;
    let result = (|| {
        let storage = (
            authority.manifest_bytes(),
            authority.retained_generations(),
            authority.staging_attempts(),
        );
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources()?;
        let base = semantic_workspace::reporting::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        hook(OperationsEvidencePoint::AfterOperationsReplay);
        let derivation = render_derivation(&prepared)?;
        let prepared = prepared.into_evidence_input(derivation, storage)?;
        let change_artifacts =
            semantic_workspace_change::render_prepared_artifacts(&prepared.change)?;
        hook(OperationsEvidencePoint::ChangeArtifactsRendered);
        let artifacts = evidence_artifact::render_evidence(&prepared, &change_artifacts)?;
        evidence_verification::verify_replay(
            &submitted,
            &evidence_source,
            artifacts.operations_evidence(),
            change_artifacts.evidence_bytes(),
        )?;
        let replay_token = evidence_artifact::exact_replay_token(
            &prepared,
            &change_artifacts,
            &artifacts,
            &evidence_source,
        )?;
        hook(OperationsEvidencePoint::OperationsEvidenceReplayed);
        let receipt =
            evidence_artifact::render_receipt(&prepared, &artifacts, &replay_token, false)?;
        hook(OperationsEvidencePoint::ReceiptRendered);
        Ok(receipt)
    })();
    authority.finish_reporting(result)
}

pub fn derive(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceOperationsDerivation, WorkspaceReportFailure> {
    derive_with_hook(root, proposal_path, |_| {})
}

/// Returns the exact derived Change proposal through the typed caller route.
pub fn derived_change_proposal(
    root: &Path,
    proposal_path: &Path,
) -> Result<String, WorkspaceReportFailure> {
    derive(root, proposal_path)
        .map(SemanticWorkspaceOperationsDerivation::into_derived_change_proposal)
}

/// Returns the canonical derivation through the typed caller route.
pub fn derivation(root: &Path, proposal_path: &Path) -> Result<String, WorkspaceReportFailure> {
    derive(root, proposal_path).map(SemanticWorkspaceOperationsDerivation::into_derivation)
}

pub(crate) fn derive_with_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(OperationsDerivePoint),
) -> Result<SemanticWorkspaceOperationsDerivation, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_operations_proposal(proposal_path).and_then(|source| {
        hook(OperationsDerivePoint::ProposalOwned);
        parse_proposal(&source)
    });
    let (mut authority, proposal) = locked
        .authenticate_operations(proposal)
        .map_err(map_base_operations_builder_limit)?;
    let result = (|| {
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources()?;
        let base = semantic_workspace::reporting::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        let derivation = render_derivation(&prepared)?;
        hook(OperationsDerivePoint::DerivationRendered);
        Ok(derivation)
    })();
    authority.finish_reporting(result)
}

pub fn apply(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, WorkspaceReportFailure> {
    apply_with_hook(root, proposal_path, evidence_path, |_, _, _, _| Ok(()))
}

pub(crate) fn apply_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(
        OperationsEvidencePoint,
        &Path,
        Option<&Path>,
        Option<&Path>,
    ) -> std::io::Result<()>,
) -> Result<String, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_apply_lock(root)?;
    let active_path = root.join(".semaprax-workspace/ACTIVE");
    let input = read_operations_proposal(proposal_path).and_then(|proposal_source| {
        hook(
            OperationsEvidencePoint::ProposalOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("proposal post-read hook failed", error))?;
        let evidence_source = evidence_verification::read_evidence(evidence_path)?;
        let submitted = evidence_verification::parse_evidence(&evidence_source)?;
        hook(
            OperationsEvidencePoint::EvidenceOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Evidence post-read hook failed", error))?;
        let proposal = parse_proposal(&proposal_source)?;
        Ok((proposal, evidence_source, submitted))
    });
    let (mut authority, (proposal, evidence_source, submitted)) = locked
        .authenticate_operations(input)
        .map_err(map_base_operations_builder_limit)?;
    let prepublication: Result<_, WorkspaceReportFailure> = (|| {
        let storage = (
            authority.manifest_bytes(),
            authority.retained_generations(),
            authority.staging_attempts(),
        );
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources()?;
        let base = semantic_workspace::reporting::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        hook(
            OperationsEvidencePoint::AfterOperationsReplay,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Operations replay hook failed", error))?;
        let derivation = render_derivation(&prepared)?;
        let prepared = prepared.into_evidence_input(derivation, storage)?;
        let change_artifacts =
            semantic_workspace_change::render_prepared_artifacts(&prepared.change)?;
        hook(
            OperationsEvidencePoint::ChangeArtifactsRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Change artifact hook failed", error))?;
        let artifacts = evidence_artifact::render_evidence(&prepared, &change_artifacts)?;
        evidence_verification::verify_replay(
            &submitted,
            &evidence_source,
            artifacts.operations_evidence(),
            change_artifacts.evidence_bytes(),
        )?;
        let replay_token = evidence_artifact::exact_replay_token(
            &prepared,
            &change_artifacts,
            &artifacts,
            &evidence_source,
        )?;
        hook(
            OperationsEvidencePoint::OperationsEvidenceReplayed,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Operations Evidence replay hook failed", error))?;
        let receipt =
            evidence_artifact::render_receipt(&prepared, &artifacts, &replay_token, true)?;
        hook(
            OperationsEvidencePoint::ReceiptRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("application receipt hook failed", error))?;
        Ok((prepared, receipt, replay_token))
    })();
    let (prepared, receipt, replay_token) = match prepublication {
        Ok(value) => value,
        Err(diagnostics) => return authority.finish_reporting(Err((diagnostics).into())),
    };
    let (candidate_files, candidate_manifest, candidate_revision) =
        prepared.change.into_operations_commit_parts();
    let commit = SemanticWorkspaceOperationsCommitAuthority {
        authority,
        candidate_files,
        candidate_manifest,
        candidate_revision,
        receipt,
        operations_proposal_digest: replay_token.operations_proposal_digest,
        derivation_digest: replay_token.derivation_digest,
        derived_change_proposal_digest: replay_token.derived_change_proposal_digest,
        workspace_change_evidence_digest: replay_token.workspace_change_evidence_digest,
        operations_evidence_digest: replay_token.operations_evidence_digest,
    };
    workspace::commit_semantic_operations_authority_with_hook(
        commit,
        |point, active, staged, candidate| {
            hook(
                OperationsEvidencePoint::Workspace(point),
                active,
                staged,
                candidate,
            )
        },
    )
    .map_err(Into::into)
}
