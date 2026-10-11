//! Ordinary Core-Wasm shared-owner consumer closure for OPT #746.
//!
//! [Owned Bytes partition](../../docs/OWNED-BYTE-PARTITION-V1.md) requires a
//! distinct authenticated Core-Wasm handle domain whose consumers close over
//! every reachable Bytes operation before any producer is admitted. Ordinary
//! Core-Wasm Bytes are host-owned tokens behind the frozen two-word carrier
//! `((0x80000000 | token) << 32) | extent`, so the domain lives host-side:
//! `shared_owner/registry.mjs` owns backing, range, generation and issuer
//! authority, and `shared_owner/host.mjs` routes every implemented ordinary
//! consumer through it. The guest never receives a pointer, offset or header,
//! and no consumer classifies a carrier by probing memory.
//!
//! This module decides, from a module's actual import section, whether every
//! import is a lease-aware consumer of that private host or carries no Bytes
//! authority. Any other crossing (public owned envelopes, providers,
//! collections, record/leaf/Box payloads and the remaining String toolkit)
//! refuses by name before a lease could reach it. Producer admission is
//! closed: [`admit_producer`] refuses until the cross-backend source/HIR join
//! supplies a validated `bytes_partition` selection, and no `env` import
//! issues a lease. Neither the closure receipt nor the private fused lease
//! proof grants that authority.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "selected only after the cross-backend bytes_partition join"
    )
)]

/// Private authority domain; distinct from every public token table.
pub(in crate::wasm) const DOMAIN: &str = "semaprax.private.wasm-shared-byte-owner.v1";
/// Authenticated lease registry: backings, ranges, generations, issuer.
pub(in crate::wasm) const REGISTRY_SOURCE: &str = include_str!("shared_owner/registry.mjs");
/// Private strict host composing the registry into ordinary consumers.
pub(in crate::wasm) const HOST_SOURCE: &str = include_str!("shared_owner/host.mjs");
/// The only namespace the ordinary emitter uses for these consumers.
const NAMESPACE: &str = "env";

/// Ordinary `env` consumers the private host resolves through the registry.
/// Each accepts a whole owner or a live lease, and rejects anything else.
pub(in crate::wasm) const LEASE_AWARE: &[&str] = &[
    "spx_bytes_zeroed",
    "spx_bytes_copy",
    "spx_bytes_get",
    "spx_bytes_as_slice",
    "spx_bytes_drop",
    "spx_bytes_set",
    "spx_bytes_set5",
    "spx_bytes_set1_or5",
    "spx_bytes_set1_or6_or48",
    "spx_string_from_utf8_v1",
    "spx_string_from_str_v2",
    "spx_string_len_chars_v1",
    "spx_string_starts_with_v1",
    "spx_string_contains_v1",
    "spx_string_compare_v2",
    "spx_vec_with_capacity_v2",
    "spx_vec_push_v2",
    "spx_vec_len_v2",
    "spx_vec_capacity_v2",
    "spx_vec_get_v2",
    "spx_vec_drop_v2",
    "spx_vec_reserve_exact_v2",
    "spx_vec_set_v2",
    "spx_vec_clear_v2",
    "spx_iter_bytes_into_v2",
    "spx_iter_bytes_next_v2",
    "spx_iter_bytes_drop_v2",
];

/// Imports that never receive or return a Bytes carrier.
const CARRIER_FREE: &[&str] = &[
    "spx_add",
    "spx_sub",
    "spx_mul",
    "spx_div",
    "spx_rem",
    "spx_neg",
    "spx_contract_fail",
];

/// Named crossings a lease must not reach until their owner joins the domain.
/// Prefix entries end in `_`.
const REFUSED: &[(&str, Crossing)] = &[
    ("spx_owned_", Crossing::PublicOwnedEnvelope),
    ("spx_status_record", Crossing::PublicOwnedEnvelope),
    ("spx_semantic_", Crossing::PublicOwnedEnvelope),
    ("spx_pg_", Crossing::Provider),
    ("spx_file_", Crossing::Provider),
    ("spx_filesystem_", Crossing::Provider),
    ("spx_https_", Crossing::Provider),
    ("spx_network_", Crossing::Provider),
    ("spx_process_", Crossing::Provider),
    ("spx_command_", Crossing::Provider),
    ("spx_environment_", Crossing::Provider),
    ("spx_collection_", Crossing::Collection),
    ("spx_format_step_v1", Crossing::Format),
    ("spx_string_", Crossing::StringToolkit),
    ("spx_vec_leaf_", Crossing::RecordPayload),
    ("spx_vec_record_", Crossing::RecordPayload),
    ("spx_iter_record_", Crossing::RecordPayload),
    ("spx_vec_sort_", Crossing::RecordPayload),
    ("spx_box_", Crossing::BoxPayload),
    ("spx_vec_", Crossing::LegacyVec),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::wasm) enum Crossing {
    /// Public owned-handle envelope; needs the copy-out/provider-drop bridge.
    PublicOwnedEnvelope,
    /// Host provider boundary; needs an owning snapshot contract.
    Provider,
    /// Map/Set arena copies keys and values outside this domain.
    Collection,
    /// Literal format worker owns its own allocation-failure lane.
    Format,
    /// String toolkit operations not yet routed through the registry.
    StringToolkit,
    /// Record/owned-leaf Vec rows whose leaf drop is host-local.
    RecordPayload,
    /// Box payload drop is host-local.
    BoxPayload,
    /// Legacy scalar-only Vec imports are not part of the private host.
    LegacyVec,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::wasm) enum Refusal {
    /// The module bytes do not parse as one Core-Wasm module.
    Malformed,
    /// A non-function import cannot be authenticated as a consumer.
    NotFunction(String),
    /// An import outside the ordinary namespace.
    Namespace(String),
    /// A named crossing a lease must not reach.
    Crossing(String, Crossing),
    /// An import no list classifies; unknown never defaults to admission.
    Unknown(String),
    /// The partition producer is not admitted on Core-Wasm.
    ProducerClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::wasm) enum Disposition {
    LeaseAware,
    CarrierFree,
}

pub(in crate::wasm) fn classify(module: &str, name: &str) -> Result<Disposition, Refusal> {
    if module != NAMESPACE {
        return Err(Refusal::Namespace(format!("{module}.{name}")));
    }
    if LEASE_AWARE.contains(&name) {
        return Ok(Disposition::LeaseAware);
    }
    if CARRIER_FREE.contains(&name) {
        return Ok(Disposition::CarrierFree);
    }
    for (pattern, crossing) in REFUSED {
        let matched = if pattern.ends_with('_') {
            name.starts_with(pattern)
        } else {
            name == *pattern
        };
        if matched {
            return Err(Refusal::Crossing(name.to_owned(), *crossing));
        }
    }
    Err(Refusal::Unknown(name.to_owned()))
}

/// Receipt that every import of one exact module is closed over the domain.
/// It is proof data about that module's imports only; it grants no producer,
/// source or public-adapter authority.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::wasm) struct Closure {
    pub(in crate::wasm) lease_aware: Vec<String>,
    pub(in crate::wasm) carrier_free: Vec<String>,
}

/// Classifies the actual import section in order and refuses at the first
/// import outside the closure.
pub(in crate::wasm) fn consumer_closure(module: &[u8]) -> Result<Closure, Refusal> {
    let mut closure = Closure {
        lease_aware: Vec::new(),
        carrier_free: Vec::new(),
    };
    for payload in wasmparser::Parser::new(0).parse_all(module) {
        let payload = payload.map_err(|_| Refusal::Malformed)?;
        if let wasmparser::Payload::ImportSection(section) = payload {
            for import in section.into_imports() {
                let import = import.map_err(|_| Refusal::Malformed)?;
                if !matches!(import.ty, wasmparser::TypeRef::Func(_)) {
                    return Err(Refusal::NotFunction(format!(
                        "{}.{}",
                        import.module, import.name
                    )));
                }
                match classify(import.module, import.name)? {
                    Disposition::LeaseAware => closure.lease_aware.push(import.name.to_owned()),
                    Disposition::CarrierFree => closure.carrier_free.push(import.name.to_owned()),
                }
            }
        }
    }
    Ok(closure)
}

/// Producer admission stays closed until the cross-backend join. A closure
/// receipt is necessary, never sufficient.
pub(in crate::wasm) fn admit_producer(_closure: &Closure) -> Result<(), Refusal> {
    Err(Refusal::ProducerClosed)
}

#[cfg(test)]
mod tests;
