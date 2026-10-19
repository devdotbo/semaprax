use super::*;
use std::io::{Cursor, Error, ErrorKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn authenticate_keys(fixture: Fixture, rows: &serde_json::Value) {
    let mut actual = rows
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["source"].as_str())
        .collect::<Vec<_>>();
    actual.sort();
    // Keep duplicates: presence selects the unchanged projector, which still
    // refuses more than one matching row. Never silently deduplicate evidence.
    assert_eq!(actual.as_slice(), membership::keys(fixture));
    for key in &actual {
        assert!(membership::keys(fixture).binary_search(key).is_ok());
    }
    assert!(membership::keys(fixture).binary_search(&"\0").is_err());
    assert_eq!(project(fixture, "\0", "unchanged"), "unchanged");
}
#[test]
fn fixed_projection_json_transport_refuses_hostile_io_before_parsing() {
    let shared = include_str!("sealed_io.rs");
    assert_eq!(
        super::super::main_e177_final_inputs::value_direct(
            "src/source_reflows_merge/runtime_json/sealed_io.rs",
            "bef85d126a5ce9c72f6f74835d9966d60d7b9bff6622302d4df864adf1701fd0",
            shared,
        ),
        shared,
        "complete shared sealed reader before all original I/O hostiles",
    );
    // Both exact selector arms normalize to the identical common source before
    // any historical source inverse. No arbitrary edited source is accepted.
    let authenticated = super::super::main_e177_final_inputs::value_direct(
        "src/source_reflows_merge/runtime_json.rs",
        "5a95fa1f07fac99c62f6572817df247695a4d2e274d5accc5ce231d7dc150bc4",
        include_str!("../runtime_json.rs"),
    );
    let authenticated = complete_preformat_transport(&authenticated);
    let current = super::super::release_current_input(
        "src/source_reflows_merge/runtime_json.rs",
        &authenticated,
    );
    let release = super::super::release_custody::input(
        super::super::release_custody::Fixture::Previous2c,
        "src/source_reflows_merge/runtime_json.rs",
        &current,
    );
    let joined =
        super::super::joined_custody::input("src/source_reflows_merge/runtime_json.rs", &release);
    let prefix_view = super::super::prefix_custody::input(
        super::super::prefix_custody::Fixture::SealedB5,
        "src/source_reflows_merge/runtime_json.rs",
        &joined,
    );
    let successor = super::super::successor_custody::input(
        super::super::successor_custody::Fixture::Sealed724,
        "src/source_reflows_merge/runtime_json.rs",
        &prefix_view,
    );
    let final_view = super::super::final_custody::input(
        super::super::final_custody::Fixture::Common,
        "src/source_reflows_merge/runtime_json.rs",
        &successor,
    );
    let arm_view = super::super::metadata_json::previous(
        "src/source_reflows_merge/runtime_json.rs",
        &final_view,
    );
    let arm = arm_view.as_str();
    let common = pair_custody::normalize("src/source_reflows_merge/runtime_json.rs", arm);
    let embedded = common.replace(
        "runtime_json/transport_runtime.rs",
        "runtime_json/transport_embedded.rs",
    );
    assert_eq!(
        pair_custody::normalize("src/source_reflows_merge/runtime_json.rs", &embedded),
        common
    );
    assert!(std::panic::catch_unwind(|| {
        pair_custody::normalize(
            "src/source_reflows_merge/runtime_json.rs",
            &(common.clone() + " "),
        )
    })
    .is_err());
    let path = std::env::temp_dir().join(format!(
        "semaprax-projection-json-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    // Acquire cleanup ownership only after successful creation. Never remove a
    // pre-existing directory when creation itself fails.
    std::fs::create_dir(&path).unwrap();
    let directory = Directory(path);
    let path = directory.0.join("fixture.json");
    let bytes = b"[{\"source\":\"exact\"}]";
    let sha = digest(bytes);
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        read_fixture(&path, bytes.len(), sha).unwrap().as_bytes(),
        bytes
    );
    assert_eq!(
        read_body(&mut Cursor::new(bytes), bytes.len(), sha)
            .unwrap()
            .as_bytes(),
        bytes
    );
    // Check every original allowlisted repository fixture without retaining a
    // cache or multiple payloads. A missing/drifted fixture must fail this owner.
    let custody = load(Fixture::Custody);
    assert_eq!(selected_transport::load(Fixture::Custody), custody);
    let custody: serde_json::Value = serde_json::from_str(&custody).unwrap();
    assert_eq!(custody.as_array().unwrap().len(), 6);
    authenticate_keys(Fixture::Custody, &custody);
    for fixture in inventory::ORIGINAL_FIXTURES {
        let payload = load(fixture);
        assert_eq!(selected_transport::load(fixture), payload);
        let rows: serde_json::Value = serde_json::from_str(&payload).unwrap();
        authenticate_keys(fixture, &rows);
    }
    // New bindings traverse the unchanged reader before their owning JSON/source
    // assertions. Every original fixture and all hostile-I/O controls stay above
    // and below this loop unchanged.
    for (fixture, rows) in [
        (AdditionalFixture::HistoricalCompact7, 7),
        (AdditionalFixture::HistoricalReceivers12, 12),
        (AdditionalFixture::OrdinaryPlanHistory, 6),
        (AdditionalFixture::OrdinaryPlanIntegrated, 6),
    ] {
        let spec = fixture.spec();
        let payload = additional(fixture);
        assert_eq!(payload.len(), spec.length);
        assert_eq!(digest(payload.as_bytes()), spec.sha256);
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), rows);
    }
    // Compiled presence must reach the original duplicate-row refusal, never
    // choose one row or turn a duplicate into an unchanged-input success.
    let duplicate = r#"[{"source":"same"},{"source":"same"}]"#;
    assert!(
        std::panic::catch_unwind(|| { super::super::project(duplicate, "same", "unchanged") })
            .is_err()
    );
    // Ten distinct hostile boundaries. None hands unauthenticated data to serde.
    assert_eq!(
        read_fixture(&directory.0.join("missing"), bytes.len(), sha),
        Err(Failure::Input)
    );
    assert!(matches!(
        read_fixture(&directory.0, bytes.len(), sha),
        Err(Failure::Input | Failure::Bound)
    ));
    assert_eq!(
        read_fixture(&path, OUTPUT_CAP + 1, sha),
        Err(Failure::Bound)
    );
    assert_eq!(
        read_fixture(&path, bytes.len() + 1, sha),
        Err(Failure::Bound)
    );
    assert_eq!(
        read_fixture(&path, bytes.len() - 1, sha),
        Err(Failure::Bound)
    );
    let mut altered = bytes.to_vec();
    altered[0] ^= 1;
    std::fs::write(&path, &altered).unwrap();
    assert_eq!(read_fixture(&path, bytes.len(), sha), Err(Failure::Digest));
    let invalid = [0xff_u8];
    std::fs::write(&path, invalid).unwrap();
    assert_eq!(
        read_fixture(&path, invalid.len(), digest(&invalid)),
        Err(Failure::Utf8)
    );
    assert_eq!(
        read_body(
            &mut Cursor::new(&bytes[..bytes.len() - 1]),
            bytes.len(),
            sha
        ),
        Err(Failure::Read)
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        read_body(&mut Cursor::new(trailing), bytes.len(), sha),
        Err(Failure::Trailing)
    );
    struct RefuseEof(Cursor<&'static [u8]>);
    impl Read for RefuseEof {
        #[allow(
            clippy::io_other_error,
            reason = "The complete saved control retains its original expressions for independent parity checks."
        )]
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            if self.0.position() as usize == self.0.get_ref().len() {
                Err(Error::new(ErrorKind::Other, "refused EOF check"))
            } else {
                self.0.read(output)
            }
        }
    }
    assert_eq!(
        read_body(&mut RefuseEof(Cursor::new(bytes)), bytes.len(), sha),
        Err(Failure::Read)
    );
}

#[test]
fn maina3_parts_cover_every_fixed_key_once_under_the_original_reader_cap() {
    use super::super::maina3_custody::Input;
    for fixture in [
        Input::Previous16c,
        Input::Maina3,
        Input::Alias1ae,
        Input::MainE47,
    ] {
        let mut opened = std::collections::BTreeSet::new();
        let mut complete = Vec::new();
        for source in fixture.keys() {
            let spec = fixture.spec(source);
            assert!(spec.length <= OUTPUT_CAP);
            if !opened.insert(spec.path) {
                continue;
            }
            let text = maina3_input(fixture, source);
            assert_eq!(text.len(), spec.length);
            let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
            for row in rows.as_array().unwrap() {
                let key = row["source"].as_str().unwrap();
                assert!(fixture.keys().binary_search(&key).is_ok());
                assert_eq!(fixture.spec(key).path, spec.path);
                complete.push(key.to_owned());
            }
        }
        complete.sort();
        assert_eq!(complete, fixture.keys());
    }
}

#[test]
fn maina3_known_part_source_refuses_gaps_duplicates_and_foreign_rows() {
    for payload in [
        "[]",
        r#"[{"source":"foreign"}]"#,
        r#"[{"source":"known"},{"source":"known"}]"#,
        "{}",
        "not json",
    ] {
        assert!(std::panic::catch_unwind(|| maina3_part_source(payload, "known")).is_err());
    }
    maina3_part_source(r#"[{"source":"known"}]"#, "known");
}

// RUNTIME848_RECEIVER_CONTROLS_BEGIN
#[test]
fn fixed_receiver_json_fixtures_keep_complete_inputs_inverses_and_hostile_io() {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-receiver-json-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let path = directory.0.join("fixture.json");
    for (fixture, expected_rows) in [
        (ReceiverFixture::SelectedCalls, 18),
        (ReceiverFixture::Main67107, 12),
    ] {
        let spec = fixture.spec();
        assert!(spec.length <= OUTPUT_CAP);
        let text = receiver_fixture(fixture);
        assert_eq!(text.len(), spec.length);
        assert_eq!(digest(text.as_bytes()), spec.sha256);
        let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), expected_rows);
        std::fs::write(&path, text.as_bytes()).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256).unwrap(), text);
        assert_eq!(
            read_fixture(&directory.0.join("missing"), spec.length, spec.sha256),
            Err(Failure::Input)
        );
        std::fs::write(&path, &text.as_bytes()[..spec.length - 1]).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        let mut extra = text.as_bytes().to_vec();
        extra.push(0);
        std::fs::write(&path, &extra).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        assert_eq!(
            read_body(&mut Cursor::new(&extra), spec.length, spec.sha256),
            Err(Failure::Trailing)
        );
        extra.truncate(spec.length);
        extra[0] ^= 1;
        std::fs::write(&path, &extra).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Digest)
        );
    }
    let name = "src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs";
    let current =
        include_str!("../../cleanup_plan/replay/structural_control/calls/custody_tests/source.rs");
    let original = receiver_fixture_source_before(name, current);
    assert!(std::panic::catch_unwind(|| receiver_fixture_source_before(name, &original)).is_err());
    assert!(std::panic::catch_unwind(|| receiver_fixture_source_before(
        name,
        &(current.to_owned() + " ")
    ))
    .is_err());
    let runtime = include_str!("../runtime_json.rs");
    let (runtime_before, _) = runtime
        .split_once("\n// RUNTIME848_FIXED_RECEIVERS_BEGIN\n")
        .unwrap();
    assert_eq!(
        digest(runtime_before.as_bytes()),
        [
            148, 112, 193, 25, 208, 50, 49, 30, 155, 150, 103, 198, 66, 216, 71, 82, 129, 20, 180,
            161, 173, 36, 105, 237, 225, 125, 49, 19, 77, 149, 155, 236
        ]
    );
    let mut root_before = complete_receiver_entry_glue(
        "src/source_reflows_merge.rs",
        include_str!("../../source_reflows_merge.rs"),
    );
    assert_eq!(&root_before[685..867], ";\n// Closed authenticated repository DATA only; no live-source or compiler authority.\npub(crate) use runtime_json::{receiver_fixture, receiver_fixture_source_before, ReceiverFixture}");
    root_before.replace_range(685..867, "");
    assert_eq!(
        digest(root_before.as_bytes()),
        [
            47, 199, 81, 107, 241, 8, 163, 83, 135, 195, 164, 227, 44, 126, 21, 210, 102, 90, 21,
            60, 154, 237, 154, 234, 103, 203, 114, 174, 218, 132, 112, 225
        ]
    );
    let mut receiver_before = complete_receiver_entry_glue(
        "src/source_reflows/incoming67107_semantic_receivers.rs",
        include_str!("../../source_reflows/incoming67107_semantic_receivers.rs"),
    );
    assert_eq!(receiver_before.matches("fn proofs() -> &'static [serde_json::Value] {\n    // Preserve the old parsed metadata cache, but authenticate the runtime\n    // dependency on every reached lookup, including an already warm cache.\n    let payload = crate::source_reflows_merge::receiver_fixture(\n        crate::source_reflows_merge::ReceiverFixture::Main67107,\n    );\n    PROOF_RECORDS.get_or_init(|| {\n        let mut original: Vec<serde_json::Value> = serde_json::from_str(&payload).unwrap();\n        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();\n        assert_eq!(original.len(), 12);\n        assert_eq!(additional.len(), 2);\n        original.extend(additional);\n        original\n    })\n}\n").count(), 1);
    receiver_before = receiver_before.replacen("fn proofs() -> &'static [serde_json::Value] {\n    // Preserve the old parsed metadata cache, but authenticate the runtime\n    // dependency on every reached lookup, including an already warm cache.\n    let payload = crate::source_reflows_merge::receiver_fixture(\n        crate::source_reflows_merge::ReceiverFixture::Main67107,\n    );\n    PROOF_RECORDS.get_or_init(|| {\n        let mut original: Vec<serde_json::Value> = serde_json::from_str(&payload).unwrap();\n        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();\n        assert_eq!(original.len(), 12);\n        assert_eq!(additional.len(), 2);\n        original.extend(additional);\n        original\n    })\n}\n", "fn proofs() -> &'static [serde_json::Value] {\n    PROOF_RECORDS.get_or_init(|| {\n        let mut original: Vec<serde_json::Value> = serde_json::from_str(PROOFS).unwrap();\n        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();\n        assert_eq!(original.len(), 12);\n        assert_eq!(additional.len(), 2);\n        original.extend(additional);\n        original\n    })\n}\n", 1);
    assert_eq!(receiver_before.matches("const LIVE_AUDITS:").count(), 1);
    receiver_before = receiver_before.replacen("const LIVE_AUDITS:", "const PROOFS: &str = include_str!(\n    \"../../docs/evidence/main67107-production-composition-20261017/receiver-main12.json\"\n);\nconst LIVE_AUDITS:", 1);
    assert_eq!(
        digest(receiver_before.as_bytes()),
        [
            39, 65, 174, 128, 29, 207, 74, 77, 46, 65, 104, 18, 87, 107, 203, 246, 159, 102, 233,
            74, 214, 151, 70, 234, 129, 152, 154, 85, 79, 28, 253, 63
        ]
    );
}

// RUNTIME848_SIX_PACKET_CONTROLS_BEGIN
#[test]
fn six_large_historical_packets_keep_full_inputs_whole_consumers_and_fresh_hostile_reads() {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-six-history-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let path = directory.0.join("fixture.json");
    let mut total = 0;
    for (fixture, expected_rows, source, current, before, after, whole_before) in [
        (ReceiverFixture::PaidLiveReuse, 3, "src/loan_plan/paid_live/reuse_tests.rs",
            include_str!("../../loan_plan/paid_live/reuse_tests.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../docs/evidence/paid-live-pending-reuse-3c351/whole-source-inverses.json\"\n    ))\n    .unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::PaidLiveReuse,\n        ))\n        .unwrap();", [216, 109, 245, 106, 145, 100, 59, 79, 208, 189, 243, 178, 186, 250, 188, 116, 114, 193, 253, 203, 236, 111, 186, 54, 140, 212, 192, 81, 111, 43, 75, 79]),
        (ReceiverFixture::LexicalOperands, 14, "src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs",
            include_str!("../../cleanup_plan/replay/lexical_replay/operand_tests/source.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json\"\n    )).unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::LexicalOperands,\n        ))\n        .unwrap();", [87, 95, 116, 97, 251, 102, 36, 248, 223, 223, 30, 213, 17, 203, 173, 225, 34, 122, 71, 53, 138, 189, 8, 88, 117, 83, 150, 217, 73, 226, 147, 27]),
        (ReceiverFixture::PrivateLayoutCapacity, 9, "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs",
            include_str!("../../workspace_graph/expected_projection/private_layout/capacity_tests/source.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../docs/evidence/private-layout-entry-capacity-834/exact-source-inverses.json\"\n    ))\n    .unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::PrivateLayoutCapacity,\n        ))\n        .unwrap();", [203, 76, 112, 158, 205, 13, 137, 102, 236, 3, 201, 204, 157, 102, 100, 104, 24, 40, 97, 222, 216, 203, 12, 134, 14, 210, 12, 53, 48, 0, 100, 226]),
        (ReceiverFixture::RegistryScratch, 5, "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs",
            include_str!("../../workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs"),
            "    let packet: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../docs/evidence/registry-census-scratch-833/source-inverses.json\"\n    ))\n    .unwrap();", "    let packet: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::RegistryScratch,\n        ))\n        .unwrap();", [11, 243, 168, 104, 18, 92, 93, 56, 30, 173, 59, 103, 18, 127, 98, 32, 133, 216, 251, 160, 155, 35, 134, 45, 182, 139, 218, 209, 60, 39, 30, 198]),
        (ReceiverFixture::ColdCanonical, 3, "src/workspace_graph/cold_canonical/tests.rs",
            include_str!("../../workspace_graph/cold_canonical/tests.rs"),
            "    let packet: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../docs/evidence/canonical-comment-comparison-835/source-inverses.json\"\n    ))\n    .unwrap();", "    let packet: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::ColdCanonical,\n        ))\n        .unwrap();", [236, 194, 160, 122, 195, 102, 173, 234, 221, 201, 134, 197, 191, 28, 103, 96, 0, 233, 94, 201, 143, 93, 213, 38, 195, 124, 120, 246, 84, 6, 214, 44]),
        (ReceiverFixture::BorrowedGeneric, 3, "src/workspace_graph/generic_type_import/borrowed_tests.rs",
            include_str!("../../workspace_graph/generic_type_import/borrowed_tests.rs"),
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../docs/evidence/borrowed-generic-type-rewrite-141387/whole-source-inverses.json\"\n    ))\n    .unwrap();", "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::BorrowedGeneric,\n        ))\n        .unwrap();", [211, 49, 200, 85, 196, 27, 40, 98, 133, 132, 189, 37, 118, 201, 60, 15, 114, 34, 110, 208, 123, 235, 28, 254, 37, 39, 46, 23, 138, 10, 155, 49]),
    ] {
        let spec = fixture.spec();
        assert!(spec.length <= OUTPUT_CAP);
        let payload = receiver_fixture(fixture);
        assert_eq!((payload.len(), digest(payload.as_bytes())), (spec.length, spec.sha256));
        total += payload.len();
        let packet: serde_json::Value = serde_json::from_str(&payload).unwrap();
        let rows = packet.as_array().unwrap_or_else(|| packet["owners"].as_array().unwrap());
        assert_eq!(rows.len(), expected_rows);
        let names: std::collections::BTreeSet<_> = rows.iter().map(|row| row["source"].as_str().unwrap()).collect();
        assert_eq!(names.len(), expected_rows, "complete unique source inventory");
        let entry = complete_receiver_entry_glue(source, current);
        let current = entry.as_str();
        assert_eq!(current.matches(after).count(), 1, "one transport change: {source}");
        let original = current.replacen(after, before, 1);
        assert_eq!(digest(original.as_bytes()), whole_before, "entire original consumer: {source}");
        assert_eq!(original.matches(before).count(), 1);
        assert_eq!(original.replacen(before, after, 1), current, "whole forward transport: {source}");
        assert_ne!(digest((original + " ").as_bytes()), whole_before);

        // Warm a valid read, then mutate the SAME path. Every read authenticates
        // a newly opened regular handle; no parsed or successful-verdict cache.
        std::fs::write(&path, payload.as_bytes()).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256).unwrap(), payload);
        assert_eq!(read_fixture(&directory.0.join("missing"), spec.length, spec.sha256), Err(Failure::Input));
        std::fs::write(&path, &payload.as_bytes()[..spec.length-1]).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256), Err(Failure::Bound));
        let mut changed = payload.as_bytes().to_vec(); changed.push(0);
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256), Err(Failure::Bound));
        assert_eq!(read_body(&mut Cursor::new(&changed), spec.length, spec.sha256), Err(Failure::Trailing));
        changed.truncate(spec.length); changed[0] ^= 1;
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256), Err(Failure::Digest));
        std::fs::write(&path, payload.as_bytes()).unwrap();
        assert_eq!(read_fixture(&path, spec.length, spec.sha256).unwrap(), payload);
    }
    assert_eq!(
        total, 13_704_202,
        "six complete physical inputs, not measured compiler memory"
    );
    let runtime = include_str!("../runtime_json.rs");
    let mut original = complete_receiver_entry_glue("src/source_reflows_merge/runtime_json.rs", runtime);
    for (start, end) in [
        (
            "    // RUNTIME848_SIX_VARIANTS_BEGIN\n",
            "    // RUNTIME848_SIX_VARIANTS_END\n",
        ),
        (
            "            // RUNTIME848_SIX_SPECS_BEGIN\n",
            "            // RUNTIME848_SIX_SPECS_END\n",
        ),
    ] {
        assert_eq!(original.matches(start).count(), 1);
        assert_eq!(original.matches(end).count(), 1);
        let at = original.find(start).unwrap();
        let stop = original.find(end).unwrap() + end.len();
        original.replace_range(at..stop, "");
    }
    assert_eq!(
        digest(original.as_bytes()),
        [
            213, 63, 228, 97, 226, 49, 216, 191, 160, 97, 207, 139, 31, 227, 30, 112, 155, 50, 194,
            254, 15, 162, 234, 74, 121, 193, 148, 191, 8, 98, 94, 67
        ],
        "whole pre-six runtime reader facade"
    );
    let mut original_tests = include_str!("tests.rs")
        .split_once("\n// RUNTIME848_SIX_PACKET_CONTROLS_BEGIN\n")
        .unwrap()
        .0
        .to_owned();
    // The two finite ENTRY acquisitions below preserve the complete old test
    // body after their exact source-only glue is removed, before the unchanged
    // formatter/transport inverse. The helper itself is outside this prefix.
    for (after, before) in [
        ("    let mut root_before = complete_receiver_entry_glue(\n        \"src/source_reflows_merge.rs\",\n        include_str!(\"../../source_reflows_merge.rs\"),\n    );", "    let mut root_before = include_str!(\"../../source_reflows_merge.rs\").to_owned();"),
        ("    let mut receiver_before = complete_receiver_entry_glue(\n        \"src/source_reflows/incoming67107_semantic_receivers.rs\",\n        include_str!(\"../../source_reflows/incoming67107_semantic_receivers.rs\"),\n    );", "    let mut receiver_before =\n        include_str!(\"../../source_reflows/incoming67107_semantic_receivers.rs\").to_owned();"),
    ] {
        assert_eq!(original_tests.matches(after).count(), 1);
        original_tests = original_tests.replacen(after, before, 1);
    }
    let bridge = "    let authenticated = complete_preformat_transport(&authenticated);\n";
    assert_eq!(original_tests.matches(bridge).count(), 1);
    original_tests = original_tests.replacen(bridge, "", 1);
    // Exact formatter-only inverse; preserve every complete preexisting assertion.
    let formatted = "    assert!(std::panic::catch_unwind(|| {\n        pair_custody::normalize(\n            \"src/source_reflows_merge/runtime_json.rs\",\n            &(common.clone() + \" \"),\n        )\n    })\n    .is_err());\n";
    let prior = "    assert!(\n        std::panic::catch_unwind(|| {\n            pair_custody::normalize(\n                \"src/source_reflows_merge/runtime_json.rs\",\n                &(common.clone() + \" \"),\n            )\n        })\n        .is_err()\n    );\n";
    assert_eq!(original_tests.matches(formatted).count(), 1);
    original_tests = original_tests.replacen(formatted, prior, 1);
    assert_eq!(
        digest(original_tests.as_bytes()),
        [
            51, 227, 120, 141, 142, 179, 169, 142, 247, 90, 121, 64, 112, 162, 124, 75, 28, 22, 29,
            92, 76, 36, 44, 27, 69, 214, 243, 50, 225, 111, 97, 243
        ],
        "all prior runtime fixture controls retained"
    );
}

// The current-first reader above acquires the complete formatted transport.
// Restore only its exact compatibility annotation and four formatter hunks
// before replaying all unchanged independent historical transport controls.
fn complete_preformat_transport(current: &str) -> String {
    assert_eq!(current.len(), 8140);
    assert_eq!(digest(current.as_bytes()), [90, 149, 250, 31, 7, 250, 201, 156, 98, 246, 87, 40, 23, 223, 36, 118, 149, 164, 210, 226, 116, 213, 172, 204, 92, 226, 49, 215, 220, 21, 11, 196]);
    let mut original = current.to_owned();
    for (after, before) in [
        ("#[allow(\n    dead_code,\n    reason = \"Retained compatibility entry for independently compiled historical and semantic controls.\"\n)]\n", ""),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main44 metadata\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main44 metadata\")\n"),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main44 source domain\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main44 source domain\")\n"),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main043 metadata\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main043 metadata\")\n"),
        ("    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)\n        .expect(\"complete authenticated Main043 helper input\")\n", "    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect(\"complete authenticated Main043 helper input\")\n"),

    ] {
        assert_eq!(original.matches(after).count(), 1);
        original = original.replacen(after, before, 1);
    }
    assert_eq!(original.len(), 7972);
    assert_eq!(digest(original.as_bytes()), [134, 67, 1, 208, 184, 127, 141, 233, 92, 107, 141, 31, 47, 211, 6, 195, 187, 150, 234, 201, 47, 86, 105, 152, 99, 8, 6, 95, 42, 189, 174, 239]);
    original
}

// Exact approved ENTRY-glue inverse, separate from every original runtime-data
// transport inverse. Full compiled bytes are bound before any literal edit;
// no archived source is substituted and no old assertion/hash is retargeted.
#[rustfmt::skip]
fn complete_receiver_entry_glue(source: &str, current: &str) -> String {
    let current = complete_receiver_followup_glue(source, current);
    let current = current.as_str();
    let (length, current_sha, old_length, old_sha, edits): (usize, [u8; 32], usize, [u8; 32], &[(&str, &str)]) = match source {
        "src/source_reflows_merge.rs" => (
            54109, [220, 244, 224, 223, 238, 175, 36, 141, 79, 190, 146, 91, 141, 163, 27, 142, 129, 150, 45, 135, 252, 98, 241, 82, 51, 110, 137, 62, 124, 114, 39, 213],
            47396, [103, 201, 154, 111, 183, 144, 217, 242, 102, 241, 45, 9, 134, 5, 177, 82, 54, 234, 142, 160, 125, 200, 148, 230, 143, 61, 21, 37, 149, 251, 239, 223],
            &[
                ("}\n\n// These three formatter inverses bind the ENTIRE actual compiled child body,\n// not merely an archived before image. No token or string-content normalization.\npub(crate) fn receiver_child_entry(source: &str, current: &str) -> String {\n    let (expected, length, compiled_sha, after, before) = match source {\n        \"src/workspace_graph/expected_projection/private_layout/before_registry.rs\" => (\n            \"0d6e2e30c78d040eafcd7c11e66a4696d17e949aa09eac7d79d6d1b6634721cd\", 8142,\n            \"986c2f1eb41715186ee5a1856e58abbcde03d53e86e53b1e903e1b0845c002b8\",\n            \"}\\n\\n#[cfg(test)]\\nthread_local! {static DERIVATIONS:\",\n            \"}\\n\\n\\n#[cfg(test)]\\nthread_local! {static DERIVATIONS:\",\n        ),\n        \"src/workspace_graph/cold_canonical/before.rs\" => (\n            \"78fbcd95f4430f0750b4ae1e8b2cfdc5277c3bf9ef02fe4776cea96bc5a494e2\", 19289,\n            \"cd153b0db9d19b6b8983fc66f3970494d1247c2180b595d839db96eeaf4f8487\",\n            \"                    staged!(\\n                        CorePrebound,\\n                        constructor_failure::attempt::reserve(resolve_builder_bytes)\\n                    )?;\",\n            \"                    staged!(CorePrebound, constructor_failure::attempt::reserve(resolve_builder_bytes))?;\",\n        ),\n        \"src/format/comments/compare/tests.rs\" => (\n            \"7adedbc8fc515199d21e71a93fbe7a1624c5500e46278f4ddf834050ef0a7b38\", 3421,\n            \"4b422dd4781d2123eddd2eabb1459c5e3bedb222fc6957729d4c444e3ca2cf2c\",\n            \"    assert_eq!(\\n        refusal.message,\\n        \\\"a module must declare at least one function\\\"\\n    );\",\n            \"    assert_eq!(refusal.message, \\\"a module must declare at least one function\\\");\",\n        ),\n        _ => return current.to_owned(),\n    };\n    // Fresh physical/compiled/archive authentication precedes historical lookup.\n    let entry = main_e177_final_inputs::value_direct(source, expected, current);\n    assert_eq!(\n        current.len(),\n        length,\n        \"complete compiled child extent: {source}\"\n    );\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))\n        ),\n        compiled_sha,\n        \"complete compiled child bytes: {source}\"\n    );\n    assert_eq!(\n        current.matches(after).count(),\n        1,\n        \"one literal child formatter inverse: {source}\"\n    );\n    let inverse = current.replacen(after, before, 1);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(inverse.as_bytes()))\n        ),\n        expected,\n        \"complete original compiled child: {source}\"\n    );\n    assert_eq!(\n        inverse, entry,\n        \"whole compiled child equals independently acquired history: {source}\"\n    );\n    inverse\n}\n\npub(crate) fn consumer_child(domain: &str, source: &str, current: &str) -> String {\n    let entry = receiver_child_entry(source, current);\n    let current = entry.as_str();\n    let branch = final_fix_input(source, current);\n    let current = branch.as_str();\n", "}\n\npub(crate) fn consumer_child(domain: &str, source: &str, current: &str) -> String {\n    let branch = final_fix_input(source, current);\n    let current = branch.as_str();\n"),
                ("    runtime_json::project(runtime_json::Fixture::F16, source, current)\n}\n// Closed packet/source pairs only. This acquires the original consumer INPUT,\n// never its final inverse output; all caller hooks and full-byte checks remain.\npub(crate) fn receiver_fixture_entry(\n    fixture: ReceiverFixture,\n    source: &str,\n    current: &str,\n) -> String {\n    let expected = match (fixture, source) {\n        (ReceiverFixture::PaidLiveReuse, \"src/loan_plan/paid_live.rs\") => \"bcb520a64f14a76f2bac1e6053ce40f424d15e11e8b1e70f8c67d4406c450374\",\n        (ReceiverFixture::PaidLiveReuse, \"src/loan_plan/paid_plan.rs\") => \"0eb3df7863a61410f5a4547831aa399ffd03d460dad86f1f40caf626de0f2be7\",\n        (ReceiverFixture::PaidLiveReuse, \"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs\") => \"d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0\",\n        (ReceiverFixture::BorrowedGeneric, \"src/workspace_graph/generic_type_import.rs\") => \"e4aaa275d0b6d79539a30b496844388ba9fc3b4b793499b907b803f759ee8dc9\",\n        (ReceiverFixture::BorrowedGeneric, \"src/workspace_graph/expected_projection/cost.rs\") => \"ddf8dc2e74e4c0cc193e4e592671ffcfd9118b8bfe91cef2d0cf19eca3a635ce\",\n        (ReceiverFixture::BorrowedGeneric, \"tests/workspace/semantic_graph/source_inventory.rs\") => \"4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14\",\n        (ReceiverFixture::LexicalOperands, \"src/cleanup_plan/replay/expression_facts/receiving.rs\") => \"db822f51812769eadf1570bfee4b315ceadd31b689fc6efa643e5da03d7a9bb7\",\n        (ReceiverFixture::LexicalOperands, \"src/cleanup_plan/replay/expression_facts/receiving/directory.rs\") => \"d161622003e500b3b9197c0f463b88eee388d9447c5a87ba75f464d3aa2a59cc\",\n        _ => return current.to_owned(),\n    };\n    let entry = main_e177_final_inputs::value_direct(source, expected, current);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))\n        ),\n        expected,\n        \"complete fixed receiver input: {source}\"\n    );\n    entry\n}\n\npub(crate) fn consumer_input(domain: &str, source: &str, current: &str) -> String {\n    let entry = if domain == \"lexical832\" {\n        receiver_fixture_entry(ReceiverFixture::LexicalOperands, source, current)\n    } else {\n        current.to_owned()\n    };\n    let current = entry.as_str();\n    let predecessor = corrected_consumers_previous(source, current);\n    let current = predecessor.as_str();\n", "    runtime_json::project(runtime_json::Fixture::F16, source, current)\n}\npub(crate) fn consumer_input(domain: &str, source: &str, current: &str) -> String {\n    let predecessor = corrected_consumers_previous(source, current);\n    let current = predecessor.as_str();\n"),
                ("        return current.to_owned();\n    };\n    let entry = historical_project_entry(source, row[\"current_sha256\"].as_str().unwrap(), current);\n    let acquired = main67107_inputs::compile35f_expected_input(\n        source,\n        row[\"current_sha256\"].as_str().unwrap(),\n        &entry,\n    );\n    let current = acquired.as_str();\n", "        return current.to_owned();\n    };\n    let acquired = main67107_inputs::compile35f_expected_input(\n        source,\n        row[\"current_sha256\"].as_str().unwrap(),\n        current,\n    );\n    let current = acquired.as_str();\n"),
                ("    metadata_json::project(metadata_json::Fixture::M24, source, current)\n}\n// Only literal historical ENTRY stages are acquired here. The complete existing\n// compile35f front and every intermediate inverse below still execute in order.\nfn historical_project_entry(source: &str, expected: &str, current: &str) -> String {\n    let admitted = matches!(\n        (source, expected),\n        (\n            \"src/cleanup_plan/replay/live_oracle/source_custody/b.rs\",\n            \"ca0e42fbd3f14f29ff59b7dc4c6bff26a364a1e305967bcf548970a53774a9ca\"\n        ) | (\n            \"src/workspace_graph.rs\",\n            \"b8d71709bbe9961b338552bba4f6098e59c8d78c5da7ca5f8ff4157b36ea3f31\"\n        ) | (\n            \"tests/workspace/semantic_graph/source_inventory.rs\",\n            \"aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989\"\n        ) | (\n            \"src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs\",\n            \"0bf3a868125c5d381ead3b67127f622085d8fba09b23862db68bdad13c271ec6\"\n        ) | (\n            \"src/workspace_graph/cold_canonical/tests.rs\",\n            \"ecc2a07ac366adeaddc986c5bf1c676000e95ec98f5dd526c37c78f65406d62c\"\n        )\n    );\n    if !admitted {\n        return current.to_owned();\n    }\n    let entry = main_e177_final_inputs::value_direct(source, expected, current);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))\n        ),\n        expected,\n        \"complete fixed historical projector entry: {source}\"\n    );\n    entry\n}\nfn project(rows: &str, source: &str, current: &str) -> String {\n    let rows: serde_json::Value = serde_json::from_str(rows).expect(\"fixed merge projections\");\n", "    metadata_json::project(metadata_json::Fixture::M24, source, current)\n}\nfn project(rows: &str, source: &str, current: &str) -> String {\n    let rows: serde_json::Value = serde_json::from_str(rows).expect(\"fixed merge projections\");\n"),
            ],
        ),
        "src/source_reflows/incoming67107_semantic_receivers.rs" => (
            36321, [138, 197, 176, 188, 130, 183, 72, 150, 237, 233, 145, 134, 57, 185, 6, 147, 13, 52, 132, 244, 187, 179, 145, 232, 28, 116, 228, 171, 81, 196, 145, 42],
            34859, [122, 20, 238, 148, 109, 68, 228, 36, 72, 199, 176, 105, 238, 13, 170, 26, 53, 222, 218, 229, 46, 140, 122, 153, 140, 186, 216, 180, 59, 55, 149, 232],
            &[
                ("\npub(super) fn invert_ordinary(proof: &serde_json::Value, current: &str) -> String {\n    // Only these six exact Main67107 entry tuples changed after this packet.\n    // The full existing MAIN21 acquisition and every inverse below still run.\n    let path = proof[\"path\"].as_str().unwrap();\n    let expected = proof[\"after_sha256\"].as_str().unwrap();\n    let entry = match (path, expected) {\n        (\n            \"src/cleanup_plan/replay.rs\",\n            \"30616d71694d393a730c0c8e49a25bbad7539b07ff96ed901b9d07be29765b9c\",\n        )\n        | (\n            \"tests/hir_module_boundaries.rs\",\n            \"f25b784eba1a2ea26a07b260b2ddff67b6dd90c0a6402fd143dc42ca68847d4e\",\n        )\n        | (\n            \"tests/workspace/semantic_graph/source_inventory.rs\",\n            \"59e5b98593c2c2d443d1479e7c89564551d515ca921f7abab2cc8f5d579c8c21\",\n        )\n        | (\n            \"src/source_verify/declaration.rs\",\n            \"c93ac4c883fe6b10c831a2603c994ea03880986fd0b7a394bfa41ec8a06d88cb\",\n        )\n        | (\n            \"src/cleanup_plan/replay/branch_clone/tests.rs\",\n            \"46bf0451ef390225f540d031f326100c7f4288cf7e9d50e30ce7ddb68a448e0f\",\n        )\n        | (\n            \"src/cleanup_plan/replay/factored.rs\",\n            \"343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6\",\n        ) => Some(\n            crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                path, expected, current,\n            ),\n        ),\n        _ => None,\n    };\n    let acquired = super::semantic_receivers::incoming_main21::ours_expected_input(\n        proof,\n        entry.as_deref().unwrap_or(current),\n    );\n    let current = acquired.as_str();\n    let path = proof[\"path\"].as_str().unwrap();\n", "\npub(super) fn invert_ordinary(proof: &serde_json::Value, current: &str) -> String {\n    let acquired = super::semantic_receivers::incoming_main21::ours_expected_input(proof, current);\n    let current = acquired.as_str();\n    let path = proof[\"path\"].as_str().unwrap();\n"),
            ],
        ),
        "src/source_reflows_merge/runtime_json.rs" => (
            14714, [167, 211, 139, 31, 139, 210, 147, 221, 132, 58, 13, 174, 159, 107, 39, 111, 55, 78, 173, 69, 151, 130, 153, 143, 180, 1, 53, 67, 17, 2, 106, 99],
            12620, [42, 11, 178, 164, 170, 141, 211, 74, 103, 86, 92, 35, 245, 71, 60, 68, 151, 176, 99, 37, 230, 112, 28, 63, 135, 141, 195, 223, 24, 241, 103, 67],
            &[
                ("// exactly the two transport edits before applying its unchanged original inverse.\npub(crate) fn receiver_fixture_source_before(source: &str, current: &str) -> String {\n    // These ten exact entry images belong to the unchanged SelectedCalls\n    // inverse packet. Acquire them only after current physical/compiled/archive\n    // authentication; the original self-transport inverse below stays intact.\n    let expected = match source {\n        \"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs\" => Some(\"d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0\"),\n        \"src/cleanup_plan/replay/lexical_replay.rs\" => Some(\"d0f2b7fcfc3815f0c1207c064d8bcb14d4323dff614d953988b962138ff975d0\"),\n        \"src/cleanup_plan/replay/live_oracle/source_custody/b.rs\" => Some(\"4d534b5433078d8d65ec9f09c7f763614a177cd743aa02465de64c023e509173\"),\n        \"src/cleanup_plan/replay/scalar_replay.rs\" => Some(\"6cbb996ba104a91029e7fb323ad2801c483d6f71b4b19ef10fd097a16f0c764a\"),\n        \"src/cleanup_plan/replay/selected_entry/tests/session_errors.rs\" => Some(\"2b27003287b4e325ed126b4e26f9f933fc039e42f66739242b63759d3d713b05\"),\n        \"src/cleanup_plan/replay/semantic_refusal.rs\" => Some(\"fc1a6f41a225e0936ede78ff9923eb0eea1e93ce5c8e37fbde9709e6ee78c823\"),\n        \"src/cleanup_plan/replay/semantic_refusal/tests.rs\" => Some(\"aca69e92356ddfb4d7bbb774ca647a981cf86d402d9b0dff71108adef777b1ad\"),\n        \"src/cleanup_plan/replay/structural_control/calls.rs\" => Some(\"17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000\"),\n        \"tests/hir_module_boundaries.rs\" => Some(\"055a8f7ddd8b9f5b2d12122154de053a0a6823b75c298580cd4770628c4e8851\"),\n        \"tests/workspace/semantic_graph/source_inventory.rs\" => Some(\"4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14\"),\n        _ => None,\n    };\n    if let Some(expected) = expected {\n        let entry = super::main_e177_final_inputs::value_direct(source, expected, current);\n        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(digest(entry.as_bytes()))\n            ),\n            expected,\n            \"complete SelectedCalls entry: {source}\"\n        );\n        return entry;\n    }\n    if source != \"src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs\" {\n        return current.to_owned();\n", "// exactly the two transport edits before applying its unchanged original inverse.\npub(crate) fn receiver_fixture_source_before(source: &str, current: &str) -> String {\n    if source != \"src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs\" {\n        return current.to_owned();\n"),
            ],
        ),
        "src/loan_plan/paid_live/reuse_tests.rs" => (
            23070, [11, 82, 121, 23, 118, 57, 133, 4, 26, 80, 11, 217, 215, 7, 45, 35, 141, 33, 3, 215, 144, 161, 83, 129, 160, 44, 213, 202, 5, 239, 120, 144],
            22879, [29, 31, 28, 104, 175, 197, 177, 253, 65, 121, 126, 1, 143, 180, 108, 206, 220, 42, 28, 236, 134, 227, 71, 220, 92, 77, 126, 203, 41, 245, 228, 215],
            &[
                ("    ];\n    for (index, (input, original)) in inputs.into_iter().enumerate() {\n        let mut restored = crate::source_reflows_merge::receiver_fixture_entry(\n            crate::source_reflows_merge::ReceiverFixture::PaidLiveReuse,\n            rows[index][\"source\"].as_str().unwrap(),\n            input,\n        );\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n", "    ];\n    for (index, (input, original)) in inputs.into_iter().enumerate() {\n        let mut restored = input.to_owned();\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n"),
            ],
        ),
        "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs" => (
            17919, [167, 225, 70, 248, 156, 144, 177, 202, 203, 7, 163, 153, 113, 231, 213, 31, 225, 235, 50, 184, 117, 51, 5, 208, 146, 182, 174, 231, 22, 39, 200, 144],
            17756, [77, 94, 167, 131, 177, 133, 95, 203, 10, 215, 155, 188, 10, 160, 19, 67, 13, 68, 6, 235, 139, 180, 191, 109, 225, 22, 105, 251, 8, 138, 93, 80],
            &[
                ("    );\n    assert_eq!(\n        crate::source_reflows_merge::receiver_child_entry(\n            \"src/workspace_graph/expected_projection/private_layout/before_registry.rs\",\n            include_str!(\"../before_registry.rs\"),\n        ),\n        old_registry\n            .replace(\n", "    );\n    assert_eq!(\n        include_str!(\"../before_registry.rs\"),\n        old_registry\n            .replace(\n"),
            ],
        ),
        "src/workspace_graph/cold_canonical/tests.rs" => (
            16089, [159, 134, 251, 182, 196, 13, 82, 133, 209, 58, 210, 185, 14, 176, 152, 96, 236, 187, 130, 146, 185, 74, 163, 104, 168, 144, 183, 195, 233, 215, 28, 198],
            15976, [253, 251, 107, 203, 202, 134, 102, 142, 252, 216, 93, 192, 84, 194, 137, 134, 174, 45, 98, 185, 238, 250, 155, 161, 133, 185, 224, 145, 21, 45, 31, 202],
            &[
                ("        1,\n    );\n    assert_eq!(crate::source_reflows_merge::receiver_child_entry(\n        \"src/workspace_graph/cold_canonical/before.rs\", include_str!(\"before.rs\")\n    ),format!(\"//! Complete immutable940 cold preflight; no comparison-sink delegation.\\nuse super::super::*;\\n{body}\\n{wrapper}\"));\n    assert_eq!(\n        include_str!(\"../../format.rs\"),\n", "        1,\n    );\n    assert_eq!(include_str!(\"before.rs\"),format!(\"//! Complete immutable940 cold preflight; no comparison-sink delegation.\\nuse super::super::*;\\n{body}\\n{wrapper}\"));\n    assert_eq!(\n        include_str!(\"../../format.rs\"),\n"),
            ],
        ),
        "src/workspace_graph/generic_type_import/borrowed_tests.rs" => (
            22118, [202, 187, 147, 95, 149, 53, 150, 5, 112, 15, 89, 9, 253, 39, 249, 230, 219, 114, 190, 79, 195, 123, 12, 163, 6, 10, 89, 51, 38, 71, 247, 45],
            21925, [122, 22, 60, 168, 39, 48, 188, 186, 198, 227, 197, 115, 185, 6, 70, 61, 229, 178, 34, 40, 205, 46, 52, 195, 119, 176, 189, 129, 168, 145, 134, 244],
            &[
                ("    ];\n    for (index, (input, before)) in inputs.into_iter().enumerate() {\n        let mut restored = crate::source_reflows_merge::receiver_fixture_entry(\n            crate::source_reflows_merge::ReceiverFixture::BorrowedGeneric,\n            rows[index][\"source\"].as_str().unwrap(),\n            input,\n        );\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n", "    ];\n    for (index, (input, before)) in inputs.into_iter().enumerate() {\n        let mut restored = input.to_owned();\n        for hook in rows[index][\"hooks\"].as_array().unwrap() {\n            let after = hook[\"after\"].as_str().unwrap();\n"),
            ],
        ),
        // The other two six-packet consumers are byteexact already; their
        // unchanged whole original transport checks below remain mandatory.
        _ => return current.to_owned(),
    };
    assert_eq!(current.len(), length, "complete compiled ENTRY-glue extent: {source}");
    assert_eq!(digest(current.as_bytes()), current_sha, "complete compiled ENTRY-glue input: {source}");
    let mut original = current.to_owned();
    for &(after, before) in edits {
        assert_eq!(original.matches(after).count(), 1, "one fixed ENTRY-glue hunk: {source}");
        original = original.replacen(after, before, 1);
    }
    assert_eq!(original.len(), old_length, "whole original ENTRY extent: {source}");
    assert_eq!(digest(original.as_bytes()), old_sha, "whole original ENTRY bytes: {source}");
    original
}

// Later ENTRY and compiled-receiver repairs bind each whole current input before
// restoring its complete predecessor. Every original transport proof still runs.
#[rustfmt::skip]
fn complete_receiver_followup_glue(source: &str, current: &str) -> String {
    let three_before = complete_three_fixture_runtime_glue(source, current);
    let current = three_before.as_str();
    let (length, current_sha, old_length, old_sha, edits): (usize, [u8; 32], usize, [u8; 32], &[(&str, &str)]) = match source {
        "src/source_reflows_merge/runtime_json.rs" => (
            16413, [115, 31, 40, 89, 151, 87, 160, 144, 206, 18, 193, 29, 161, 8, 204, 10, 234, 91, 62, 62, 24, 10, 192, 187, 90, 213, 109, 83, 22, 95, 253, 236],
            14714, [167, 211, 139, 31, 139, 210, 147, 221, 132, 58, 13, 174, 159, 107, 39, 111, 55, 78, 173, 69, 151, 130, 153, 143, 180, 1, 53, 67, 17, 2, 106, 99],
            &[
                ("    // These fourteen exact entry images belong to the unchanged SelectedCalls\n", "    // These ten exact entry images belong to the unchanged SelectedCalls\n"),
                ("        \"src/cleanup_plan/replay/structural_control.rs\" => Some(\"84c9d537b31a2f24e0f4c977ea0341bf4a4a6a738c299f5e7e35976a4946a037\"),\n        \"src/cleanup_plan/replay/structural_control/blocks.rs\" => Some(\"be348bfc7e713d79dccb11d6022a7371a3419390703aac7e667ae613f591b725\"),\n        \"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\" => Some(\"93a5af9fc855967b0fd48573c0dd7ff8075ef63cda3ab8e9a28787d6be47ea04\"),\n        \"src/cleanup_plan/replay/structural_control/calls/custody_tests.rs\" => Some(\"55245329e3a803f31cbdf5488c4068934f1b74ac03cfca84bbf5eb6d93f53dc7\"),\n", ""),
                ("    assert_eq!(current.len(), 7873);\n    assert_eq!(digest(current.as_bytes()), [22, 25, 246, 163, 6, 93, 58, 87, 222, 52, 59, 43, 203, 215, 114, 78, 99, 244, 200, 168, 70, 222, 252, 141, 0, 68, 117, 28, 248, 37, 90, 5]);\n    assert_eq!(current.matches(\"        crate::source_reflows_merge::receiver_fixture_source_before(\\n            \\\"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\\\",\\n            include_str!(\\\"../../blocks_before_receipt.rs\\\"),\\n        ).replacen(\").count(), 1);\n    let prior = current.replacen(\"        crate::source_reflows_merge::receiver_fixture_source_before(\\n            \\\"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\\\",\\n            include_str!(\\\"../../blocks_before_receipt.rs\\\"),\\n        ).replacen(\", \"        include_str!(\\\"../../blocks_before_receipt.rs\\\").replacen(\", 1);\n    assert_eq!(prior.len(), 7706);\n    assert_eq!(digest(prior.as_bytes()), [131, 224, 221, 224, 214, 14, 199, 238, 82, 83, 218, 38, 95, 37, 112, 115, 28, 115, 130, 137, 146, 83, 41, 253, 48, 76, 196, 9, 53, 64, 39, 52]);\n    let current = prior.as_str();\n", ""),
            ],
        ),
        "src/source_reflows/incoming67107_semantic_receivers.rs" => (
            36765, [222, 83, 109, 234, 39, 106, 78, 24, 1, 168, 223, 6, 247, 196, 92, 190, 217, 22, 177, 184, 97, 231, 199, 133, 154, 47, 4, 14, 227, 119, 89, 200],
            36321, [138, 197, 176, 188, 130, 183, 72, 150, 237, 233, 145, 134, 57, 185, 6, 147, 13, 52, 132, 244, 187, 179, 145, 232, 28, 116, 228, 171, 81, 196, 145, 42],
            &[
                ("    // Only these nine exact Main67107 entry tuples changed after this packet.\n", "    // Only these six exact Main67107 entry tuples changed after this packet.\n"),
                ("        )\n        | (\n            \"src/cleanup_plan/replay/factored/oracle_hook.rs\",\n            \"0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae\",\n        )\n        | (\n            \"src/hir/validation.rs\",\n            \"f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba\",\n        )\n        | (\n            \"src/hir/expr_nodes.rs\",\n            \"e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f\",\n", ""),
            ],
        ),
        "src/loan_plan/paid_live/reuse_tests.rs" => (
            25265, [103, 34, 182, 254, 37, 184, 65, 0, 103, 25, 244, 122, 254, 182, 97, 13, 24, 128, 252, 94, 221, 111, 122, 114, 55, 223, 218, 122, 63, 22, 171, 181],
            23070, [11, 82, 121, 23, 118, 57, 133, 4, 26, 80, 11, 217, 215, 7, 45, 35, 141, 33, 3, 215, 144, 161, 83, 129, 160, 44, 213, 202, 5, 239, 120, 144],
            &[
                ("    let old_live = original_test_receiver(\n        \"src/loan_plan/paid_live/original.rs\",\n        include_str!(\"original.rs\"),\n    );\n", "    let old_live = include_str!(\"original.rs\");\n"),
                ("    let mut before_facade = original_test_receiver(\n        \"src/loan_plan/paid_live/funded_before.rs\",\n        include_str!(\"funded_before.rs\"),\n    );\n", "    let mut before_facade = include_str!(\"funded_before.rs\").to_owned();\n"),
                ("\n// Only test-module lint/namespace glue separates these compiled receivers from\n// their complete original bodies. Authenticate current bytes before inversion.\nfn original_test_receiver(source: &str, compiled: &str) -> String {\n    use sha2::{Digest, Sha256};\n    let (length, current_hash, count, schema_import, old_length, old_hash) = match source {\n        \"src/loan_plan/paid_live/original.rs\" => (\n            2893,\n            \"53f1bf0c833c8bf88d391938c958e05b4210a78f7a26fd787cd7e06e0f27c210\",\n            1,\n            false,\n            2786,\n            \"9315163a396ddc3f3905dc8b56dd220ccbb6d852dd270d18f59a171308af4c26\",\n        ),\n        \"src/loan_plan/paid_live/funded_before.rs\" => (\n            3628,\n            \"9934c14303d48cf9f221065b9525628edc69919a65e8002e85444d03d7eee918\",\n            3,\n            true,\n            3264,\n            \"8acbf4f2aef5cb26524261c8ac27a9d0b6adce6334d43e11eb972fb88ef61401\",\n        ),\n        _ => panic!(\"unknown complete receiver source\"),\n    };\n    let mut original = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n        source,\n        current_hash,\n        compiled,\n    );\n    assert_eq!(original.len(), length);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))\n        ),\n        current_hash\n    );\n    let lint = \"#[allow(\\n    clippy::duplicate_mod,\\n    reason = \\\"Run the same controls against independent receivers.\\\"\\n)]\\n\";\n    assert_eq!(original.matches(lint).count(), count);\n    original = original.replace(lint, \"\");\n    if schema_import {\n        let import = \"use crate::loan_plan::LOAN_PLAN_SCHEMA_V3;\\n\";\n        assert_eq!(original.matches(import).count(), 1);\n        original = original.replacen(import, \"\", 1);\n    }\n    assert_eq!(original.len(), old_length);\n    assert_eq!(\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))\n        ),\n        old_hash\n    );\n    original\n}\n", ""),
            ],
        ),
        "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs" => (
            6885, [76, 102, 69, 3, 184, 194, 148, 15, 156, 53, 25, 82, 77, 128, 201, 165, 143, 35, 79, 141, 189, 185, 115, 132, 56, 221, 165, 239, 246, 253, 30, 221],
            4326, [89, 47, 249, 155, 25, 144, 0, 165, 41, 230, 143, 57, 233, 30, 152, 111, 193, 51, 135, 23, 24, 150, 88, 137, 35, 31, 1, 57, 255, 106, 87, 94],
            &[
                ("        let entry = capacity_receiver_entry(path, source);\n        let projected = crate::source_reflows_merge::consumer_input(\"capacity834\", path, &entry);\n", "        let projected = crate::source_reflows_merge::consumer_input(\"capacity834\", path, source);\n"),
                ("    let before = capacity_receiver_entry(\n        \"src/workspace_graph/expected_projection/private_layout/capacity_before.rs\",\n        include_str!(\"../capacity_before.rs\"),\n    );\n", "    let before = include_str!(\"../capacity_before.rs\");\n"),
                ("\n// Closed whole-input acquisition precedes every unchanged capacity inverse.\nfn capacity_receiver_entry(source: &str, compiled: &str) -> String {\n    use sha2::{Digest, Sha256};\n    let digest = |s: &str| {\n        format!(\n            \"{:x}\",\n            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))\n        )\n    };\n    match source {\n        \"src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs\" => {\n            let prior = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                source,\n                \"592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e\",\n                compiled,\n            );\n            assert_eq!(prior.len(), 4326);\n            assert_eq!(\n                digest(&prior),\n                \"592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e\"\n            );\n            prior\n        }\n        \"src/workspace_graph/expected_projection/private_layout/capacity_before.rs\" => {\n            let current = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                source,\n                \"e0dfdd0ec25d52c0855f8c53027e1c4b23fbe8f7d24b045d5f0d34f1ee91ccff\",\n                compiled,\n            );\n            assert_eq!(current.len(), 3983);\n            assert_eq!(\n                digest(&current),\n                \"e0dfdd0ec25d52c0855f8c53027e1c4b23fbe8f7d24b045d5f0d34f1ee91ccff\"\n            );\n            let formatted = \"use super::{\\n    debit, explicit_only, ledger, names, owner_index, private_name, refusal,\\n    reserve_builder_structure, sorting, visit, Absence, AuthoredDeclaration, BTreeMap, Diagnostic,\\n    Entry, Frame, LayoutClosure, Program, DEPTH,\\n};\\n\";\n            let original = \"use super::{Absence, AuthoredDeclaration, BTreeMap, DEPTH, Diagnostic, Entry, Frame, LayoutClosure, Program, debit, explicit_only, ledger, names, owner_index, private_name, refusal, reserve_builder_structure, sorting, visit};\\n\";\n            assert_eq!(current.matches(formatted).count(), 1);\n            let prior = current.replacen(formatted, original, 1);\n            assert_eq!(prior.len(), 3968);\n            assert_eq!(\n                digest(&prior),\n                \"8f3117e619ea89862c658ef2104c50022c05a0876298737f8446e0168e32e75d\"\n            );\n            prior\n        }\n        _ => compiled.to_owned(),\n    }\n}\n", ""),
            ],
        ),
        "src/source_reflows_merge.rs" => (
            54430, [0, 181, 145, 52, 193, 99, 14, 239, 19, 193, 225, 223, 200, 51, 127, 154, 104, 73, 110, 244, 166, 26, 14, 164, 50, 98, 245, 250, 210, 209, 190, 29],
            54109, [220, 244, 224, 223, 238, 175, 36, 141, 79, 190, 146, 91, 141, 163, 27, 142, 129, 150, 45, 135, 252, 98, 241, 82, 51, 110, 137, 62, 124, 114, 39, 213],
            &[
                ("        ) | (\n            \"src/cleanup_plan/replay/lexical_replay.rs\",\n            \"52a1b94530881bd2f59d873a71663fcd1656a9a2401440bc4f8edb69839fbd7f\"\n        ) | (\n            \"src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs\",\n            \"575f7461fb6624f8dfdf1ed511cbade1227a47358abd0858755396d949e2931b\"\n", ""),
            ],
        ),
        _ => return current.to_owned(),
    };
    assert_eq!(current.len(), length, "complete later ENTRY helper extent: {source}");
    assert_eq!(digest(current.as_bytes()), current_sha, "complete later ENTRY helper source: {source}");
    let mut before = current.to_owned();
    for (after, prior) in edits {
        assert_eq!(before.matches(after).count(), 1, "exact later ENTRY helper edit: {source}");
        before = before.replacen(after, prior, 1);
    }
    assert_eq!(before.len(), old_length, "complete prior ENTRY helper extent: {source}");
    assert_eq!(digest(before.as_bytes()), old_sha, "complete prior ENTRY helper body: {source}");
    before
}

// RUNTIME848_THREE_CONTROLS_BEGIN
// Only these fixed DATA acquisitions change. Every historical source assertion
// below receives its complete original input; nothing is read under a compiler
// allocation or budget probe and no parsed/verdict cache is introduced.
fn complete_three_fixture_runtime_glue(source: &str, current: &str) -> String {
    let restored = complete_embedded_fixture_parent_before(source, current);
    let current = restored.as_str();
    let restored = complete_projection_entry_followup_glue(source, current);
    let current = restored.as_str();
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 20520);
    assert_eq!(
        digest(current.as_bytes()),
        [
            128, 135, 79, 202, 72, 83, 188, 103, 239, 228, 64, 98, 116, 61, 101, 98, 170, 240, 248,
            47, 111, 143, 17, 194, 137, 222, 39, 131, 196, 126, 8, 85
        ]
    );
    let mut before = current.to_owned();
    for (start, end) in [
        (
            "    // RUNTIME848_THREE_VARIANTS_BEGIN\n",
            "    // RUNTIME848_THREE_VARIANTS_END\n",
        ),
        (
            "            // RUNTIME848_THREE_SPECS_BEGIN\n",
            "            // RUNTIME848_THREE_SPECS_END\n",
        ),
        (
            "    // RUNTIME848_THREE_SOURCE_BEGIN\n",
            "    // RUNTIME848_THREE_SOURCE_END\n",
        ),
    ] {
        assert_eq!(before.matches(start).count(), 1);
        assert_eq!(before.matches(end).count(), 1);
        let first = before.find(start).unwrap();
        let last = before.find(end).unwrap() + end.len();
        assert!(first < last);
        before.replace_range(first..last, "");
    }
    assert_eq!(before.len(), 16413);
    assert_eq!(
        digest(before.as_bytes()),
        [
            115, 31, 40, 89, 151, 87, 160, 144, 206, 18, 193, 29, 161, 8, 204, 10, 234, 91, 62, 62,
            24, 10, 192, 187, 90, 213, 109, 83, 22, 95, 253, 236
        ]
    );
    before
}

fn three_fixture_test_source_before(current: &str) -> String {
    let marker = "\n// RUNTIME848_THREE_CONTROLS_BEGIN\n";
    assert_eq!(current.matches(marker).count(), 1);
    assert!(current.ends_with("// RUNTIME848_THREE_CONTROLS_END\n"));
    let (original, _) = current.split_once(marker).unwrap();
    let mut before = original.to_owned();
    let hook = "    let three_before = complete_three_fixture_runtime_glue(source, current);\n    let current = three_before.as_str();\n";
    assert_eq!(before.matches(hook).count(), 1);
    before = before.replacen(hook, "", 1);
    assert_eq!(before.len(), 62885);
    assert_eq!(
        digest(before.as_bytes()),
        [
            181, 109, 123, 172, 36, 153, 196, 35, 87, 18, 76, 84, 94, 250, 107, 184, 83, 146, 44,
            213, 28, 171, 77, 54, 40, 235, 237, 68, 179, 186, 120, 89
        ]
    );
    before
}

#[test]
fn three_runtime_packets_keep_whole_consumers_and_refuse_each_fresh_hostile_read() {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-three-history-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let path = directory.0.join("fixture.json");
    for (fixture, row_count, current, length, sha, old_length, old_sha, edits) in [
        (
            ReceiverFixture::RegionEntry,
            6,
            include_str!(
                "../../cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs"
            ),
            22278,
            [
                97, 105, 89, 24, 61, 78, 70, 62, 242, 87, 227, 223, 90, 253, 33, 23, 14, 43, 174,
                114, 178, 200, 144, 81, 63, 167, 81, 105, 255, 173, 118, 45,
            ],
            22284,
            [
                231, 253, 211, 212, 17, 178, 96, 176, 103, 228, 106, 74, 97, 180, 107, 36, 70, 160,
                211, 7, 12, 100, 113, 117, 59, 87, 200, 121, 61, 251, 98, 168,
            ],
            &[(
                "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::RegionEntry,\n        ))\n        .unwrap();",
                "    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json\"\n    )))\n    .unwrap();",
            )][..],
        ),
        (
            ReceiverFixture::DiagnosticWriter,
            10,
            include_str!("../../diagnostic/writer/tests.rs"),
            12196,
            [
                190, 196, 111, 129, 149, 10, 242, 87, 98, 82, 198, 98, 135, 70, 2, 108, 198, 53,
                196, 200, 24, 87, 21, 100, 62, 176, 202, 77, 21, 123, 146, 235,
            ],
            12183,
            [
                155, 180, 211, 126, 240, 24, 153, 35, 56, 69, 157, 144, 92, 117, 88, 31, 126, 66,
                20, 135, 80, 136, 13, 252, 139, 246, 134, 243, 251, 103, 100, 240,
            ],
            &[(
                "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::DiagnosticWriter,\n        ))\n        .unwrap();",
                "    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/diagnostic-writer-846/source-inverses.json\"\n    )))\n    .unwrap();",
            )][..],
        ),
        (
            ReceiverFixture::SignatureRewrite,
            7,
            include_str!("../../workspace_graph/expected_projection/type_rewrite/receiving.rs"),
            28277,
            [
                198, 17, 126, 152, 25, 183, 254, 171, 26, 83, 8, 219, 9, 71, 3, 107, 176, 194, 125,
                45, 139, 98, 251, 184, 50, 47, 197, 221, 238, 199, 127, 102,
            ],
            28245,
            [
                235, 243, 216, 43, 205, 12, 70, 195, 199, 184, 108, 43, 55, 194, 77, 30, 15, 148,
                46, 227, 254, 221, 174, 133, 214, 58, 237, 35, 120, 109, 199, 24,
            ],
            &[
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
            ][..],
        ),
    ] {
        let source = match fixture {
            ReceiverFixture::RegionEntry => "src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs",
            ReceiverFixture::DiagnosticWriter => "src/diagnostic/writer/tests.rs",
            ReceiverFixture::SignatureRewrite => "src/workspace_graph/expected_projection/type_rewrite/receiving.rs",
            _ => unreachable!(),
        };
        let prior = embedded_acquisition_source_before(source, current)
            .expect("each whole compiled consumer has one closed restored acquisition");
        assert!(std::panic::catch_unwind(|| embedded_acquisition_source_before(source, &prior)).is_err(),
            "already-runtime is not the new embedded current source");
        assert!(embedded_acquisition_source_before("foreign/source.rs", current).is_none());
        for foreign in [format!("{current} "), current[..current.len() - 1].to_owned(),
            current.replacen("use ", "use  ", 1)] {
            assert_ne!(foreign, current);
            assert!(std::panic::catch_unwind(|| embedded_acquisition_source_before(source, &foreign)).is_err());
        }
        let restored = complete_projection_entry_followup_glue(source, current);
        let current = restored.as_str();
        let spec = fixture.spec();
        assert!(spec.length <= OUTPUT_CAP);
        let payload = receiver_fixture(fixture);
        assert_eq!(payload.len(), spec.length);
        assert_eq!(digest(payload.as_bytes()), spec.sha256);
        let rows: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), row_count);
        assert_eq!(current.len(), length);
        assert_eq!(digest(current.as_bytes()), sha);
        let restore = |subject: &str| {
            assert_eq!(subject.len(), length);
            assert_eq!(digest(subject.as_bytes()), sha);
            let mut before = subject.to_owned();
            for &(after, prior) in edits {
                assert_eq!(before.matches(after).count(), 1);
                before = before.replacen(after, prior, 1);
            }
            assert_eq!(before.len(), old_length);
            assert_eq!(digest(before.as_bytes()), old_sha);
            before
        };
        drop(restore(current));
        for foreign in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            current.replacen("use ", "use  ", 1),
        ] {
            assert_ne!(foreign, current);
            assert!(std::panic::catch_unwind(|| restore(&foreign)).is_err());
        }
        assert_eq!(
            read_fixture(&directory.0.join("missing"), spec.length, spec.sha256),
            Err(Failure::Input)
        );
        std::fs::write(&path, &payload).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256).unwrap(),
            payload
        );
        std::fs::write(&path, &payload.as_bytes()[..spec.length - 1]).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        let mut altered = payload.as_bytes().to_vec();
        altered.push(b' ');
        std::fs::write(&path, &altered).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Bound)
        );
        altered.pop();
        altered[0] ^= 1;
        std::fs::write(&path, &altered).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256),
            Err(Failure::Digest)
        );
        std::fs::write(&path, &payload).unwrap();
        assert_eq!(
            read_fixture(&path, spec.length, spec.sha256).unwrap(),
            payload
        );
    }
    let signature =
        include_str!("../../workspace_graph/expected_projection/type_rewrite/receiving.rs");
    drop(ReceiverFixture::signature_rewrite_source_before(signature));
    for foreign in [
        format!("{signature} "),
        signature.replacen("use ", "uSe ", 1),
    ] {
        assert_ne!(foreign, signature);
        assert!(
            std::panic::catch_unwind(|| ReceiverFixture::signature_rewrite_source_before(&foreign))
                .is_err()
        );
    }
    let runtime = include_str!("../runtime_json.rs");
    drop(complete_three_fixture_runtime_glue(
        "src/source_reflows_merge/runtime_json.rs",
        runtime,
    ));
    assert!(
        std::panic::catch_unwind(|| complete_three_fixture_runtime_glue(
            "src/source_reflows_merge/runtime_json.rs",
            &format!("{runtime} "),
        ))
        .is_err()
    );
    let controls = include_str!("tests.rs");
    drop(three_fixture_test_source_before(controls));
    assert!(
        std::panic::catch_unwind(|| three_fixture_test_source_before(&format!("{controls} ")))
            .is_err()
    );
}

// Each fixed complete compiled source is restored before its unchanged transport inverse.
fn complete_projection_entry_followup_glue(source: &str, current: &str) -> String {
    let restored = embedded_acquisition_source_before(source, current)
        .unwrap_or_else(|| current.to_owned());
    let current = restored.as_str();
    let (length, sha, old_length, old_sha, edits): (usize, [u8; 32], usize, [u8; 32], &[(usize, usize, &str, &str)]) = match source {
        "src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs" => (30430, [6, 118, 157, 69, 85, 225, 141, 91, 170, 251, 173, 54, 35, 215, 41, 0, 76, 10, 246, 17, 113, 52, 40, 221, 41, 47, 112, 23, 206, 255, 53, 244], 22278, [97, 105, 89, 24, 61, 78, 70, 62, 242, 87, 227, 223, 90, 253, 33, 23, 14, 43, 174, 114, 178, 200, 144, 81, 63, 167, 81, 105, 255, 173, 118, 45], &[
            (21158, 22438, "expected_entry = match source {\n        \"src/cleanup_plan/replay.rs\" => {\n            Some(\"cce98dd37682ccb93081bae2eeb53ea103562370461844c12d16a59cb732d31b\")\n        }\n        \"src/cleanup_plan/replay/lexical_replay.rs\" => {\n            Some(\"d0f2b7fcfc3815f0c1207c064d8bcb14d4323dff614d953988b962138ff975d0\")\n        }\n        \"src/cleanup_plan/replay/expression_facts/receiving.rs\" => {\n            Some(\"db822f51812769eadf1570bfee4b315ceadd31b689fc6efa643e5da03d7a9bb7\")\n        }\n        \"src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs\" => {\n            Some(\"112b1a3f5e47c5d0c99c476c2374e7eb9dbfdc04d9220a94f6bf73b613ee294e\")\n        }\n        _ => None,\n    };\n    let entry = match expected_entry {\n        Some(expected) => {\n            let entry = crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n                source, expected, current,\n            );\n            assert_eq!(\n                format!(\n                    \"{:x}\",\n                    crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))\n                ),\n                expected,\n                \"complete region-entry historical input: {source}\"\n            );\n            entry\n        }\n        None => current.to_owned(),\n    };\n    let ", ""),
            (22569, 22570, "&", "curr"),
            (22573, 22575, "ry", ""),
            (22596, 22598, "di", "cur"),
            (22600, 22601, "c", "n"),
            (22602, 22619, "ory_before_lookup", ""),
            (22622, 22668, "region_entry_directory_before_lookup(source, &", ""),
            (22678, 22722, ");\n    let current = directory_before_lookup", ""),
            (23663, 30430, "\n// The existing OPT885 inverse runs first. This exact later lookup-stage inverse\n// then supplies its returned bytes to every unchanged original d4 row/hook.\n#[rustfmt::skip]\nfn region_entry_directory_before_lookup(source: &str, current: &str) -> String {\n    use sha2::{Digest, Sha256};\n    if source != \"src/cleanup_plan/replay/expression_facts/receiving/directory.rs\" {\n        return current.to_owned();\n    }\n    assert_eq!(current.len(), 20833);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))),\n        \"71bddf2e91cd7b6768523035ce49c111e41686dc5104adbcdb8dd7165675e464\");\n    let mut restored = current.to_owned();\n    for added in [\n        \"    fn status_key(&self) -> Option<&StatusSourceId> {\\n        None\\n    }\\n\",\n        \"    fn status_key(&self) -> Option<&StatusSourceId> {\\n        Some(self)\\n    }\\n\",\n        \"    fn status_key(&self) -> Option<&StatusSourceId> {\\n        T::status_key(self)\\n    }\\n\",\n        \"    lookup: Option<Box<status_prefix::FrozenStatusIndex>>,\\n\",\n        \"\\nstruct LookupCursor {\\n    at: usize,\\n    byte: usize,\\n    mask: u16,\\n    symbol: u16,\\n    direction: bool,\\n    units: usize,\\n}\\n// Complete initialized cursor plus receiving state, status-key Option and\\n// borrowed receivers, final candidate/ordering and comparison temporaries.\\nconst LOOKUP_BYTES: usize = 2 * std::mem::size_of::<LookupCursor>()\\n    + 2 * std::mem::size_of::<Option<&StatusSourceId>>()\\n    + 4 * std::mem::size_of::<&StatusSourceId>()\\n    + std::mem::size_of::<&status_prefix::FrozenStatusIndex>()\\n    + 2 * std::mem::size_of::<Option<usize>>()\\n    + std::mem::size_of::<usize>()\\n    + std::mem::size_of::<Ordering>();\\n\",\n        \"    #[cfg(test)]\\n    pub(super) fn has_frozen_lookup_for_test(&self) -> bool {\\n        self.lookup.is_some()\\n    }\\n\\n\",\n        \"        // The new optional field has two initialized receiving slots. Existing\\n        // directory fields and reservation requests receive no credit.\\n        budget.reserve_bytes(\\n            function,\\n            2 * std::mem::size_of::<Option<Box<status_prefix::FrozenStatusIndex>>>(),\\n            phase,\\n        )?;\\n        budget.charge(function, 1, phase)?;\\n\",\n        \"            lookup: None,\\n\",\n        \"        if let Some(index) = &self.lookup {\\n            // Binding checks read no foreign payer or cached key. A rejected\\n            // receiver remains rejected even after its original owner returns.\\n            if index.failed.get() {\\n                return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n            }\\n            if !std::ptr::eq(index.function, function)\\n                || !std::ptr::eq(index.budget, budget)\\n                || !index.ledger.current()\\n                || index.keys != self.nodes.as_ptr() as usize\\n                || index.len != self.nodes.len()\\n            {\\n                index.failed.set(true);\\n                return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n            }\\n            self.authenticate(function, budget)?;\\n            // All traversal state is one fixed receiving frame. Loop steps\\n            // update its paid fields rather than constructing a new path.\\n            budget.reserve_bytes(function, LOOKUP_BYTES, self.phase)?;\\n            budget.charge(function, 24, self.phase)?;\\n            let Some(query) = key.status_key() else {\\n                index.failed.set(true);\\n                return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n            };\\n            let mut cursor = LookupCursor {\\n                at: index.root,\\n                byte: 0,\\n                mask: 0,\\n                symbol: 0,\\n                direction: false,\\n                units: 0,\\n            };\\n            cursor.byte = index.nodes[cursor.at].byte;\\n            while cursor.byte != NIL {\\n                budget.charge(function, VISIT_UNITS + 24, self.phase)?;\\n                cursor.mask = index.nodes[cursor.at].mask;\\n                cursor.symbol = if cursor.byte < query.expression.as_str().len() {\\n                    256 | query.expression.as_str().as_bytes()[cursor.byte] as u16\\n                } else if cursor.byte == query.expression.as_str().len() {\\n                    match query.lane {\\n                        StatusLane::OwnerAdmission => 0,\\n                        StatusLane::OperationFailure => 1,\\n                        StatusLane::AllocationFailure => 2,\\n                        StatusLane::ContractFalse => 3,\\n                    }\\n                } else {\\n                    0\\n                };\\n                cursor.direction = cursor.symbol & cursor.mask != 0;\\n                cursor.at = if cursor.direction {\\n                    index.nodes[cursor.at].one\\n                } else {\\n                    index.nodes[cursor.at].zero\\n                };\\n                cursor.byte = index.nodes[cursor.at].byte;\\n            }\\n            budget.charge(function, VISIT_UNITS + 2, self.phase)?;\\n            cursor.at = index.nodes[cursor.at].zero;\\n            let candidate = &self.nodes[cursor.at];\\n            // The index selects only a candidate. Preserve the complete old\\n            // key-comparison request and actual Ord equality, without a prefix\\n            // discount, identifier parsing, hashes or numeric-ID authority.\\n            cursor.units = match key\\n                .comparison_units(&candidate.key)\\n                .and_then(|units| units.checked_add(VISIT_UNITS))\\n            {\\n                Some(units) => units,\\n                None => {\\n                    return Err(overflow(\\n                        function,\\n                        budget,\\n                        \\\"directory key comparison work overflow\\\",\\n                    ))\\n                }\\n            };\\n            budget.charge(function, cursor.units, self.phase)?;\\n            return Ok(Ord::cmp(key, &candidate.key) == Ordering::Equal);\\n        }\\n\",\n        \"        if let Some(index) = &self.lookup {\\n            // Publication seals this privately owned directory. No mutation or\\n            // fallback can revive a rejected read-only index.\\n            index.failed.set(true);\\n            return Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\\n        }\\n\",\n        \"\\n#[cfg(test)]\\nmod before_cursor;\\n\",\n\n    ] {\n        assert_eq!(restored.matches(added).count(), 1, \"complete unique directory lookup-stage inverse\");\n        restored = restored.replacen(added, \"\", 1);\n    }\n    assert_eq!(restored.len(), 15428);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))),\n        \"d161622003e500b3b9197c0f463b88eee388d9447c5a87ba75f464d3aa2a59cc\");\n    restored\n}\n", ""),
        ]),
        "src/source_reflows_merge/runtime_json.rs" => (21463, [36, 131, 234, 253, 199, 214, 78, 255, 237, 1, 163, 123, 248, 147, 178, 119, 246, 197, 124, 195, 237, 204, 53, 62, 231, 78, 238, 229, 124, 131, 42, 24], 20520, [128, 135, 79, 202, 72, 83, 188, 103, 239, 228, 64, 98, 116, 61, 101, 98, 170, 240, 248, 47, 111, 143, 17, 194, 137, 222, 39, 131, 196, 126, 8, 85], &[
            (9360, 10303, "\n        );\n        // The whole transport predecessor above is unchanged. Re-enter its\n        // exact pre-formatter consumer stage before every original projector.\n        let formatted = \"            let projected = crate::source_reflows_merge::consumer_input(\\n                \\\"signature825\\\",\\n                row[\\\"source\\\"].as_str().unwrap(),\\n                current,\\n            );\";\n        let original = \"            let projected = crate::source_reflows_merge::consumer_input(\\\"signature825\\\", row[\\\"source\\\"].as_str().unwrap(), current);\";\n        assert_eq!(before.matches(formatted).count(), 1);\n        before = before.replacen(formatted, original, 1);\n        assert_eq!(before.len(), 28182);\n        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(digest(before.as_bytes()))\n            ),\n            \"b5eb131534d7bbd10d95aa2e8eeeafd3efa7d55fde34b5ab8fa0a24fbf396062\"", ""),
        ]),
        _ => return current.to_owned(),
    };
    assert_eq!(current.len(), length);
    assert_eq!(digest(current.as_bytes()), sha);
    let mut before = current.to_owned();
    for &(start, end, after, prior) in edits.iter().rev() {
        assert!(start <= end && end <= before.len());
        assert!(before.is_char_boundary(start) && before.is_char_boundary(end));
        assert_eq!(&before[start..end], after);
        before.replace_range(start..end, prior);
    }
    assert_eq!(before.len(), old_length);
    assert_eq!(digest(before.as_bytes()), old_sha);
    before
}

// The facade itself keeps every original receiver. This full-current front
// removes only the new closed consumer front before all unchanged predecessors.
fn complete_embedded_fixture_parent_before(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 26084);
    assert_eq!(
        digest(current.as_bytes()),
        [
            86, 236, 159, 193, 237, 230, 90, 86, 16, 172, 70, 242, 216, 45, 74, 38, 172, 131, 168,
            228, 236, 226, 180, 34, 107, 188, 32, 110, 17, 27, 146, 110
        ]
    );
    let mut before = current.to_owned();
    let added = "\n// EMBEDDED848_THREE_CURRENT_BEGIN\n// Only the three evaluated acquisitions were restored. Authenticate each whole\n// compiled consumer before reconstructing its complete runtime-source stage;\n// every original transport inverse and historical assertion still follows.\nfn embedded_acquisition_source_before(source: &str, current: &str) -> Option<String> {\n    let (length, sha, old_length, old_sha, embedded, runtime): (\n        usize,\n        [u8; 32],\n        usize,\n        [u8; 32],\n        &str,\n        &str,\n    ) = match source {\n        \"src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs\" => (\n            30436,\n            [\n                237, 115, 207, 242, 72, 24, 228, 75, 198, 221, 208, 179, 60, 28, 61, 102, 143, 8,\n                11, 1, 245, 253, 222, 0, 14, 31, 63, 135, 113, 217, 167, 142,\n            ],\n            30430,\n            [\n                6, 118, 157, 69, 85, 225, 141, 91, 170, 251, 173, 54, 35, 215, 41, 0, 76, 10, 246,\n                17, 113, 52, 40, 221, 41, 47, 112, 23, 206, 255, 53, 244,\n            ],\n            \"    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\\n        env!(\\\"CARGO_MANIFEST_DIR\\\"),\\n        \\\"/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json\\\"\\n    )))\\n    .unwrap();\",\n            \"    let rows: serde_json::Value =\\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\\n            crate::source_reflows_merge::ReceiverFixture::RegionEntry,\\n        ))\\n        .unwrap();\",\n        ),\n        \"src/diagnostic/writer/tests.rs\" => (\n            12183,\n            [\n                155, 180, 211, 126, 240, 24, 153, 35, 56, 69, 157, 144, 92, 117, 88, 31, 126, 66,\n                20, 135, 80, 136, 13, 252, 139, 246, 134, 243, 251, 103, 100, 240,\n            ],\n            12196,\n            [\n                190, 196, 111, 129, 149, 10, 242, 87, 98, 82, 198, 98, 135, 70, 2, 108, 198, 53,\n                196, 200, 24, 87, 21, 100, 62, 176, 202, 77, 21, 123, 146, 235,\n            ],\n            \"    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\\n        env!(\\\"CARGO_MANIFEST_DIR\\\"),\\n        \\\"/docs/evidence/diagnostic-writer-846/source-inverses.json\\\"\\n    )))\\n    .unwrap();\",\n            \"    let rows: serde_json::Value =\\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\\n            crate::source_reflows_merge::ReceiverFixture::DiagnosticWriter,\\n        ))\\n        .unwrap();\",\n        ),\n        \"src/workspace_graph/expected_projection/type_rewrite/receiving.rs\" => (\n            28276,\n            [\n                22, 221, 107, 83, 203, 128, 52, 44, 220, 161, 220, 11, 112, 61, 14, 175, 0, 10,\n                191, 237, 97, 47, 8, 152, 187, 200, 254, 55, 207, 140, 29, 71,\n            ],\n            28277,\n            [\n                198, 17, 126, 152, 25, 183, 254, 171, 26, 83, 8, 219, 9, 71, 3, 107, 176, 194, 125,\n                45, 139, 98, 251, 184, 50, 47, 197, 221, 238, 199, 127, 102,\n            ],\n            \"        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\\n            env!(\\\"CARGO_MANIFEST_DIR\\\"),\\n            \\\"/docs/evidence/constructor-signature-rewrite-825/source-inverses.json\\\"\\n        )))\\n        .unwrap();\",\n            \"        let rows: serde_json::Value =\\n            serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\\n                crate::source_reflows_merge::ReceiverFixture::SignatureRewrite,\\n            ))\\n            .unwrap();\",\n        ),\n        _ => return None,\n    };\n    assert_eq!(\n        current.len(),\n        length,\n        \"complete embedded consumer extent: {source}\"\n    );\n    assert_eq!(\n        digest(current.as_bytes()),\n        sha,\n        \"complete embedded consumer body: {source}\"\n    );\n    assert_eq!(\n        current.matches(embedded).count(),\n        1,\n        \"unique acquisition restoration: {source}\"\n    );\n    let before = current.replacen(embedded, runtime, 1);\n    assert_eq!(\n        before.len(),\n        old_length,\n        \"complete runtime consumer extent: {source}\"\n    );\n    assert_eq!(\n        digest(before.as_bytes()),\n        old_sha,\n        \"complete runtime consumer body: {source}\"\n    );\n    Some(before)\n}\n// EMBEDDED848_THREE_CURRENT_END\n";
    assert_eq!(before.matches(added).count(), 1);
    before = before.replacen(added, "", 1);
    let added = "        let prior = embedded_acquisition_source_before(\n            \"src/workspace_graph/expected_projection/type_rewrite/receiving.rs\",\n            current,\n        )\n        .expect(\"the signature self-source is a closed embedded consumer\");\n        let current = prior.as_str();\n";
    assert_eq!(before.matches(added).count(), 1);
    before = before.replacen(added, "", 1);
    assert_eq!(before.len(), 21463);
    assert_eq!(
        digest(before.as_bytes()),
        [
            36, 131, 234, 253, 199, 214, 78, 255, 237, 1, 163, 123, 248, 147, 178, 119, 246, 197,
            124, 195, 237, 204, 53, 62, 231, 78, 238, 229, 124, 131, 42, 24
        ]
    );
    before
}
// RUNTIME848_THREE_CONTROLS_END
