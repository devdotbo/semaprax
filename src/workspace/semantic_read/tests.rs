//! Single-use source and graph extraction from a semantic read authority.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::workspace::{acquire_semantic_change_read, acquire_semantic_read};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const PROVIDER: &str = "module read.provider;\n\n@id(\"read.value\")\nfn value() -> i64 { 1 }\n";
const ENTRY: &str = "module read.entry;\n\n@id(\"read.entry.main\")\nfn main() -> i64 { 2 }\n";

struct Fixture {
    root: PathBuf,
    sources: Vec<(String, String)>,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-semantic-read-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let mut sources = Vec::new();
        for (path, source) in [("a/provider.spx", PROVIDER), ("z/entry.spx", ENTRY)] {
            let program = crate::parse(source, path).unwrap();
            let canonical = crate::format::canonical(&program);
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), &canonical).unwrap();
            sources.push((path.to_owned(), canonical));
        }
        let path_set = root.join("paths.json");
        std::fs::write(
            &path_set,
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/entry.spx\"}]}\n",
        )
        .unwrap();
        crate::semantic_workspace::initialize(&root, &path_set).unwrap();
        Self { root, sources }
    }

    fn active(&self) -> Vec<u8> {
        std::fs::read(self.root.join(".semaprax-workspace/ACTIVE")).unwrap()
    }

    fn assert_lock_released(&self) {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.root.join(".semaprax-workspace/LOCK"))
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).expect("the read authority must release LOCK");
        fs2::FileExt::unlock(&lock).unwrap();
    }

    fn assert_sources_unchanged(&self) {
        for (path, source) in &self.sources {
            assert_eq!(
                std::fs::read_to_string(self.root.join(path)).unwrap(),
                *source
            );
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn assert_consumed(diagnostics: &[crate::diagnostic::Diagnostic], what: &str) {
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-G153");
    assert_eq!(
        diagnostics[0].message,
        format!("semantic workspace {what} already consumed")
    );
}

/// Every field of one complete extraction through a separate authority.
fn extraction_of(root: &Path) -> Vec<(String, String, String, String, String)> {
    let mut authority = acquire_semantic_read(root).unwrap();
    let sources = authority.take_sources().unwrap();
    authority.take_graph().unwrap();
    authority.finish(Ok(())).unwrap();
    sources
        .into_iter()
        .map(|file| {
            (
                file.path,
                file.source_graph_schema,
                file.source_revision,
                file.source_digest,
                file.source,
            )
        })
        .collect()
}

#[test]
fn source_first_extraction_returns_the_snapshot_once() {
    let fixture = Fixture::new("source-first");
    let expected = extraction_of(&fixture.root);
    let active = fixture.active();
    let mut authority = acquire_semantic_read(&fixture.root).unwrap();

    let sources = authority.take_sources().unwrap();
    let extracted = sources
        .iter()
        .map(|source| {
            (
                source.path.clone(),
                source.source_graph_schema.clone(),
                source.source_revision.clone(),
                source.source_digest.clone(),
                source.source.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(extracted, expected);
    assert_eq!(
        extracted
            .iter()
            .map(|file| (file.0.clone(), file.4.clone()))
            .collect::<Vec<_>>(),
        fixture.sources
    );

    // The second extraction is refused outright: no metadata comes back
    // paired with the source strings the first extraction emptied.
    assert_consumed(&authority.take_sources().err().unwrap(), "sources were");
    authority.take_graph().unwrap();
    authority.finish(Ok(())).unwrap();
    fixture.assert_lock_released();
    assert_eq!(fixture.active(), active);
    fixture.assert_sources_unchanged();
}

#[test]
fn graph_first_extraction_keeps_both_single_use() {
    let fixture = Fixture::new("graph-first");
    let expected = extraction_of(&fixture.root);
    let mut authority = acquire_semantic_change_read(&fixture.root).unwrap();

    authority.take_graph().unwrap();
    assert_consumed(&authority.take_graph().err().unwrap(), "graph was");
    let sources = authority.take_sources().unwrap();
    assert_eq!(sources.len(), expected.len());
    for (source, expected) in sources.iter().zip(&expected) {
        assert_eq!(source.path, expected.0);
        assert_eq!(source.source_digest, expected.3);
        assert_eq!(source.source, expected.4);
    }
    assert_consumed(&authority.take_sources().err().unwrap(), "sources were");
    authority.finish(Ok(())).unwrap();
    fixture.assert_lock_released();
}

#[test]
fn a_double_extraction_failure_unlocks_and_leaves_the_workspace_unchanged() {
    let fixture = Fixture::new("double-extraction-cleanup");
    let active = fixture.active();
    let mut authority = acquire_semantic_change_read(&fixture.root).unwrap();

    let result = (|| {
        let _graph = authority.take_graph()?;
        let _first = authority.take_sources()?;
        authority.take_sources()
    })();
    let diagnostics = authority.finish(result).err().unwrap();
    assert_consumed(&diagnostics, "sources were");
    fixture.assert_lock_released();
    assert_eq!(fixture.active(), active);
    fixture.assert_sources_unchanged();

    // Unconsumed graph authority still fails `finish` closed after a source
    // extraction, and still releases the lock.
    let mut authority = acquire_semantic_read(&fixture.root).unwrap();
    authority.take_sources().unwrap();
    let diagnostics = authority.finish(Ok(())).err().unwrap();
    assert_eq!(diagnostics[0].code, "SPX-G153");
    fixture.assert_lock_released();
    assert_eq!(fixture.active(), active);
}

#[test]
fn typed_finish_unlocks_once_and_moves_original_failure_without_publication() {
    let fixture = Fixture::new("typed-failure");
    let before = fixture.active();
    let authority = acquire_semantic_read(&fixture.root).unwrap();
    let diagnostics = vec![crate::diagnostic::Diagnostic::io_static(
        "SPX-G153",
        "semantic workspace graph already consumed",
    )];
    let pointer = diagnostics.as_ptr();
    let text = diagnostics[0].message.as_ptr();
    let failure = authority
        .finish_reporting::<()>(Err(diagnostics.into()))
        .unwrap_err();
    assert_eq!(failure.reported().unwrap().as_ptr(), pointer);
    assert_eq!(failure.reported().unwrap()[0].message.as_ptr(), text);
    assert_eq!(fixture.active(), before);
    fixture.assert_sources_unchanged();
    fixture.assert_lock_released();
}

#[test]
fn actual_authority_finish_moves_exact_paid_setup_refusal_through_unlock_and_drop() {
    use crate::cleanup::ReplayContext;
    use crate::cleanup_plan::{
        ReplayPhysicalReservation as Physical, SelectedReplayReport as Acquired,
        SelectedReplayReportForm as Form,
    };
    let fixture = Fixture::new("typed-paid-failure");
    let authority = acquire_semantic_read(&fixture.root).unwrap();
    let program = crate::hir::resolve(
        &crate::check(
            "module finish.paid; @id(\"main\") fn main()->i64{0}",
            "finish.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let functions = program.functions();
    let function = functions.get(0).unwrap();
    let form = Form::new("SPX-T781", "retained authority setup failure");
    let bytes = std::mem::size_of::<crate::diagnostic::Diagnostic>() + form.message().len();
    // Inline snapshots cover the complete existing acquisition/finish/Drop route.
    // They do not open child observations or change payment or counted backing.
    let mut boundaries = [crate::test_allocation_observer::Metrics::default(); 6];
    let ((), cost) = crate::test_allocation_observer::measure(|| {
        let acquired = Acquired::try_acquire(Physical::new(bytes), form)
            .unwrap_or_else(|_| panic!("exact report"));
        boundaries[0] = crate::test_allocation_observer::current_metrics();
        let pointers = acquired.backing();
        let refusal = acquired.prepare(function).err().unwrap();
        boundaries[1] = crate::test_allocation_observer::current_metrics();
        let failure =
            crate::workspace_report::WorkspaceReportFailure::selected(refusal, Some(form));
        boundaries[2] = crate::test_allocation_observer::current_metrics();
        let mut failure = authority.finish_reporting::<()>(Err(failure)).unwrap_err();
        boundaries[3] = crate::test_allocation_observer::current_metrics();
        assert_eq!(failure.reported().unwrap().as_ptr(), pointers.0);
        assert_eq!(failure.reported().unwrap()[0].message.as_ptr(), pointers.1);
        let ledger = failure.selected_owner_mut().unwrap().ledger_mut().unwrap();
        let first = ledger.reserve(0).unwrap_err();
        assert_eq!(ledger.reserve(1).unwrap_err(), first);
        boundaries[4] = crate::test_allocation_observer::current_metrics();
        drop(failure);
        boundaries[5] = crate::test_allocation_observer::current_metrics();
    });
    assert!(!cost.invalid);
    assert_eq!(
        (
            cost.allocations,
            cost.reallocations,
            cost.allocated,
            cost.live,
            cost.freed
        ),
        (2, 0, bytes, 0, bytes),
        "complete authority route; boundaries [report acquired, prepared refusal, affine failure, finish returned after authority Drop, sticky retries, failure dropped]={boundaries:?}"
    );
    fixture.assert_sources_unchanged();
    fixture.assert_lock_released();
}
