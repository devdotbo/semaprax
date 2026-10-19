//! Authenticated test-only repository data. No compiler or budget authority.
//! Runtime files are required at the exact compile-time repository location.
#![cfg(test)]
use std::io::Read;
#[path = "runtime_json/sealed_io.rs"]
mod sealed_io;
pub(super) use sealed_io::{additional, AdditionalFixture};
use sealed_io::{digest, read_body, read_fixture, Failure, Spec, OUTPUT_CAP};

#[path = "runtime_json/inventory.rs"]
mod inventory;
pub(super) use inventory::Fixture;
#[path = "runtime_json/membership.rs"]
mod membership;
// The sole measured-arm source switch; both transport sources are common.
#[path = "runtime_json/pair_custody.rs"]
mod pair_custody;
#[path = "runtime_json/transport_runtime.rs"]
mod selected_transport;
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
pub(super) fn pair_normalize(source: &str, current: &str) -> String {
    pair_custody::normalize(source, current)
}
pub(super) fn pair_previous(source: &str, current: &str) -> String {
    pair_custody::previous(source, current)
}

// Sealed sibling inventory only; no arbitrary path or live-source API.
// Independent complete Main8a immutable tables, through the unchanged sealed reader.
pub(super) fn main8a_metadata(fixture: super::main8a_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main8a immutable metadata")
}
pub(super) fn metadata(fixture: super::metadata_json::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated immutable source metadata")
}
// Final merge input capsules use the same fixed, bounded authenticated reader.
pub(super) fn main8a_input(fixture: super::main8a_custody::Input) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main8a input capsule")
}
// Fixed final sibling only; the original one-descriptor reader is unchanged.
pub(super) fn final_custody(fixture: super::final_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated final source custody")
}
// Sealed successor inventory; identical private descriptor reader.
pub(super) fn successor_custody(fixture: super::successor_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated successor source custody")
}
pub(super) fn prefix_custody(fixture: super::prefix_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated prefix source custody")
}
pub(super) fn joined_custody(fixture: super::joined_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated joined source custody")
}
pub(super) fn release_custody(fixture: super::release_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated release source custody")
}
pub(super) fn load(fixture: Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated immutable projection JSON fixture")
}
// The compiled index proves absence against these exact immutable fixture specs.
// An unused fixture is not opened on this route; the all-fixture owner separately
// authenticates availability, complete bytes and exact key multiplicity.
pub(super) fn project(fixture: Fixture, source: &str, current: &str) -> String {
    if membership::keys(fixture).binary_search(&source).is_err() {
        return current.to_owned();
    }
    super::project(&selected_transport::load(fixture), source, current)
}
#[path = "runtime_json/tests.rs"]
mod tests;

// Separate fixed a3 tables; the original single-fd/length/SHA/UTF8 reader is unchanged.
pub(super) fn maina3_metadata(fixture: super::maina3_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated a3 immutable metadata")
}
pub(super) fn maina3_input(fixture: super::maina3_custody::Input, source: &str) -> String {
    let spec = fixture.spec(source);
    let payload = read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated a3 input capsule");
    maina3_part_source(&payload, source);
    payload
}
fn maina3_part_source(payload: &str, source: &str) {
    let rows: serde_json::Value = serde_json::from_str(payload).expect("fixed a3 part rows");
    assert_eq!(
        rows.as_array()
            .unwrap()
            .iter()
            .filter(|row| row["source"] == source)
            .count(),
        1,
        "exact known a3 source in selected bounded part"
    );
}

// Full incoming arrays use the same sealed reader; metadata never authorizes live work.
pub(super) fn main44_metadata(fixture: super::main44_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main44 metadata")
}
pub(super) fn main44_input() -> String {
    let spec = super::main44_custody::input_spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main44 source domain")
}

// Independent fixed Main043 descriptors through the original sealed bounded reader.
pub(super) fn main043_metadata(fixture: super::main043_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main043 metadata")
}
pub(super) fn main043_input() -> String {
    let spec = super::main043_custody::input_spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main043 helper input")
}

// RUNTIME848_FIXED_RECEIVERS_BEGIN
// Two immutable test-runtime dependencies; no discovery, cache or new reader.
#[derive(Clone, Copy)]
pub(crate) enum ReceiverFixture {
    SelectedCalls,
    Main67107,
    // RUNTIME848_SIX_VARIANTS_BEGIN
    PaidLiveReuse,
    LexicalOperands,
    PrivateLayoutCapacity,
    RegistryScratch,
    ColdCanonical,
    BorrowedGeneric,
    // RUNTIME848_SIX_VARIANTS_END
    // RUNTIME848_THREE_VARIANTS_BEGIN
    RegionEntry,
    DiagnosticWriter,
    SignatureRewrite,
    // RUNTIME848_THREE_VARIANTS_END
}
impl ReceiverFixture {
    // RUNTIME848_THREE_SOURCE_BEGIN
    // This callback receives the complete compiled self-source, not a path or
    // canonical-row request. Authenticate it before the three exact transport
    // reversals and every unchanged original signature/source proof pipeline.
    pub(crate) fn signature_rewrite_source_before(current: &str) -> String {
        assert_eq!(current.len(), 28277);
        assert_eq!(
            digest(current.as_bytes()),
            [
                198, 17, 126, 152, 25, 183, 254, 171, 26, 83, 8, 219, 9, 71, 3, 107, 176, 194, 125,
                45, 139, 98, 251, 184, 50, 47, 197, 221, 238, 199, 127, 102
            ]
        );
        let mut before = current.to_owned();
        for (after, original) in [
            (
                "        let rows: serde_json::Value =\n            serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n                crate::source_reflows_merge::ReceiverFixture::SignatureRewrite,\n            ))\n            .unwrap();",
                "        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n            env!(\"CARGO_MANIFEST_DIR\"),\n            \"/docs/evidence/constructor-signature-rewrite-825/source-inverses.json\"\n        )))\n        .unwrap();",
            ),
            (
                "        let current_receiver =\n            crate::source_reflows_merge::ReceiverFixture::signature_rewrite_source_before(\n                include_str!(\"receiving.rs\"),\n            );\n",
                "",
            ),
            (
                "current_receiver.as_str()",
                "include_str!(concat!(\n                    env!(\"CARGO_MANIFEST_DIR\"),\n                    \"/src/workspace_graph/expected_projection/type_rewrite/receiving.rs\"\n                ))",
            ),
        ] {
            assert_eq!(before.matches(after).count(), 1);
            before = before.replacen(after, original, 1);
        }
        assert_eq!(before.len(), 28245);
        assert_eq!(
            digest(before.as_bytes()),
            [
                235, 243, 216, 43, 205, 12, 70, 195, 199, 184, 108, 43, 55, 194, 77, 30, 15, 148,
                46, 227, 254, 221, 174, 133, 214, 58, 237, 35, 120, 109, 199, 24
            ]
        );
        before
    }
    // RUNTIME848_THREE_SOURCE_END
    fn spec(self) -> Spec {
        match self {
            // RUNTIME848_SIX_SPECS_BEGIN
            Self::PaidLiveReuse => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-live-pending-reuse-3c351/whole-source-inverses.json"),
                length: 3576034,
                sha256: [175, 88, 101, 138, 154, 38, 252, 9, 156, 230, 194, 230, 75, 76, 213, 24, 61, 243, 206, 29, 108, 169, 97, 95, 44, 25, 206, 12, 141, 45, 83, 251],
            },
            Self::LexicalOperands => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json"),
                length: 2485760,
                sha256: [203, 244, 183, 123, 0, 154, 182, 125, 92, 251, 176, 112, 8, 30, 56, 131, 132, 45, 202, 131, 180, 69, 127, 109, 176, 70, 214, 65, 36, 223, 177, 243],
            },
            Self::PrivateLayoutCapacity => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/private-layout-entry-capacity-834/exact-source-inverses.json"),
                length: 2435619,
                sha256: [24, 156, 92, 39, 33, 34, 138, 91, 21, 161, 118, 59, 160, 15, 255, 156, 35, 72, 243, 38, 204, 56, 42, 130, 111, 149, 170, 5, 114, 223, 222, 8],
            },
            Self::RegistryScratch => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/registry-census-scratch-833/source-inverses.json"),
                length: 1887564,
                sha256: [224, 173, 90, 94, 217, 248, 71, 175, 109, 73, 51, 88, 246, 89, 174, 60, 88, 184, 158, 29, 152, 245, 12, 27, 153, 103, 24, 193, 57, 235, 3, 82],
            },
            Self::ColdCanonical => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-comment-comparison-835/source-inverses.json"),
                length: 1821023,
                sha256: [61, 179, 82, 168, 172, 142, 181, 238, 74, 100, 67, 195, 156, 217, 135, 74, 119, 95, 129, 9, 166, 51, 53, 135, 37, 142, 41, 228, 120, 212, 166, 140],
            },
            Self::BorrowedGeneric => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/borrowed-generic-type-rewrite-141387/whole-source-inverses.json"),
                length: 1498202,
                sha256: [71, 23, 207, 207, 19, 209, 138, 153, 88, 241, 15, 220, 76, 228, 59, 21, 77, 168, 152, 82, 242, 198, 111, 217, 192, 38, 52, 208, 27, 126, 215, 228],
            },
            // RUNTIME848_SIX_SPECS_END
            // RUNTIME848_THREE_SPECS_BEGIN
            Self::RegionEntry => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json"
                ),
                length: 3841085,
                sha256: [
                    218, 167, 174, 164, 251, 148, 90, 158, 250, 93, 6, 233, 174, 206, 237, 156, 6,
                    92, 144, 192, 122, 218, 66, 73, 60, 119, 244, 243, 41, 189, 21, 199,
                ],
            },
            Self::DiagnosticWriter => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/diagnostic-writer-846/source-inverses.json"
                ),
                length: 2446626,
                sha256: [
                    165, 242, 11, 99, 243, 34, 176, 86, 112, 114, 117, 252, 130, 218, 100, 219,
                    130, 146, 175, 189, 109, 65, 32, 181, 200, 188, 186, 108, 73, 86, 91, 104,
                ],
            },
            Self::SignatureRewrite => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/source-inverses.json"
                ),
                length: 2353229,
                sha256: [
                    78, 171, 172, 57, 131, 198, 179, 97, 165, 22, 99, 105, 44, 105, 216, 167, 92,
                    221, 121, 110, 125, 156, 175, 164, 99, 160, 55, 91, 22, 173, 120, 35,
                ],
            },
            // RUNTIME848_THREE_SPECS_END
            Self::SelectedCalls => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/selected-call-refusal-781/exact-source-inverses.json"
                ),
                length: 2614264,
                sha256: [
                    74, 253, 119, 191, 254, 162, 249, 198, 84, 36, 10, 18, 217, 177, 53, 7, 205,
                    95, 186, 130, 189, 110, 193, 196, 36, 139, 88, 217, 220, 125, 245, 209,
                ],
            },
            Self::Main67107 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main67107-production-composition-20261017/receiver-main12.json"
                ),
                length: 1193191,
                sha256: [
                    39, 218, 14, 63, 48, 47, 133, 120, 85, 202, 13, 224, 212, 113, 135, 75, 199,
                    127, 92, 195, 76, 251, 33, 125, 206, 26, 103, 209, 20, 222, 136, 154,
                ],
            },
        }
    }
}
pub(crate) fn receiver_fixture(fixture: ReceiverFixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated immutable receiver fixture")
}
// The original selected-call assertion audits its own complete source. Restore
// exactly the two transport edits before applying its unchanged original inverse.
pub(crate) fn receiver_fixture_source_before(source: &str, current: &str) -> String {
    // These fourteen exact entry images belong to the unchanged SelectedCalls
    // inverse packet. Acquire them only after current physical/compiled/archive
    // authentication; the original self-transport inverse below stays intact.
    let expected = match source {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => Some("d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0"),
        "src/cleanup_plan/replay/lexical_replay.rs" => Some("d0f2b7fcfc3815f0c1207c064d8bcb14d4323dff614d953988b962138ff975d0"),
        "src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => Some("4d534b5433078d8d65ec9f09c7f763614a177cd743aa02465de64c023e509173"),
        "src/cleanup_plan/replay/scalar_replay.rs" => Some("6cbb996ba104a91029e7fb323ad2801c483d6f71b4b19ef10fd097a16f0c764a"),
        "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs" => Some("2b27003287b4e325ed126b4e26f9f933fc039e42f66739242b63759d3d713b05"),
        "src/cleanup_plan/replay/semantic_refusal.rs" => Some("fc1a6f41a225e0936ede78ff9923eb0eea1e93ce5c8e37fbde9709e6ee78c823"),
        "src/cleanup_plan/replay/semantic_refusal/tests.rs" => Some("aca69e92356ddfb4d7bbb774ca647a981cf86d402d9b0dff71108adef777b1ad"),
        "src/cleanup_plan/replay/structural_control.rs" => Some("84c9d537b31a2f24e0f4c977ea0341bf4a4a6a738c299f5e7e35976a4946a037"),
        "src/cleanup_plan/replay/structural_control/blocks.rs" => Some("be348bfc7e713d79dccb11d6022a7371a3419390703aac7e667ae613f591b725"),
        "src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs" => Some("93a5af9fc855967b0fd48573c0dd7ff8075ef63cda3ab8e9a28787d6be47ea04"),
        "src/cleanup_plan/replay/structural_control/calls/custody_tests.rs" => Some("55245329e3a803f31cbdf5488c4068934f1b74ac03cfca84bbf5eb6d93f53dc7"),
        "src/cleanup_plan/replay/structural_control/calls.rs" => Some("17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000"),
        "tests/hir_module_boundaries.rs" => Some("055a8f7ddd8b9f5b2d12122154de053a0a6823b75c298580cd4770628c4e8851"),
        "tests/workspace/semantic_graph/source_inventory.rs" => Some("4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14"),
        _ => None,
    };
    if let Some(expected) = expected {
        let entry = super::main_e177_final_inputs::value_direct(source, expected, current);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(digest(entry.as_bytes()))
            ),
            expected,
            "complete SelectedCalls entry: {source}"
        );
        return entry;
    }
    if source != "src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 7873);
    assert_eq!(digest(current.as_bytes()), [22, 25, 246, 163, 6, 93, 58, 87, 222, 52, 59, 43, 203, 215, 114, 78, 99, 244, 200, 168, 70, 222, 252, 141, 0, 68, 117, 28, 248, 37, 90, 5]);
    assert_eq!(current.matches("        crate::source_reflows_merge::receiver_fixture_source_before(\n            \"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\",\n            include_str!(\"../../blocks_before_receipt.rs\"),\n        ).replacen(").count(), 1);
    let prior = current.replacen("        crate::source_reflows_merge::receiver_fixture_source_before(\n            \"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\",\n            include_str!(\"../../blocks_before_receipt.rs\"),\n        ).replacen(", "        include_str!(\"../../blocks_before_receipt.rs\").replacen(", 1);
    assert_eq!(prior.len(), 7706);
    assert_eq!(digest(prior.as_bytes()), [131, 224, 221, 224, 214, 14, 199, 238, 82, 83, 218, 38, 95, 37, 112, 115, 28, 115, 130, 137, 146, 83, 41, 253, 48, 76, 196, 9, 53, 64, 39, 52]);
    let current = prior.as_str();
    let mut before = current.to_owned();
    assert_eq!(before.matches("    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::SelectedCalls,\n        ))\n        .unwrap();").count(), 1, "exact runtime fixture transport edit");
    before = before.replacen("    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::SelectedCalls,\n        ))\n        .unwrap();", "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../../docs/evidence/selected-call-refusal-781/exact-source-inverses.json\"\n    ))\n    .unwrap();", 1);
    assert_eq!(before.matches("        let mut current = crate::source_reflows_merge::receiver_fixture_source_before(name, source);").count(), 1, "exact runtime fixture transport edit");
    before = before.replacen("        let mut current = crate::source_reflows_merge::receiver_fixture_source_before(name, source);", "        let mut current = source.to_owned();", 1);
    assert_eq!(
        digest(before.as_bytes()),
        [
            122, 25, 222, 23, 184, 16, 230, 100, 106, 120, 205, 160, 9, 162, 204, 93, 38, 84, 161,
            101, 31, 187, 150, 206, 28, 92, 42, 51, 124, 152, 232, 137
        ],
        "complete original selected-call source"
    );
    before
}
