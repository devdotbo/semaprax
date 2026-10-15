//! Bounded read-only shape discovery. A matching generator predicate is not
//! candidate admission, a dependency grant, or a backend support verdict.
use super::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Every omitted policy remains an explicit unmet prerequisite, never a default
/// codec choice. Continuations require the exact revision from the first page.
#[derive(Clone, Debug)]
pub struct JsonCodecSuitabilityOptions {
    pub source: Option<String>,
    pub record: String,
    pub target: Option<String>,
    pub revision: Option<String>,
    pub max_string_bytes: Option<usize>,
    pub max_array_items: Option<usize>,
    pub max_bytes: usize,
    pub offset: usize,
}
impl Default for JsonCodecSuitabilityOptions {
    fn default() -> Self {
        Self {
            source: None,
            record: String::new(),
            target: None,
            revision: None,
            max_string_bytes: None,
            max_array_items: None,
            max_bytes: 4096,
            offset: 0,
        }
    }
}
fn hash(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
fn diagnostics(errors: Vec<Diagnostic>) -> Value {
    json!(errors
        .into_iter()
        .map(|d| json!({"code":d.code,"reason":d.message}))
        .collect::<Vec<_>>())
}
fn profiles(string: usize, array: usize) -> [JsonCodecProfile; 12] {
    use JsonCodecProfile::*;
    [
        FlatScalars,
        IdentifierViews,
        RequestViews,
        StreamRequestViews,
        OwnedRequest,
        StreamOwnedRequest,
        Utf8OwnedRequest {
            max_string_bytes: string,
        },
        StreamUtf8OwnedRequest {
            max_string_bytes: string,
        },
        CollectionResponse {
            max_string_bytes: string,
        },
        NestedRequest {
            max_string_bytes: string,
            max_array_items: array,
        },
        StreamNestedRequest {
            max_string_bytes: string,
            max_array_items: array,
        },
        NestedResponse {
            max_string_bytes: string,
            max_array_items: array,
        },
    ]
}
mod inspection;
mod paging;
#[cfg(test)]
mod saved_inspection;
#[cfg(test)]
mod saved_page;

/// Inspect existing generator predicates against one currently admitted source.
/// No replacement is checked, retained, written or returned. All candidate and
/// backend verification remains mandatory at the ordinary derivation boundary.
pub fn explain_json_codec_suitability(
    revision: &ProjectRevision,
    options: &JsonCodecSuitabilityOptions,
) -> Result<String, Vec<Diagnostic>> {
    if !(512..=16384).contains(&options.max_bytes)
        || options
            .max_string_bytes
            .is_some_and(|n| !(1..=64).contains(&n))
        || options
            .max_array_items
            .is_some_and(|n| !(1..=256).contains(&n))
        || options.offset >= 12
        || (options.offset != 0 && options.revision.is_none())
        || options
            .revision
            .as_ref()
            .is_some_and(|r| r != revision.project_revision())
    {
        return Err(refusal("codec explanation requires bounded policy, offset and an exact current revision for continuation"));
    }
    if options
        .target
        .as_ref()
        .is_some_and(|t| t != "native64" && t != "wasm32")
    {
        return Err(refusal(
            "codec explanation target must be native64 or wasm32",
        ));
    }
    revision.check()?;
    let mut selected = None;
    for source in revision.sources() {
        if !revision
            .manifest()
            .sources()
            .iter()
            .any(|p| p == source.path())
            || options.source.as_ref().is_some_and(|p| p != source.path())
        {
            continue;
        }
        let program = crate::parse(source.source(), source.path()).map_err(|d| vec![d])?;
        if program.types.iter().any(|t| t.stable_id == options.record) {
            if selected.is_some() {
                return Err(refusal("codec explanation type identity is ambiguous"));
            }
            selected = Some((source, program));
        }
    }
    let (source, program) = selected.ok_or_else(|| {
        refusal("codec explanation type identity was not found in exact authored source")
    })?;
    if source.source().len() > MAX_SCHEMA_BYTES {
        return Err(refusal(
            "codec explanation schema source exceeds 65536 bytes",
        ));
    }
    let declaration = program
        .types
        .iter()
        .find(|t| t.stable_id == options.record)
        .expect("selected exact type");
    let target = options.target.as_deref().or_else(|| {
        revision.manifest().target_matrix().and_then(|m| {
            if m.len() == 1 {
                Some(m[0].as_str())
            } else {
                None
            }
        })
    });
    let target_policy = target.map(|t| {
        match revision
            .manifest()
            .admit_build_target(if t == "wasm32" { "wasm" } else { "native" })
        {
            Ok(()) => json!({"manifest_allows":true}),
            Err(e) => json!({"manifest_allows":false,"refusals":diagnostics(e)}),
        }
    });
    // Every profile is inspected before paging, including profiles outside
    // this page. Internal generation/parse/size failures retain their order.
    let mut rows = Vec::with_capacity(12);
    for profile in profiles(
        options.max_string_bytes.unwrap_or(1),
        options.max_array_items.unwrap_or(1),
    ) {
        rows.push(inspection::inspect(
            revision,
            options,
            &program,
            declaration,
            source.path(),
            profile,
        )?);
    }
    let envelope = json!({"schema":"semaprax.json-codec-suitability.v1","project_revision":revision.project_revision(),
        "workspace_revision":revision.workspace_revision(),"source":source.path(),"source_sha256":hash(source.source().as_bytes()),
        "type":options.record,"target":target,"target_manifest_policy":target_policy,
        "policy":{"max_string_bytes":options.max_string_bytes,"max_array_items":options.max_array_items},
        "profile_count":rows.len(),"offset":options.offset,
        "profiles":[],"next_offset":Value::Null,"coverage":"continued"});
    let count = rows.len();
    let mut selected_rows = rows.into_iter().skip(options.offset);
    paging::page(envelope, count, options.offset, options.max_bytes, |_| {
        selected_rows
            .next()
            .expect("checked sequential profile page")
            .render()
    })
}

#[cfg(test)]
mod tests;
