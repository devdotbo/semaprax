//! Authority-held typed reporting route. Original Vec APIs stay independently compiled.
use super::*;
use crate::workspace_report::WorkspaceReportFailure;
mod prepare;
use prepare::prepare_owned_with_limit_typed as prepare_owned_with_limit;

pub fn generate(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceStructuralChangeArtifacts, WorkspaceReportFailure> {
    generate_with_hook(root, proposal_path, |_| {})
}

pub fn preview(root: &Path, proposal_path: &Path) -> Result<String, WorkspaceReportFailure> {
    generate(root, proposal_path).map(SemanticWorkspaceStructuralChangeArtifacts::into_preview)
}

pub fn evidence(root: &Path, proposal_path: &Path) -> Result<String, WorkspaceReportFailure> {
    generate(root, proposal_path).map(SemanticWorkspaceStructuralChangeArtifacts::into_evidence)
}

pub fn verify(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, WorkspaceReportFailure> {
    verify_with_hook(root, proposal_path, evidence_path, |_| {})
}

pub(crate) fn generate_with_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(StructuralGeneratePoint),
) -> Result<artifact::SemanticWorkspaceStructuralChangeArtifacts, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_proposal(proposal_path).and_then(|source| {
        let change_set = parse_proposal(&source)?;
        hook(StructuralGeneratePoint::ProposalOwned);
        Ok(change_set)
    });
    let (authority, change_set) = locked
        .authenticate(proposal)
        .map_err(map_base_builder_limit)?;
    with_authenticated_structural_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        hook(StructuralGeneratePoint::ArtifactsRendered);
        Ok(artifacts)
    })
}

pub(crate) fn verify_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(StructuralVerifyPoint),
) -> Result<String, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(StructuralVerifyPoint::ProposalOwned);
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(StructuralVerifyPoint::EvidenceOwned);
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) =
        locked.authenticate(input).map_err(map_base_builder_limit)?;
    with_authenticated_structural_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        let receipt =
            artifact::render_verification_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(StructuralVerifyPoint::ReceiptRendered);
        Ok(receipt)
    })
}

fn with_authenticated_structural_authority<T>(
    authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceStructuralChangeSet,
    operation: impl FnOnce(
        SemanticWorkspacePreparedStructuralChange,
    ) -> Result<T, WorkspaceReportFailure>,
) -> Result<T, WorkspaceReportFailure> {
    let (authority, prepared) = prepare_authenticated_structural_authority(authority, change_set)?;
    let result = operation(prepared);
    authority.finish_reporting(result)
}

fn prepare_authenticated_structural_authority(
    mut authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceStructuralChangeSet,
) -> Result<
    (
        workspace::WorkspaceSemanticReadAuthority,
        SemanticWorkspacePreparedStructuralChange,
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
            Ok(()) => Err((replay()).into()),
        },
    }
}

pub(crate) fn prepare_owned(
    authenticated_revision: String,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    base_graph: workspace_graph::WorkspaceGraphBuild,
    storage: (usize, usize, usize),
    change_set: SemanticWorkspaceStructuralChangeSet,
) -> Result<SemanticWorkspacePreparedStructuralChange, WorkspaceReportFailure> {
    prepare_owned_with_limit(
        authenticated_revision,
        sources,
        base_graph,
        storage,
        change_set,
        MAX_ANALYSIS_BUILDER_BYTES,
    )
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
    mut hook: impl FnMut(
        StructuralApplyPoint,
        &Path,
        Option<&Path>,
        Option<&Path>,
    ) -> std::io::Result<()>,
) -> Result<String, WorkspaceReportFailure> {
    let locked = workspace::acquire_semantic_change_apply_lock(root)?;
    let active_path = root.join(".semaprax-workspace/ACTIVE");
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(
            StructuralApplyPoint::ProposalOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| apply_hook_error("proposal post-read hook failed", error))?;
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(
            StructuralApplyPoint::EvidenceOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| apply_hook_error("Evidence post-read hook failed", error))?;
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) =
        locked.authenticate(input).map_err(map_base_builder_limit)?;
    let (authority, prepared) = prepare_authenticated_structural_authority(authority, change_set)?;
    let prepublication: Result<_, WorkspaceReportFailure> = (|| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        hook(StructuralApplyPoint::AfterReplay, &active_path, None, None)
            .map_err(|error| apply_hook_error("exact replay hook failed", error))?;
        let receipt =
            artifact::render_application_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(
            StructuralApplyPoint::ReceiptRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| apply_hook_error("application receipt hook failed", error))?;
        Ok(receipt)
    })();
    let receipt = match prepublication {
        Ok(receipt) => receipt,
        Err(diagnostics) => return authority.finish_reporting(Err((diagnostics).into())),
    };
    let (candidate_files, candidate_manifest, candidate_revision) =
        prepared.into_candidate_generation_parts();
    let commit = SemanticWorkspaceStructuralChangeCommitAuthority {
        authority,
        candidate_files,
        candidate_manifest,
        candidate_revision,
        receipt,
    };
    workspace::commit_semantic_structural_change_authority_with_hook(
        commit,
        |point, active, staged, candidate| {
            hook(
                StructuralApplyPoint::Workspace(point),
                active,
                staged,
                candidate,
            )
        },
    )
    .map_err(Into::into)
}
