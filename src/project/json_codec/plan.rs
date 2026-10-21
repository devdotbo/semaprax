//! Complete codec candidates derived from one immutable authored revision.
use super::{derive_replacement, refusal, JsonCodecProfile};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use crate::semantic_workspace::SemanticWorkspaceSource;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ENTRIES: usize = 8;
const MAX_SOURCES: usize = 64;
const MAX_SOURCE_TOTAL: usize = 2 * 1024 * 1024;
const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonCodecPlanEntry {
    pub source_path: String,
    pub record_id: String,
    pub profile: JsonCodecProfile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonCodecBundleSource {
    pub path: String,
    pub source: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonCodecEntryReceipt {
    pub source_path: String,
    pub record_id: String,
    pub profile: String,
    pub max_string_bytes: Option<usize>,
    pub max_array_items: Option<usize>,
    pub authored_sha256: String,
    pub candidate_sha256: String,
    pub generated_declarations: Vec<String>,
}

/// Reviewable evidence only: no filesystem or managed-generation authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonCodecBundle {
    pub schema: String,
    pub authored_project_revision: String,
    pub candidate_project_revision: String,
    pub candidate_workspace_revision: String,
    pub manifest: String,
    pub authored_sources: Vec<JsonCodecBundleSource>,
    pub candidate_sources: Vec<JsonCodecBundleSource>,
    pub entries: Vec<JsonCodecEntryReceipt>,
}

fn digest(source: &str) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

fn source(path: &str, text: &str) -> JsonCodecBundleSource {
    JsonCodecBundleSource {
        path: path.to_owned(),
        source: text.to_owned(),
        sha256: digest(text),
    }
}

pub(super) fn profile_fields(
    profile: JsonCodecProfile,
) -> (&'static str, Option<usize>, Option<usize>) {
    use JsonCodecProfile::*;
    match profile {
        FlatScalars => ("flat-scalars.v1", None, None),
        IdentifierViews => ("identifier-views.v1", None, None),
        RequestViews => ("request-views.v1", None, None),
        StreamRequestViews => ("stream-request-views.v1", None, None),
        OwnedRequestDecodeOnlyV1 => ("owned-request-decode-only.v1", None, None),
        StreamOwnedRequestDecodeOnlyV1 => ("stream-owned-request-decode-only.v1", None, None),
        Utf8OwnedRequestDecodeOnlyV1 { max_string_bytes } => (
            "utf8-owned-request-decode-only.v1",
            Some(max_string_bytes),
            None,
        ),
        StreamUtf8OwnedRequestDecodeOnlyV1 { max_string_bytes } => (
            "stream-utf8-owned-request-decode-only.v1",
            Some(max_string_bytes),
            None,
        ),
        OwnedRequest => ("owned-request.v1", None, None),
        StreamOwnedRequest => ("stream-owned-request.v1", None, None),
        Utf8OwnedRequest { max_string_bytes } => {
            ("utf8-owned-request.v1", Some(max_string_bytes), None)
        }
        StreamUtf8OwnedRequest { max_string_bytes } => {
            ("stream-utf8-owned-request.v1", Some(max_string_bytes), None)
        }
        CollectionResponse { max_string_bytes } => (
            "bounded-collection-response.v1",
            Some(max_string_bytes),
            None,
        ),
        NestedRequest {
            max_string_bytes,
            max_array_items,
        } => (
            "bounded-nested-request.v1",
            Some(max_string_bytes),
            Some(max_array_items),
        ),
        StreamNestedRequest {
            max_string_bytes,
            max_array_items,
        } => (
            "bounded-stream-nested-request.v1",
            Some(max_string_bytes),
            Some(max_array_items),
        ),
        NestedResponseV2 {
            max_string_bytes,
            max_array_items,
        } => (
            "bounded-nested-response.v2",
            Some(max_string_bytes),
            Some(max_array_items),
        ),
        NestedResponse {
            max_string_bytes,
            max_array_items,
        } => (
            "bounded-nested-response.v1",
            Some(max_string_bytes),
            Some(max_array_items),
        ),
    }
}

// Unverified bytes only. No HIR, candidate revision, checked flag, root path,
// publication operation or public constructor exists on this private carrier.
struct Draft {
    authored_project_revision: String,
    manifest: String,
    authored_sources: Vec<JsonCodecBundleSource>,
    candidate_sources: Vec<SemanticWorkspaceSource>,
    entries: Vec<JsonCodecEntryReceipt>,
}
impl PartialEq for Draft {
    fn eq(&self, other: &Self) -> bool {
        self.authored_project_revision == other.authored_project_revision
            && self.manifest == other.manifest
            && self.authored_sources == other.authored_sources
            && self.entries == other.entries
            && self.candidate_sources.len() == other.candidate_sources.len()
            && self
                .candidate_sources
                .iter()
                .zip(&other.candidate_sources)
                .all(|(a, b)| a.path == b.path && a.source == b.source)
    }
}

/// All selectors read the same revision. No intermediate replacement is an input
/// to another entry. The complete linked candidate is checked before return.
/// Public derivation retains its complete ordinary build and both HIR checks.
pub fn derive_json_codec_plan(
    revision: &ProjectRevision,
    entries: &[JsonCodecPlanEntry],
) -> Result<JsonCodecBundle, Vec<Diagnostic>> {
    let draft = derive_draft(revision, entries)?;
    check_draft(revision, draft)
}

/// Starter-only replay: independent generation compares every field and full
/// source byte before ONE ordinary candidate build. Public replay stays below.
pub(in crate::project) fn derive_replayed_json_codec_plan(
    revision: &ProjectRevision,
    entries: &[JsonCodecPlanEntry],
) -> Result<JsonCodecBundle, Vec<Diagnostic>> {
    let draft = derive_draft(revision, entries)?;
    replay_draft(revision, entries, draft)
}
fn replay_draft(
    revision: &ProjectRevision,
    entries: &[JsonCodecPlanEntry],
    draft: Draft,
) -> Result<JsonCodecBundle, Vec<Diagnostic>> {
    let regenerated = derive_draft(revision, entries)?;
    if draft != regenerated {
        return Err(refusal(
            "JSON codec bundle differs from its exact immutable-revision plan replay",
        ));
    }
    drop(regenerated); // No second draft backing survives candidate admission.
    check_draft(revision, draft)
}

fn derive_draft(
    revision: &ProjectRevision,
    entries: &[JsonCodecPlanEntry],
) -> Result<Draft, Vec<Diagnostic>> {
    revision.check()?;
    if entries.is_empty()
        || entries.len() > MAX_ENTRIES
        || revision.sources().len() > MAX_SOURCES
        || revision
            .sources()
            .iter()
            .map(|s| s.source().len())
            .try_fold(0usize, usize::checked_add)
            .is_none_or(|total| total > MAX_SOURCE_TOTAL)
    {
        return Err(refusal(
            "JSON codec plan exceeds its fixed entry/source bounds",
        ));
    }
    let mut ordered = entries.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    let mut paths = BTreeSet::new();
    let mut records = BTreeSet::new();
    for entry in &ordered {
        if !paths.insert(&entry.source_path) || !records.insert(&entry.record_id) {
            return Err(refusal(
                "JSON codec plan contains duplicate or overlapping outputs/records",
            ));
        }
    }
    let mut replacements = BTreeMap::new();
    let mut receipts = Vec::new();
    for entry in ordered {
        let generated = derive_replacement(
            revision,
            &entry.source_path,
            &entry.record_id,
            entry.profile,
        )?;
        let original = revision
            .sources()
            .iter()
            .find(|s| s.path() == entry.source_path)
            .expect("derivation authenticated path");
        let authored = crate::parse(original.source(), &entry.source_path).map_err(|e| vec![e])?;
        let authored_ids = authored
            .types
            .iter()
            .map(|t| t.stable_id.as_str())
            .chain(authored.functions.iter().map(|f| f.stable_id.as_str()))
            .collect::<BTreeSet<_>>();
        let parsed = crate::parse(&generated, &entry.source_path).map_err(|e| vec![e])?;
        let mut generated_declarations = parsed
            .types
            .iter()
            .map(|t| &t.stable_id)
            .chain(parsed.functions.iter().map(|f| &f.stable_id))
            .filter(|id| !authored_ids.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        generated_declarations.sort();
        let (profile, bytes, items) = profile_fields(entry.profile);
        receipts.push(JsonCodecEntryReceipt {
            source_path: entry.source_path.clone(),
            record_id: entry.record_id.clone(),
            profile: profile.to_owned(),
            max_string_bytes: bytes,
            max_array_items: items,
            authored_sha256: digest(original.source()),
            candidate_sha256: digest(&generated),
            generated_declarations,
        });
        replacements.insert(entry.source_path.clone(), generated);
    }
    let candidate_sources = revision
        .sources()
        .iter()
        .map(|s| SemanticWorkspaceSource {
            path: s.path().to_owned(),
            source: replacements
                .remove(s.path())
                .unwrap_or_else(|| s.source().to_owned()),
        })
        .collect::<Vec<_>>();
    if candidate_sources
        .iter()
        .map(|s| s.source.len())
        .try_fold(0usize, usize::checked_add)
        .is_none_or(|total| total > MAX_SOURCE_TOTAL)
    {
        return Err(refusal(
            "JSON codec combined candidate exceeds its fixed total source bound",
        ));
    }
    let mut originals = revision
        .sources()
        .iter()
        .map(|s| source(s.path(), s.source()))
        .collect::<Vec<_>>();
    originals.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Draft {
        authored_project_revision: revision.project_revision().to_owned(),
        manifest: revision.manifest().to_canonical_toml(),
        authored_sources: originals,
        candidate_sources,
        entries: receipts,
    })
}

fn check_draft(
    revision: &ProjectRevision,
    draft: Draft,
) -> Result<JsonCodecBundle, Vec<Diagnostic>> {
    // Ordinary complete Project resolution rejects cross-entry declaration and
    // import collisions; no helper names or receipt fields bypass admission.
    #[cfg(test)]
    tests::note_check(0);
    let candidate =
        crate::project::build::build_owned(revision.manifest(), draft.candidate_sources)?;
    #[cfg(test)]
    tests::note_check(1);
    crate::hir::validate(&candidate.entry_program).map_err(|e| vec![e])?;
    #[cfg(test)]
    tests::note_check(2);
    crate::hir::validate(&candidate.test_program).map_err(|e| vec![e])?;
    let mut sources = candidate
        .sources
        .iter()
        .map(|s| source(&s.path, &s.source))
        .collect::<Vec<_>>();
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    let bundle = JsonCodecBundle {
        schema: "semaprax.json-codec-bundle.v1".to_owned(),
        authored_project_revision: draft.authored_project_revision,
        candidate_project_revision: candidate.project_revision,
        candidate_workspace_revision: candidate.workspace_revision,
        manifest: draft.manifest,
        authored_sources: draft.authored_sources,
        candidate_sources: sources,
        entries: draft.entries,
    };
    // Count serialized bytes without allocating a second oversized artifact.
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_BUNDLE_BYTES)
                .ok_or_else(|| std::io::Error::other("bundle byte bound"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(&mut Count(1), &bundle)
        .map_err(|_| refusal("JSON codec bundle exceeds its fixed serialized byte bound"))?;
    Ok(bundle)
}

/// Receipts grant no authority: replay the exact plan and all complete bytes.
pub fn verify_json_codec_plan(
    revision: &ProjectRevision,
    entries: &[JsonCodecPlanEntry],
    bundle: &JsonCodecBundle,
) -> Result<(), Vec<Diagnostic>> {
    if bundle.schema != "semaprax.json-codec-bundle.v1"
        || bundle.authored_project_revision != revision.project_revision()
        || bundle.entries.len() > MAX_ENTRIES
        || bundle.authored_sources.len() > MAX_SOURCES
        || bundle.candidate_sources.len() > MAX_SOURCES
        || bundle
            .authored_sources
            .iter()
            .chain(&bundle.candidate_sources)
            .any(|s| s.source.len() > MAX_SOURCE_TOTAL)
        || &derive_json_codec_plan(revision, entries)? != bundle
    {
        return Err(refusal(
            "JSON codec bundle differs from its exact immutable-revision plan replay",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::project) mod before;
#[cfg(test)]
pub(in crate::project) mod tests;
