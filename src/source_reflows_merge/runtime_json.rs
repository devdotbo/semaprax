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
}
impl ReceiverFixture {
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
    if source != "src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs" {
        return current.to_owned();
    }
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
