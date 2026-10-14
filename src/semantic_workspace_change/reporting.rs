//! Authority-held typed reporting route. Original Vec APIs stay independently compiled.
use super::*;
use crate::workspace_report::WorkspaceReportFailure;
mod prepare;
use prepare::prepare_owned_typed as prepare_owned;

pub fn generate(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceChangeArtifacts, WorkspaceReportFailure> {
    generate_with_operation_hook(root, proposal_path, |_| {})
}

pub fn preview(root: &Path, proposal_path: &Path) -> Result<String, WorkspaceReportFailure> {
    generate(root, proposal_path).map(artifact::SemanticWorkspaceChangeArtifacts::into_preview)
}

pub fn evidence(root: &Path, proposal_path: &Path) -> Result<String, WorkspaceReportFailure> {
    generate(root, proposal_path).map(artifact::SemanticWorkspaceChangeArtifacts::into_evidence)
}

pub fn verify(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, WorkspaceReportFailure> {
    verify_with_operation_hook(root, proposal_path, evidence_path, |_| {})
}

fn generate_with_operation_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(GeneratePoint),
) -> Result<SemanticWorkspaceChangeArtifacts, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_proposal(proposal_path).and_then(|source| parse_proposal(&source));
    if proposal.is_ok() {
        hook(GeneratePoint::ProposalOwned);
    }
    let (authority, change_set) = locked.authenticate(proposal)?;
    with_authenticated_change_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        hook(GeneratePoint::ArtifactsRendered);
        Ok(artifacts)
    })
}

fn verify_with_operation_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(VerifyPoint),
) -> Result<String, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(VerifyPoint::ProposalOwned);
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(VerifyPoint::EvidenceOwned);
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) = locked.authenticate(input)?;
    with_authenticated_change_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        let receipt =
            artifact::render_verification_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(VerifyPoint::ReceiptRendered);
        Ok(receipt)
    })
}

fn with_authenticated_change_authority<T>(
    authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceChangeSet,
    operation: impl FnOnce(SemanticWorkspacePreparedChange) -> Result<T, WorkspaceReportFailure>,
) -> Result<T, WorkspaceReportFailure> {
    let (authority, prepared) = prepare_authenticated_change_authority(authority, change_set)?;
    let result = operation(prepared);
    authority.finish_reporting(result)
}

fn prepare_authenticated_change_authority(
    mut authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceChangeSet,
) -> Result<
    (
        workspace::WorkspaceSemanticReadAuthority,
        SemanticWorkspacePreparedChange,
    ),
    WorkspaceReportFailure,
> {
    let base_workspace_revision = authority.workspace_revision().to_owned();
    let storage = (
        authority.manifest_bytes(),
        authority.retained_generations(),
        authority.staging_attempts(),
    );
    let result = (|| {
        let base_graph = authority.take_graph()?;
        let sources = authority.take_sources()?;
        prepare_owned(
            base_workspace_revision,
            sources,
            base_graph,
            storage,
            change_set,
        )
    })();
    match result {
        Ok(prepared) => Ok((authority, prepared)),
        Err(diagnostics) => match authority.finish_reporting::<()>(Err((diagnostics).into())) {
            Err(diagnostics) => Err((diagnostics).into()),
            Ok(()) => Err((replay(
                "Semantic Workspace Change failed preparation without diagnostics",
            ))
            .into()),
        },
    }
}

pub fn apply(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, WorkspaceReportFailure> {
    apply_authenticated_with_hook(root, proposal_path, evidence_path, |_, _, _, _| Ok(()))
}

pub(crate) fn apply_authenticated_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(SemanticApplyPoint, &Path, Option<&Path>, Option<&Path>) -> std::io::Result<()>,
) -> Result<String, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_apply_lock(root)?;
    let active_path = root.join(".semaprax-workspace/ACTIVE");
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(SemanticApplyPoint::ProposalOwned, &active_path, None, None)
            .map_err(|error| proposal_io_hook("proposal post-read hook failed", error))?;
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(SemanticApplyPoint::EvidenceOwned, &active_path, None, None)
            .map_err(|error| proposal_io_hook("Evidence post-read hook failed", error))?;
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) = locked.authenticate(input)?;
    let (authority, prepared) = prepare_authenticated_change_authority(authority, change_set)?;
    let prepublication: Result<_, WorkspaceReportFailure> = (|| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        hook(SemanticApplyPoint::AfterReplay, &active_path, None, None)
            .map_err(|error| proposal_io_hook("exact replay hook failed", error))?;
        let receipt =
            artifact::render_application_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(
            SemanticApplyPoint::ReceiptRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| proposal_io_hook("application receipt hook failed", error))?;
        Ok(receipt)
    })();
    let receipt = match prepublication {
        Ok(receipt) => receipt,
        Err(diagnostics) => return authority.finish_reporting(Err((diagnostics).into())),
    };
    let (candidate_files, candidate_manifest, candidate_revision) =
        prepared.into_candidate_generation_parts();
    let commit = SemanticWorkspaceChangeCommitAuthority {
        authority,
        candidate_files,
        candidate_manifest,
        candidate_revision,
        receipt,
    };
    workspace::commit_semantic_change_authority_with_hook(
        commit,
        None,
        |point, active, staged, candidate| {
            hook(
                SemanticApplyPoint::Workspace(point),
                active,
                staged,
                candidate,
            )
        },
    )
    .map_err(Into::into)
}
