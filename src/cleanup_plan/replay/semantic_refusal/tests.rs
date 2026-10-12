//! Legacy rendering remains explicitly outside the paid fixed-report contract.
use super::*;
use crate::test_allocation_observer::measure;
#[test]
fn sealed_atoms_keep_saved_detailed_text_and_allocate_no_identity_payload() {
    let program = crate::hir::resolve(
        &crate::check(
            "module refusal.text; @id(\"main\") fn main()->i64 {1}",
            "refusal.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = &program.functions[0];
    let id = function.body.id.clone();
    let storage = StorageId::Temporary(id.clone());
    let cases = [
        (
            Atom::RepeatedStatus(id.clone()),
            format!("status source for `{id}` is repeated"),
        ),
        (
            Atom::UnknownStatus(id.clone()),
            format!("status source references unknown expression `{id}`"),
        ),
        (
            Atom::NoncontiguousSlot(7),
            "cleanup slot at index 7 has non-contiguous identity or index".into(),
        ),
        (
            Atom::RepeatedStorage(storage.clone()),
            format!("cleanup storage `{storage:?}` is repeated"),
        ),
        (
            Atom::DeadCallFlag(id.clone(), LivenessFlagId(17)),
            format!("call `{id}` atomically consumes dead argument flag 17"),
        ),
        (
            Atom::RepeatedCallFlag(id.clone(), LivenessFlagId(17)),
            format!("call `{id}` atomically consumes argument flag 17 twice"),
        ),
        (
            Atom::RepeatedVariantEpoch(id.clone()),
            format!("call `{id}` atomically consumes a variant epoch twice"),
        ),
        (
            Atom::MismatchedFailure(id.clone()),
            format!("selected failure `{id}` does not match the pending failing edge"),
        ),
    ];
    for (atom, text) in cases {
        let (receipt, observed) = measure(|| Receipt {
            source: id.clone(),
            atom,
        });
        assert_eq!(observed.allocations, 0);
        assert_eq!(receipt.source().as_str().as_ptr(), id.as_str().as_ptr());
        let legacy = receipt.legacy(function);
        assert_eq!(legacy.code, "SPX-H006");
        assert_eq!(
            legacy.message,
            format!("cleanup plan for function `main` failed independent replay: {text}")
        );
    }
}
