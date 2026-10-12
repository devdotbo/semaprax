//! Actual invocation cache lifecycle. Raw input strings and the independent
//! ordinary-owned oracle (including common process-static initialization) are
//! prepared outside each allocation observation. Independent equality/wire
//! checks inside a scope are included in its requested layouts.
//! Cold covers parse/resolve/cache/core/final Project publication and Drop;
//! warm excludes the already held cold owner but includes full current replay,
//! core retention, final publication and Drop. Counts are requested layouts,
//! not RSS, compiler quotas, or a claim about every Project profile.
use super::super::super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

const MANIFEST: &str = r#"schema = "semaprax.project.v1"
name = "lifecycle"
entry = "lifecycle.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["lifecycle.answer"]
tests = ["lifecycle.tests"]
"#;
const CORE: &str =
    "module lifecycle.core; @id(\"lifecycle.helper\") fn helper(value:i64)->i64 { value+2 }";
const APP: &str = "module lifecycle.app; use function @id(\"lifecycle.helper\") from lifecycle.core as helper; @id(\"lifecycle.main\") fn main()->i64 { helper(40) } @id(\"lifecycle.answer\") fn answer(value:i64)->i64 {helper(value)}";
const TESTS: &str = "module lifecycle.tests; @id(\"lifecycle.tests.main\") fn main()->i64 {0}";

fn sources(core: &str) -> Vec<ProjectFrontendSource> {
    [
        ("src/app.spx", APP),
        ("src/core.spx", core),
        ("src/tests.spx", TESTS),
    ]
    .into_iter()
    .map(|(path, source)| {
        let ast = crate::parse(source, path).unwrap();
        let canonical = crate::format::canonical(&ast);
        let round = crate::parse(&canonical, path).unwrap();
        assert_eq!(crate::format::canonical(&round), canonical);
        ProjectFrontendSource::new(path, &canonical).unwrap()
    })
    .collect()
}
fn same(actual: &ProjectRevision, expected: &ProjectRevision) {
    assert_eq!(actual.project_revision(), expected.project_revision());
    assert_eq!(actual.workspace_revision(), expected.workspace_revision());
    assert_eq!(actual.semantic_graph(), expected.semantic_graph());
    for (actual, expected) in [
        (actual.entry_program(), expected.entry_program()),
        (actual.public_api_program(), expected.public_api_program()),
        (actual.test_program(), expected.test_program()),
    ] {
        assert_eq!(actual, expected);
        crate::hir::validate(actual).unwrap();
    }
}
fn settled(label: &str, metrics: Metrics) {
    assert!(!metrics.invalid, "{label}: {metrics:?}");
    assert!(metrics.allocations > 0, "{label}: {metrics:?}");
    assert!(metrics.allocated >= metrics.highwater);
    assert_eq!(metrics.live, 0, "{label}: {metrics:?}");
    assert_eq!(metrics.allocated, metrics.freed, "{label}: {metrics:?}");
    println!("OPT777 requested-layout scope={label} allocations={} reallocations={} requested={} highwater={} freed={} live={} observer_metadata=excluded", metrics.allocations, metrics.reallocations, metrics.allocated, metrics.highwater, metrics.freed, metrics.live);
}

/// Fixed Copy snapshots contain no observer-time text/backing. Report them
/// after the whole observation, before its unchanged settlement assertion, so
/// an actual surviving allocation is attributable to its acquisition phase.
fn stages(label: &str, snapshots: [Metrics; 7]) {
    for (stage, metrics) in [
        "cache_new",
        "build",
        "independent_parity",
        "json_report",
        "publication_drop",
        "cache_drop",
        "report_drop",
    ]
    .into_iter()
    .zip(snapshots)
    {
        println!("OPT777 stage scope={label} stage={stage} metrics={metrics:?} fixed_copy_observation=excluded");
    }
}

#[test]
fn actual_cold_warm_cache_core_and_publication_match_owned_receiver_through_drop() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    let sources = sources(CORE);
    // This fixture uses the actual formatter to acquire canonical source. The
    // public builder must still reject the original noncanonical proposal.
    let invalid = [
        ("src/app.spx", APP),
        ("src/core.spx", CORE),
        ("src/tests.spx", TESTS),
    ]
    .into_iter()
    .map(|(path, source)| ProjectFrontendSource::new(path, source).unwrap())
    .collect::<Vec<_>>();
    let mut rejected = ProjectFrontendCache::new_with_semantic_cache();
    let errors = rejected.build(&manifest, &invalid).err().unwrap();
    assert!(errors.iter().any(|error| error.code == "SPX-G170"
        && error.message == "workspace semantic source `src/app.spx` is not canonical"));
    assert!(
        rejected.checked.is_empty(),
        "failed source acquisition cannot publish a checked owner"
    );
    // The saved owned receiver is the unchanged AST-only cache path; it has
    // no checked HIR lease and resolves/retains owned modules independently.
    let mut ordinary = ProjectFrontendCache::new();
    let oracle = ordinary.build(&manifest, &sources).unwrap();
    let (cold_stages, cold) = measure(|| {
        let mut snapshots = [Metrics::default(); 7];
        let mut cache = ProjectFrontendCache::new_with_semantic_cache();
        snapshots[0] = current_metrics();
        let published = cache.build(&manifest, &sources).unwrap();
        snapshots[1] = current_metrics();
        same(published.revision(), oracle.revision());
        snapshots[2] = current_metrics();
        let report: Value = serde_json::from_str(published.to_json()).unwrap();
        snapshots[3] = current_metrics();
        assert_eq!(report["work"]["modules_parsed"], 3);
        assert_eq!(report["work"]["modules_resolved"], 3);
        assert_eq!(report["work"]["checked_HIR_reused"], 0);
        assert_eq!(cache.checked.len(), 3);
        drop(published);
        snapshots[4] = current_metrics();
        drop(cache);
        snapshots[5] = current_metrics();
        drop(report);
        snapshots[6] = current_metrics();
        snapshots
    });
    stages("cold_parse_to_publication_drop", cold_stages);
    settled("cold_parse_to_publication_drop", cold);

    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    drop(cache.build(&manifest, &sources).unwrap());
    let pointer = cache.checked["src/core.spx"].resolved.functions.as_ptr();
    let (warm_stages, warm) = measure(|| {
        let mut snapshots = [Metrics::default(); 7];
        snapshots[0] = current_metrics();
        let published = cache.build(&manifest, &sources).unwrap();
        snapshots[1] = current_metrics();
        same(published.revision(), oracle.revision());
        snapshots[2] = current_metrics();
        assert_eq!(
            pointer,
            cache.checked["src/core.spx"].resolved.functions.as_ptr()
        );
        let report: Value = serde_json::from_str(published.to_json()).unwrap();
        snapshots[3] = current_metrics();
        assert_eq!(report["work"]["modules_parsed"], 0);
        assert_eq!(report["work"]["modules_resolved"], 0);
        assert_eq!(report["work"]["checked_HIR_reused"], 3);
        assert_eq!(report["work"]["full_cross_file_checks"], true);
        assert_eq!(report["work"]["full_link_and_profile_admission"], true);
        drop(published);
        snapshots[4] = current_metrics();
        drop(cache);
        snapshots[5] = current_metrics();
        drop(report);
        snapshots[6] = current_metrics();
        snapshots
    });
    stages("warm_lookup_to_publication_drop", warm_stages);
    settled("warm_lookup_to_publication_drop", warm);

    // A source signature drift must fail the actual complete current compiler
    // route identically in owned and checked modes, leaving the held owner
    // reusable only with its exact original source again.
    let changed = self::sources(&CORE.replace("value:i64", "value:i32"));
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    drop(cache.build(&manifest, &sources).unwrap());
    let before = cache.checked["src/core.spx"].resolved.functions.as_ptr();
    let actual = cache.build(&manifest, &changed).err().unwrap();
    let expected = ordinary.build(&manifest, &changed).err().unwrap();
    assert_eq!(actual, expected);
    // Integer operands are checked in the changed i32 parameter context;
    // the standalone helper returns i32. The complete workspace checks its
    // source-ordered app callers first, so their i64 arguments win there.
    let provider = changed
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap();
    let independent = crate::check(provider.source(), "src/core.spx").unwrap_err();
    let result_mismatch = |error: &crate::diagnostic::Diagnostic| {
        error.code == "SPX-T103"
            && error.message == "function `helper` returns i32, but its signature declares i64"
    };
    assert!(independent.iter().any(result_mismatch), "{independent:?}");
    assert_eq!(actual.len(), 2, "{actual:?}");
    for error in &actual {
        assert_eq!(error.code, "SPX-T205");
        assert_eq!(error.message, "argument `value` to `helper` expects i32, received i64");
        assert_eq!(error.path.as_deref(), Some("src/app.spx"));
    }
    assert_eq!(
        before,
        cache.checked["src/core.spx"].resolved.functions.as_ptr()
    );
    let restored = cache.build(&manifest, &sources).unwrap();
    same(restored.revision(), oracle.revision());
    assert_eq!(
        before,
        cache.checked["src/core.spx"].resolved.functions.as_ptr()
    );
}
