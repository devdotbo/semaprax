//! Selected fused owner resource ABI. Authored, unexecuted until the parent's
//! whole batch runs.
use super::{build_resource_abi, build_selected_resource_abi};

const ORIGINAL: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");

#[test]
fn selected_owner_resource_abi_uses_complete_selected_revalidation_and_ordinary_stays_closed() {
    let source = crate::parse(ORIGINAL, "selected-resource-abi.spx").unwrap();
    let owner = crate::hir::selected_fused_candidate::validate_selected(&source)
        .expect("original source passes full selected validation");
    // Ordinary validation of the very held program still refuses its loop.
    let ordinary = build_resource_abi(owner.program()).unwrap_err();
    assert_eq!(ordinary.code, "SPX-H006");
    assert!(ordinary.message.contains("core.bytes.zeroed"), "{ordinary:?}");
    // A clone is not an owner; only the ordinary entry can receive it.
    let cloned = owner.program().clone();
    assert_eq!(build_resource_abi(&cloned).unwrap_err().code, "SPX-H006");
    let abi = build_selected_resource_abi(&owner).expect("selected owner resource ABI");
    assert!(abi.resources.is_empty() && abi.lifecycles.is_empty());
    assert_eq!(abi, build_selected_resource_abi(&owner).unwrap());
    // The owner prelude uses the owner constructor; the raw-program prelude
    // keeps ordinary validation.
    let emission = include_str!("../native_emit/emission.rs");
    let (raw, owned) = emission
        .split_once("pub(in crate::codegen) fn emit_validated_fused_row_prelude(")
        .unwrap();
    assert!(raw.contains("hir::validate(program)?;"));
    let owned = owned.split_once("\n}\n").unwrap().0;
    assert!(owned.contains("native_resource::build_selected_resource_abi(owner)?"));
    assert!(!owned.contains("build_resource_abi(program)"));
}
