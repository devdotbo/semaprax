use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn rebuilding_clone_omits_only_obsolete_attachments_and_settles_removed_payload() {
    let ast = crate::check(
        r#"module reuse;
@id("read") fn read(value:own Bytes)->i64 {byte_len(value)}
@id("main") fn main()->i64 {0}
"#,
        "reuse.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "read")
        .unwrap();
    assert!(!function.cleanup.slots.is_empty());
    assert!(!function.cleanup_plan.blocks.is_empty());
    // The saved full Clone is the old receiver, not an estimated copy budget.
    let mut expected = function.clone();
    expected.cleanup = CleanupInventory::unresolved();
    expected.cleanup_plan = CleanupPlan::unresolved();
    expected.loan_plan = LoanPlan::unresolved();
    assert_eq!(clone_for_rebuild(function), expected);
    let ((), old) = measure(|| {
        let copy = function.clone();
        drop(copy);
    });
    let ((), new) = measure(|| {
        let copy = clone_for_rebuild(function);
        drop(copy);
    });
    // Vec::clone allocates a real directory for every copied canonical block.
    // This literal lower bound does not use either observer's requested total.
    let removed_blocks = function.cleanup_plan.blocks.len()
        * std::mem::size_of::<crate::cleanup_plan::CleanupBlock>();
    assert!(old.allocated >= new.allocated + removed_blocks);
    assert!(old.allocations > new.allocations);
    for observed in [old, new] {
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    }
}
