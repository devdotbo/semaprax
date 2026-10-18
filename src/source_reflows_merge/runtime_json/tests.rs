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
    assert!(
        std::panic::catch_unwind(|| {
            pair_custody::normalize(
                "src/source_reflows_merge/runtime_json.rs",
                &(common.clone() + " "),
            )
        })
        .is_err()
    );
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
