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
    let mut root_before = include_str!("../../source_reflows_merge.rs").to_owned();
    assert_eq!(&root_before[685..867], ";\n// Closed authenticated repository DATA only; no live-source or compiler authority.\npub(crate) use runtime_json::{receiver_fixture, receiver_fixture_source_before, ReceiverFixture}");
    root_before.replace_range(685..867, "");
    assert_eq!(
        digest(root_before.as_bytes()),
        [
            47, 199, 81, 107, 241, 8, 163, 83, 135, 195, 164, 227, 44, 126, 21, 210, 102, 90, 21,
            60, 154, 237, 154, 234, 103, 203, 114, 174, 218, 132, 112, 225
        ]
    );
    let mut receiver_before =
        include_str!("../../source_reflows/incoming67107_semantic_receivers.rs").to_owned();
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
    let mut original = runtime.to_owned();
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
