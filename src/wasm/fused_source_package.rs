//! Versioned source-only `fused-construction-v1` Core-Wasm package (OPT #768).
//!
//! The source facade reads and parses one source exactly once, builds the
//! sealed `ValidatedFusedSelection` and hands that same owner here. This
//! adapter never reads, parses, resolves or validates source again and has no
//! ordinary fallback: [`emit_fused_source_package`] calls the private selected
//! receiver (`selected_fused::accept`) once and builds every artifact from that
//! one result, so the module, its grant and the host glue cannot come from
//! different owners. No caller supplies a grant or a module.
//!
//! The package is trusted first-install realm glue, not a JavaScript sandbox.
//! `account_bytes` is the compiled maximum; the published entry's
//! `install(accountBytes)` takes the caller's explicit finite limit, at most
//! that maximum and 1 GiB. That one limit funds both eagerly allocated buffers:
//! the namespace registry (reserved first) and the account arena (the
//! remainder). The arena is held cumulatively across invocation, cleanup and
//! reentry, never reset or refunded, and carries the host's unchanged
//! per-invocation registry debit, row backing and lease/view tables.
//! JavaScript objects, Maps, closures, String bytes and the ordinary Bytes
//! payload are not funded by it, so no total heap or RSS bound is claimed. No
//! fuel is enforced on this target. The interpreter's test-only memory limit
//! is not a Wasm budget.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "reached only through the fused-construction-v1 source facade"
    )
)]
use super::selected_fused::{accept, Artifact};
use crate::diagnostic::Diagnostic;
use sha2::{Digest, Sha256};
use std::path::Path;

#[cfg(test)]
mod tests;

/// Profile, strategy and schema identities bound into every artifact.
pub(crate) const PROFILE: &str = "fused-construction-v1";
pub(crate) const STRATEGY: &str = "FusedConstructionV1";
const PACKAGE_SCHEMA: &str = "semaprax.fused-construction-package.v1";
/// The shared source-profile result envelope used by every engine.
pub(crate) const RESULT_SCHEMA: &str = "semaprax.fused-source.v1";

/// Fixed package inventory, in publication order.
pub(crate) const MANIFEST_FILE: &str = "fused-construction-v1.json";
pub(crate) const ENTRY_FILE: &str = "fused-construction-v1.mjs";
pub(crate) const MODULE_FILE: &str = "fused-construction-v1.wasm";
pub(crate) const HOST_FILE: &str = "selected-host.mjs";
const FILES: [&str; 4] = [MANIFEST_FILE, ENTRY_FILE, MODULE_FILE, HOST_FILE];

/// The private host, shipped byte for byte; publication refuses any other.
const HOST: &str = include_str!("selected_fused/host.mjs");
const ENTRY_TEMPLATE: &str = include_str!("fused_source_package/entry.mjs");
const MANIFEST_SLOT: &str = "__SEMAPRAX_FUSED_MANIFEST__";
/// The entry's only module source: the compiler-paired file, fixed URL.
const PAIRED_LOADER: &str = "new URL('./fused-construction-v1.wasm', import.meta.url)";
/// Per registry entry, as `host.mjs` lays it out (cross-checked by tests).
const REGISTRY_ENTRY_BYTES: u64 = 8 + 4 * 6 + 1;
/// Shared source-profile account limit ceiling (1 GiB).
pub(crate) const MAX_ACCOUNT_LIMIT: u64 = 1 << 30;

fn refusal(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn lowercase_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Emit the complete package from the same sealed owner. `account_bytes` is
/// the caller's explicit compiled funding maximum (positive, at most 1 GiB);
/// `source_sha256` is the exact raw-source digest supplied only by the facade.
/// A run-time limit below the registry buffer, or below one invocation's
/// arena requirement, is refused by the entry or the actual funded arena.
pub(crate) fn emit_fused_source_package(
    owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
    account_bytes: usize,
    source_sha256: &str,
) -> Result<crate::fused_source::WasmPackage, Diagnostic> {
    let files = package_files(owner, account_bytes, source_sha256)?;
    Ok(crate::fused_source::WasmPackage::from_files(files))
}

/// Re-verify every artifact's binding, then publish atomically through the
/// existing fresh-directory package writer. Nothing is written on refusal.
pub(crate) fn publish_fused_source_package(
    package: &crate::fused_source::WasmPackage,
    output: &Path,
) -> Result<(), Diagnostic> {
    let artifacts: Vec<(&str, &[u8])> = package.files().collect();
    verify_files(&artifacts)?;
    super::publish_scalar_package(output, &artifacts)
}

/// All package files, built from one `accept` result. Exposed to the parent
/// only so owning tests can exercise splices without the facade type.
pub(super) fn package_files(
    owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
    account_bytes: usize,
    source_sha256: &str,
) -> Result<Vec<(&'static str, Vec<u8>)>, Diagnostic> {
    // Caller inputs refuse before any lowering or artifact exists.
    if !lowercase_sha256(source_sha256) {
        return Err(refusal(
            "fused-construction-v1 source identity must be one lowercase SHA-256 digest",
        ));
    }
    let funding = u64::try_from(account_bytes)
        .ok()
        .filter(|bytes| (1..=MAX_ACCOUNT_LIMIT).contains(bytes))
        .ok_or_else(|| {
            refusal("fused-construction-v1 requires an explicit positive account limit of at most 1 GiB")
        })?;
    let artifact = accept(owner).map_err(|reason| {
        refusal(format!(
            "fused-construction-v1 Core-Wasm receiver refused the sealed selection: {reason:?}"
        ))
    })?;
    let manifest = manifest(&artifact, funding, source_sha256)?;
    let entry = ENTRY_TEMPLATE.replacen(MANIFEST_SLOT, &manifest, 1);
    if entry.contains(MANIFEST_SLOT)
        || ENTRY_TEMPLATE.matches(MANIFEST_SLOT).count() != 1
        || ENTRY_TEMPLATE.matches(PAIRED_LOADER).count() != 1
        || !ENTRY_TEMPLATE.contains("export async function install(accountBytes)")
    {
        return Err(refusal("fused-construction-v1 entry template is malformed"));
    }
    let files = vec![
        (MANIFEST_FILE, manifest.into_bytes()),
        (ENTRY_FILE, entry.into_bytes()),
        (MODULE_FILE, artifact.bytes),
        (HOST_FILE, HOST.as_bytes().to_vec()),
    ];
    let borrowed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(name, bytes)| (*name, bytes.as_slice()))
        .collect();
    verify_files(&borrowed)?;
    Ok(files)
}

/// Deterministic manifest JSON. Every value is a fixed identity, a lowercase
/// digest, an integer or a fixed role name, so no escaping is required.
fn manifest(artifact: &Artifact, funding: u64, source_sha256: &str) -> Result<String, Diagnostic> {
    let grant = &artifact.grant;
    let module_sha256 = sha256_hex(&artifact.bytes);
    if grant.digest != module_sha256 {
        return Err(refusal("selected grant is not bound to the emitted module"));
    }
    let registry_buffer = grant
        .registry_entries
        .checked_mul(REGISTRY_ENTRY_BYTES)
        .ok_or_else(|| refusal("selected registry dimensions overflow"))?;
    let order = grant
        .order
        .iter()
        .map(|role| format!("\"{role}\""))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        concat!(
            "{{\"schema\":\"{schema}\",\"profile\":\"{profile}\",\"strategy\":\"{strategy}\",",
            "\"target\":\"wasm\",\"realm\":\"trusted-first-install\",",
            "\"result_schema\":\"{result}\",\"result\":\"i64\",\"entry\":\"{entry}\",",
            "\"source_sha256\":\"{source}\",\"module_sha256\":\"{module}\",",
            "\"grant\":{{\"digest\":\"{module}\",\"rows\":{rows},\"width\":{width},",
            "\"metadataBytes\":{metadata},\"rowBytes\":{row_bytes},\"accountBytes\":{account},",
            "\"registryEntries\":{entries},\"order\":[{order}]}},",
            "\"funding\":{{\"account_bytes\":{funding},\"account_limit_max\":{limit_max},",
            "\"registry_buffer_bytes\":{registry},\"per_invocation_arena_bytes\":{account},",
            "\"allocation\":\"eager-registry-then-eager-arena-remainder\",",
            "\"funded\":[\"registry-buffer\",\"arena-registry-debit\",\"row-backing\",",
            "\"lease-view-tables\"],",
            "\"not_funded\":[\"javascript-objects-maps-closures\",\"string-bytes\",",
            "\"bytes-payload-ordinary-counters\"],\"bound\":\"funded-buffers-only\",",
            "\"fuel\":\"none\"}}}}"
        ),
        schema = PACKAGE_SCHEMA,
        profile = PROFILE,
        strategy = STRATEGY,
        result = RESULT_SCHEMA,
        entry = super::selected_fused::ENTRY,
        source = source_sha256,
        module = module_sha256,
        rows = grant.rows,
        width = grant.width,
        metadata = grant.metadata_bytes,
        row_bytes = grant.row_bytes,
        account = grant.account_bytes,
        entries = grant.registry_entries,
        order = order,
        funding = funding,
        limit_max = MAX_ACCOUNT_LIMIT,
        registry = registry_buffer,
    ))
}

/// The exact inventory, the shipped host, and one module digest bound by the
/// manifest, the grant and the entry. Any splice refuses before publication.
fn verify_files(files: &[(&str, &[u8])]) -> Result<(), Diagnostic> {
    let names: Vec<&str> = files.iter().map(|(name, _)| *name).collect();
    if names != FILES {
        return Err(refusal("fused-construction-v1 package inventory differs"));
    }
    let [(_, manifest), (_, entry), (_, module), (_, host)] = files else {
        return Err(refusal("fused-construction-v1 package inventory differs"));
    };
    if *host != HOST.as_bytes() {
        return Err(refusal(
            "fused-construction-v1 host differs from the private host",
        ));
    }
    let manifest = std::str::from_utf8(manifest)
        .map_err(|_| refusal("fused-construction-v1 manifest is not UTF-8"))?;
    let entry = std::str::from_utf8(entry)
        .map_err(|_| refusal("fused-construction-v1 entry is not UTF-8"))?;
    let module_sha256 = sha256_hex(module);
    let bound_module = format!("\"module_sha256\":\"{module_sha256}\"");
    let bound_grant = format!("\"grant\":{{\"digest\":\"{module_sha256}\"");
    let (prefix, suffix) = ENTRY_TEMPLATE
        .split_once(MANIFEST_SLOT)
        .ok_or_else(|| refusal("fused-construction-v1 entry template is malformed"))?;
    if manifest.matches("\"module_sha256\":").count() != 1
        || manifest.matches("\"digest\":").count() != 1
        || !manifest.contains(&bound_module)
        || !manifest.contains(&bound_grant)
        || !manifest.contains(&format!("\"profile\":\"{PROFILE}\""))
        || !manifest.contains(&format!("\"strategy\":\"{STRATEGY}\""))
        || entry != format!("{prefix}{manifest}{suffix}")
    {
        return Err(refusal(
            "fused-construction-v1 package artifacts are not bound to one module",
        ));
    }
    Ok(())
}
