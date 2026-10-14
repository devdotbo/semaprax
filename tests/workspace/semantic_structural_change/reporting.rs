use super::*;

#[test]
fn typed_public_generation_verification_and_application_keep_full_legacy_bytes() {
    let fixture = Fixture::new("typed-report-parity");
    let old =
        semantic_workspace_structural_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let current =
        semantic_workspace_structural_change::reporting::generate(&fixture.root, &fixture.proposal)
            .unwrap();
    assert_eq!(old.evidence(), current.evidence());
    let evidence = fixture.root.join("typed-evidence.json");
    std::fs::write(&evidence, old.evidence()).unwrap();
    let old_receipt =
        semantic_workspace_structural_change::verify(&fixture.root, &fixture.proposal, &evidence)
            .unwrap();
    let current_receipt = semantic_workspace_structural_change::reporting::verify(
        &fixture.root,
        &fixture.proposal,
        &evidence,
    )
    .unwrap();
    assert_eq!(old_receipt, current_receipt);
    let receipt = semantic_workspace_structural_change::reporting::apply(
        &fixture.root,
        &fixture.proposal,
        &evidence,
    )
    .unwrap();
    let legacy_fixture = Fixture::new("typed-report-legacy-apply");
    let legacy_artifacts = semantic_workspace_structural_change::generate(
        &legacy_fixture.root,
        &legacy_fixture.proposal,
    )
    .unwrap();
    let legacy_evidence = legacy_fixture.root.join("typed-evidence.json");
    std::fs::write(&legacy_evidence, legacy_artifacts.evidence()).unwrap();
    let legacy_receipt = semantic_workspace_structural_change::apply(
        &legacy_fixture.root,
        &legacy_fixture.proposal,
        &legacy_evidence,
    )
    .unwrap();
    assert_eq!(receipt, legacy_receipt);
}

#[test]
fn typed_public_evidence_refusal_keeps_complete_original_diagnostics() {
    let fixture = Fixture::new("typed-report-negative");
    let evidence = fixture.root.join("invalid-evidence.json");
    std::fs::write(&evidence, "{}\n").unwrap();
    let old =
        semantic_workspace_structural_change::verify(&fixture.root, &fixture.proposal, &evidence)
            .unwrap_err();
    let current = semantic_workspace_structural_change::reporting::verify(
        &fixture.root,
        &fixture.proposal,
        &evidence,
    )
    .unwrap_err();
    assert_eq!(current.reported().unwrap(), old.as_slice());
}
