//! Saved payload-copy handoff and actual cold/warm source-backed cache controls.
use super::*;
use crate::project::{ProjectFrontendCache, ProjectFrontendSource, ProjectManifest};

fn saved_clone(sources: &[SemanticWorkspaceSource]) -> Vec<SemanticWorkspaceSource> {
    sources
        .iter()
        .map(|source| SemanticWorkspaceSource {
            path: source.path.clone(),
            source: source.source.clone(),
        })
        .collect()
}
#[test]
fn owned_source_handoff_keeps_exact_order_pointers_and_removes_saved_clone_payload_through_drop() {
    const A: &str = "module handoff.a; @id(\"handoff.a.main\") fn main()->i64 {7}";
    const B: &str = "module handoff.b; @id(\"handoff.b.main\") fn main()->i64 {4}";
    let sources = vec![
        SemanticWorkspaceSource {
            path: "z.spx".to_owned(),
            source: A.to_owned(),
        },
        SemanticWorkspaceSource {
            path: "a.spx".to_owned(),
            source: B.to_owned(),
        },
    ];
    let pointers = [
        (sources[0].path.as_ptr(), sources[0].source.as_ptr()),
        (sources[1].path.as_ptr(), sources[1].source.as_ptr()),
    ];
    let directory = sources.as_ptr();
    // Independent old request census: two inline carriers and four text copies.
    let expected = 2 * std::mem::size_of::<SemanticWorkspaceSource>()
        + "z.spx".len()
        + A.len()
        + "a.spx".len()
        + B.len();
    let (_, old) = crate::test_allocation_observer::measure(|| {
        let copied = saved_clone(&sources);
        assert_eq!(copied[0].path, "z.spx");
        assert_eq!(copied[1].path, "a.spx");
        assert_eq!(copied[0].source, A);
        assert_eq!(copied[1].source, B);
        assert_ne!(copied[0].source.as_ptr(), sources[0].source.as_ptr());
        drop(copied);
    });
    assert_eq!(old.allocated, expected);
    assert_eq!(old.freed, expected);
    let (_, actual) = crate::test_allocation_observer::measure(move || {
        let moved = consume(sources);
        assert_eq!(moved.as_ptr(), directory);
        for (source, (path, text)) in moved.iter().zip(pointers) {
            assert_eq!(source.path.as_ptr(), path);
            assert_eq!(source.source.as_ptr(), text);
        }
        assert_eq!(moved[0].path, "z.spx");
        assert_eq!(moved[1].path, "a.spx");
        assert_eq!(moved[0].source, A);
        assert_eq!(moved[1].source, B);
        drop(moved);
    });
    assert_eq!(actual.allocated, 0);
    // Original closure backing was created before observation; its destruction
    // remains in scope but isn't claimed as newly reserved/allocated storage.
    assert_eq!(actual.allocations, 0);
}
#[test]
fn transferred_current_closure_preserves_cold_warm_graph_and_provider_drift_refusal() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    let manifest =
        ProjectManifest::parse(&std::fs::read_to_string(root.join("semaprax.toml")).unwrap())
            .unwrap();
    let sources = manifest
        .sources()
        .iter()
        .map(|path| {
            ProjectFrontendSource::new(path, &std::fs::read_to_string(root.join(path)).unwrap())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut ordinary = ProjectFrontendCache::new();
    let old = ordinary.build(&manifest, &sources).unwrap();
    let mut checked = ProjectFrontendCache::new_with_semantic_cache();
    let cold = checked.build(&manifest, &sources).unwrap();
    let warm = checked.build(&manifest, &sources).unwrap();
    assert_eq!(
        old.revision().semantic_graph(),
        cold.revision().semantic_graph()
    );
    assert_eq!(
        cold.revision().semantic_graph(),
        warm.revision().semantic_graph()
    );
    assert_eq!(
        cold.revision().project_revision(),
        warm.revision().project_revision()
    );
    let image = |build: &crate::project::ProjectFrontendBuild| {
        crate::project::ProjectSemanticImage::derive(
            std::sync::Arc::clone(build.revision()),
            build.revision().project_revision(),
        )
        .unwrap()
    };
    assert_eq!(image(&old).to_json(), image(&cold).to_json());
    assert_eq!(image(&cold).to_json(), image(&warm).to_json());
    let report: serde_json::Value = serde_json::from_str(warm.to_json()).unwrap();
    assert_eq!(report["work"]["modules_resolved"], 0);
    assert_eq!(report["manifest_context_reset"], false);
    let before = checked.fork();
    // Actual current source drift fails the ordinary source protocol rather
    // than gaining authority from a retained checked owner or moved buffer.
    let mut stale = sources
        .iter()
        .map(|s| ProjectFrontendSource::new(s.path(), s.source()).unwrap())
        .collect::<Vec<_>>();
    let provider = stale
        .iter_mut()
        .find(|source| source.path() == "src/core.spx")
        .unwrap();
    provider.source.push_str(" invalid source");
    assert!(checked.build(&manifest, &stale).is_err());
    assert_eq!(checked.context, before.context);
    assert_eq!(checked.entries.len(), before.entries.len());
    for (path, entry) in &before.entries {
        assert!(std::sync::Arc::ptr_eq(entry, &checked.entries[path]));
    }
    for (path, entry) in &before.checked {
        assert!(std::sync::Arc::ptr_eq(entry, &checked.checked[path]));
    }
    let resumed = checked.build(&manifest, &sources).unwrap();
    assert_eq!(
        warm.revision().semantic_graph(),
        resumed.revision().semantic_graph()
    );
}
