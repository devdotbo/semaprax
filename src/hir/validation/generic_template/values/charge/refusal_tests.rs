//! Exact arithmetic refusal, reached through the original production branch.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
#[test]
fn arithmetic_generic_refusal_preserves_first_receipt_and_full_original_error_without_allocating() {
    let expected = hir_error("generic value checkpoint layout overflowed");
    let (((), observed), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let error = reserve(None).err().unwrap();
            assert_eq!(error, expected);
            assert!(matches!(error.message, std::borrow::Cow::Borrowed(_)));
            assert_eq!(error.message_owned_capacity(), 0);
            drop(error.clone());
            // A later different condition neither changes first receipt nor pays
            // for error storage. Preserve each original site's message separately.
            let later = reserve(Some(0)).err().unwrap();
            assert_eq!(later.code, "SPX-H006");
            assert_eq!(
                later.message,
                "generic value checkpoint exceeds the active validation budget"
            );
            assert!(later.path.is_none() && later.span.is_none() && later.help.is_none());
            drop(later.clone());
            drop(later);
            drop(error);
        })
    });
    assert!(refused);
    assert_eq!(first.unwrap().requested, usize::MAX);
    assert_eq!(paid, 0);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.freed, 0);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}
