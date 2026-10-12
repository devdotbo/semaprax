//! Zero-entry and exact receiver payment includes returned error Clone and Drop.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

const BUDGET: &str = "generic value checkpoint exceeds the active validation budget";
fn assert_failure(error: &Diagnostic) {
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(error.severity, crate::diagnostic::Severity::Error);
    assert_eq!(error.message, BUDGET);
    assert!(matches!(error.message, Cow::Borrowed(_)));
    assert_eq!(error.message_owned_capacity(), 0);
    assert!(error.path.is_none() && error.span.is_none() && error.help.is_none());
}
#[test]
fn zero_and_one_short_generic_entry_keep_full_original_error_without_allocating() {
    let control = std::mem::size_of::<Values<'_>>();
    for limit in [0, control - 1] {
        let (((), observed), refused, paid, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let error = Values::new().err().expect("current entry refuses");
                assert_failure(&error);
                for _ in 0..32 {
                    let copy = error.clone();
                    assert_eq!(copy, error);
                    drop(copy);
                }
                assert!(!crate::bounded_output::reserve_active_required(0));
                let repeated = Values::new().err().expect("sticky entry refuses");
                assert_eq!(repeated, error);
                drop(repeated);
                drop(error);
            })
        });
        assert!(refused);
        let first = first.expect("literal current control request");
        assert_eq!(first.requested, control);
        assert_eq!(first.remaining, limit);
        assert_eq!(paid, 0);
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.allocated, 0);
        assert_eq!(observed.freed, 0);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    }
    let (((), observed), refused, paid) = with_limit_usage(control, || {
        measure(|| {
            let values = Values::new().unwrap();
            drop(values);
        })
    });
    assert!(!refused);
    assert_eq!(paid, control);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}

#[test]
fn refused_generic_checkpoint_and_shared_edit_preserve_full_prior_map_and_settle_drop() {
    let id = ValueId::new("refusal.generic.current");
    let old_type = ResolvedType::I64;
    let new_type = ResolvedType::Bool;
    // Measure full current construction and checkpoint, not a prefilled root.
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut values = Values::new().unwrap();
                values.insert(id.clone(), &old_type).unwrap();
                let checkpoint = values.checkpoint();
                match checkpoint {
                    Ok(prior) => {
                        assert_eq!(values.get(&id), Some(&old_type));
                        assert_eq!(prior.get(&id), Some(&old_type));
                        // The shared replacement must fund a changed node. There is
                        // no remaining allowance at the exact successful setup limit.
                        assert!(values.insert(id.clone(), &new_type).is_err());
                        assert_eq!(values.get(&id), Some(&old_type));
                        assert_eq!(prior.get(&id), Some(&old_type));
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        let error = values.checkpoint().err().unwrap();
                        assert_failure(&error);
                        drop(error.clone());
                        drop(error);
                        drop(prior);
                        true
                    }
                    Err(error) => {
                        assert_failure(&error);
                        assert_eq!(values.get(&id), Some(&old_type));
                        assert!(values.insert(id.clone(), &new_type).is_err());
                        assert_eq!(values.get(&id), Some(&old_type));
                        drop(error.clone());
                        drop(error);
                        false
                    }
                }
                // `values` and every partial/error allocation dispose in this window.
            })
        })
    };
    // Obtain exact setup debit without selecting a subsequent failure.
    let (((), setup), refused, exact) = with_limit_usage(usize::MAX, || {
        measure(|| {
            let mut values = Values::new().unwrap();
            values.insert(id.clone(), &old_type).unwrap();
            let prior = values.checkpoint().unwrap();
            assert_eq!(values.get(&id), prior.get(&id));
            drop(prior);
            drop(values);
        })
    });
    assert!(!refused);
    assert!(setup.allocated <= exact);
    assert_eq!(setup.freed, setup.allocated);
    assert_eq!(setup.live, 0);
    assert!(!setup.invalid);
    let ((completed, observed), refused, paid, first) = run(exact);
    assert!(completed);
    assert!(refused);
    assert!(first.is_some());
    assert_eq!(paid, exact);
    assert_eq!(observed.allocated, setup.allocated);
    assert_eq!(observed.allocations, setup.allocations);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((completed, observed), refused, paid, first) = run(exact - 1);
    assert!(!completed);
    assert!(refused);
    assert!(first.is_some());
    assert!(observed.allocated <= paid);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}
