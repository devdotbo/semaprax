use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

#[test]
fn complete_primary_walk_without_row_loans_adds_no_replay_storage_or_reservation() {
    let source = crate::check(
        r#"module app.scalar;
@id("app.helper") fn helper(value:i64)->i64 requires value>=0; ensures result>=0; {value+1}
@id("app.main") fn main()->i64 {if false {helper(2)} else {helper(3)}}
"#,
        "row-loan-absence.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
    let observed = std::cell::Cell::new(false);
    // Both paths execute the same complete primary validator. The extra
    // observation is stack-only and is read only after all primary checks.
    let ((ordinary, old_allocations), old_refused, old_charge, old_receipt) =
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                HirValidator::new(&program)
                    .unwrap()
                    .with_helpers(&helpers)
                    .validate()
            })
        });
    let ((joined, new_allocations), new_refused, new_charge, new_receipt) =
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let result = HirValidator::new(&program)
                    .unwrap()
                    .with_row_loan_observer(&observed)
                    .with_helpers(&helpers)
                    .validate();
                if result.is_ok() && observed.get() {
                    crate::hir::row_loan::validate_program(&program).unwrap();
                }
                result
            })
        });
    assert_eq!(ordinary, joined);
    assert!(!observed.get());
    assert!(!old_refused && !new_refused);
    assert_eq!((old_charge, old_receipt), (new_charge, new_receipt));
    assert_eq!(old_allocations, new_allocations);
    assert!(!new_allocations.invalid);
    assert_eq!(new_allocations.live, 0);
}
