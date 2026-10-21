//! Whole current Agent prefilter input before the unchanged constructor inverse.
use sha2::{Digest, Sha256};
pub(super) fn typed(source: &str, current: &str) -> Option<String> {
    if source == "src/workspace_graph/constructor_failure/receiving/typed/tests.rs" {
        assert_eq!(
            current,
            include_str!("../workspace_graph/constructor_failure/receiving/typed/tests.rs")
        );
        let expected = "ddaa2f3c3d54bab731acb976de3a9482d71fa65a4d409a83c2036ed8f9ad1251";
        let before = super::main_e177_final_inputs::value_direct(source, expected, current);
        assert_eq!(before.len(), 27662);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
            ),
            expected
        );
        return Some(before);
    }
    if source != "src/workspace_graph/constructor_failure/typed.rs" {
        return None;
    }
    assert_eq!(
        current,
        include_str!("../workspace_graph/constructor_failure/typed.rs")
    );
    assert_eq!(
        current,
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
            .unwrap()
    );
    assert_eq!(current.len(), 12474);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "371ae248a8468c37edaa42a0791fb617a791e38f5fa56c6ad0583ac821fe60b4"
    );
    let edits: Vec<(usize, String, String)> = serde_json::from_str(include_str!(
        "../../docs/evidence/runtime-history-20261021/typed-inverse.json"
    ))
    .unwrap();
    let mut before = current.to_owned();
    for (at, after, prior) in edits.into_iter().rev() {
        assert_eq!(&before[at..at + after.len()], after);
        before.replace_range(at..at + after.len(), &prior);
    }
    assert_eq!(before.len(), 11670);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        "b07ecf1da889cf25e1b9296bf320070d49c58af316efe6036dce534fccd73fb7"
    );
    Some(before)
}

pub(super) fn owner(source: &str, current: &str) -> Option<String> {
    let (compiled, expected) = match source {
        "src/bounded_output.rs" => (
            include_str!("../bounded_output.rs"),
            "fa91c8eca4825931211d066d6ebde8c0ff157de1ec78b5950abe867c295e481f",
        ),
        "src/workspace_graph/initial_report/build.rs" => (
            include_str!("../workspace_graph/initial_report/build.rs"),
            "48b56f916c25d00004bf6d8bc13e56a0972d07f531fceda2c7c94b57d2af20e1",
        ),
        "src/workspace_graph/initial_report/tests.rs" => (
            include_str!("../workspace_graph/initial_report/tests.rs"),
            "0f530933b3964478e93a13bd7065cda6a97ab121b7c77508db45e72aff401ad7",
        ),
        _ => return None,
    };
    assert_eq!(current, compiled);
    assert_eq!(
        current,
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
            .unwrap()
    );
    let before = if source == "src/workspace_graph/initial_report/tests.rs" {
        let edits: Vec<(usize, String, String)> = serde_json::from_str(include_str!(
            "../../docs/evidence/runtime-history-20261021/owner-inverse.json"
        ))
        .unwrap();
        let mut before = current.to_owned();
        for (at, after, prior) in edits.into_iter().rev() {
            assert_eq!(&before[at..at + after.len()], after);
            before.replace_range(at..at + after.len(), &prior);
        }
        before
    } else {
        super::main_e177_final_inputs::value_direct(source, expected, current)
    };
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        expected
    );
    Some(before)
}
