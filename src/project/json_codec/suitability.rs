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
fn page(
    mut envelope: Value,
    rows: &[Value],
    offset: usize,
    maximum: usize,
) -> Result<String, Vec<Diagnostic>> {
    if offset >= rows.len() {
        return Err(refusal(
            "codec explanation offset is outside the complete profile inventory",
        ));
    }
    let mut accepted = Vec::new();
    for row in &rows[offset..] {
        accepted.push(row.clone());
        let next = offset + accepted.len();
        envelope["profiles"] = json!(accepted);
        envelope["next_offset"] = if next == rows.len() {
            Value::Null
        } else {
            json!(next)
        };
        envelope["coverage"] = json!(if next == rows.len() {
            "complete"
        } else {
            "continued"
        });
        let output =
            serde_json::to_string(&envelope).expect("fixed explanation fields serialize") + "\n";
        if output.len() > maximum {
            accepted.pop();
            break;
        }
    }
    if accepted.is_empty() {
        return Err(refusal(
            "codec explanation byte budget cannot hold one complete profile row and its binding",
        ));
    }
    let next = offset + accepted.len();
    envelope["profiles"] = json!(accepted);
    envelope["next_offset"] = if next == rows.len() {
        Value::Null
    } else {
        json!(next)
    };
    envelope["coverage"] = json!(if next == rows.len() {
        "complete"
    } else {
        "continued"
    });
    let output =
        serde_json::to_string(&envelope).expect("fixed explanation fields serialize") + "\n";
    if output.len() > maximum {
        return Err(refusal(
            "codec explanation envelope exceeds its byte budget",
        ));
    }
    Ok(output)
}

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
        if source.source().len() > MAX_SCHEMA_BYTES {
            return Err(refusal(
                "codec explanation schema source exceeds 65536 bytes",
            ));
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
    let mut rows = Vec::with_capacity(12);
    for profile in profiles(
        options.max_string_bytes.unwrap_or(1),
        options.max_array_items.unwrap_or(1),
    ) {
        let (selector, string, array) = plan::profile_fields(profile);
        let mut missing = Vec::new();
        if string.is_some() && options.max_string_bytes.is_none() {
            missing.push("--max-string-bytes 1..64");
        }
        if array.is_some() && options.max_array_items.is_none() {
            missing.push("--max-array-items 1..256");
        }
        let mut row = json!({"selector":selector,"shape_matches":Value::Null,"missing_policy":missing,
            "candidate_admission":"not_evaluated","backend_admission":"not_evaluated"});
        if row["missing_policy"]
            .as_array()
            .expect("policy list")
            .is_empty()
        {
            match profile_fragment(&program, declaration, profile) {
                Err(errors) => {
                    row["shape_matches"] = json!(false);
                    row["refusals"] = diagnostics(errors);
                }
                Ok(fragment) => {
                    if fragment.len() > MAX_GENERATED_BYTES {
                        return Err(refusal("codec explanation fragment exceeds the existing generated-source bound"));
                    }
                    let generated = crate::parse(&fragment, source.path()).map_err(|d| vec![d])?;
                    let mut modules = std::collections::BTreeSet::new();
                    let mut absent = std::collections::BTreeSet::new();
                    for import in &generated.module_uses {
                        modules.insert(import.target_module.as_str());
                        let id = crate::hir::DeclarationId::new(import.persistent_id.as_str());
                        if revision
                            .entry_program()
                            .declarations
                            .declaration(&id)
                            .is_none()
                        {
                            absent.insert(import.persistent_id.as_str());
                        }
                    }
                    row["shape_matches"] = json!(true);
                    row["required_modules"] = json!(modules);
                    row["imports_absent_from_current_entry_index"] = json!(absent);
                    row["remaining_prerequisites"]=json!(["ordinary complete candidate dependency/signature/profile/target/ownership/verification admission"]);
                }
            }
        }
        rows.push(row);
    }
    let envelope = json!({"schema":"semaprax.json-codec-suitability.v1","project_revision":revision.project_revision(),
        "workspace_revision":revision.workspace_revision(),"source":source.path(),"source_sha256":hash(source.source().as_bytes()),
        "type":options.record,"target":target,"target_manifest_policy":target_policy,
        "profile_count":rows.len(),"offset":options.offset,
        "profiles":[],"next_offset":Value::Null,"coverage":"continued"});
    page(envelope, &rows, options.offset, options.max_bytes)
}

#[cfg(test)]
mod tests;
