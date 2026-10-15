//! Temporary replay storage uses the same active construction ledger.
use crate::diagnostic::Diagnostic;
use std::alloc::Layout;

fn refused() -> Diagnostic {
    // The enclosing construction owner retains its first reservation receipt.
    // This inline error introduces no newly allocated diagnostic text.
    Diagnostic::io("SPX-H006", String::new())
}
fn reserve(bytes: Option<usize>) -> Result<(), Diagnostic> {
    let Some(bytes) = bytes else {
        let _ = crate::bounded_output::reserve_active_required(usize::MAX);
        return Err(refused());
    };
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(refused())
    }
}
pub(super) fn reserve_bytes(bytes: usize) -> Result<(), Diagnostic> {
    reserve(Some(bytes))
}
pub(super) fn new_vec<T>() -> Result<Vec<T>, Diagnostic> {
    reserve(Some(Layout::new::<Vec<T>>().size()))?;
    Ok(Vec::new())
}
pub(super) fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), Diagnostic> {
    reserve(Some(0))?;
    if values.len() == values.capacity() {
        let Some(capacity) = values.capacity().max(1).checked_mul(2) else {
            reserve(None)?;
            unreachable!()
        };
        reserve(
            Layout::array::<T>(capacity)
                .ok()
                .map(|layout| layout.size()),
        )?;
        if values.try_reserve_exact(capacity - values.len()).is_err() {
            reserve(None)?;
            unreachable!()
        }
        let actual = values.capacity();
        if actual > capacity {
            reserve(
                Layout::array::<T>(actual - capacity)
                    .ok()
                    .map(|layout| layout.size()),
            )?;
        }
        if actual != capacity {
            reserve(None)?;
            unreachable!()
        }
    }
    values.push(value);
    Ok(())
}

/// The internal first semantic report owns its exact text. Public Vec report
/// wrappers remain outside this receiver's storage contract.
pub(super) fn diagnostic(
    code: &'static str,
    span: Option<crate::ast::Span>,
    parts: &[&str],
) -> Diagnostic {
    let bytes = parts
        .iter()
        .try_fold(0usize, |sum, part| sum.checked_add(part.len()));
    let required = bytes.and_then(|bytes| bytes.checked_add(Layout::new::<Diagnostic>().size()));
    if reserve(required).is_err() {
        return refused();
    }
    let bytes = bytes.unwrap();
    let mut text = String::with_capacity(bytes);
    if text.capacity() != bytes {
        if text.capacity() > bytes {
            let _ = reserve(Some(text.capacity() - bytes));
        }
        let _ = reserve(None);
        return refused();
    }
    for part in parts {
        text.push_str(part);
    }
    match span {
        Some(span) => Diagnostic::error(code, text, span),
        None => Diagnostic::io(code, text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::{active_refused, with_limit_usage_refusal};
    use crate::test_allocation_observer::measure;

    #[test]
    fn replay_temporary_vector_growth_has_literal_exact_short_sticky_and_drop_limits() {
        // Five usize entries grow through2/4/8; retained stack header is separate.
        let header = Layout::new::<Vec<usize>>().size();
        let backing = (2 + 4 + 8) * std::mem::size_of::<usize>();
        let exact = header + backing;
        let (((), observed), overflow, charged, refusal) = with_limit_usage_refusal(exact, || {
            measure(|| {
                let mut values = new_vec().unwrap();
                for value in 0usize..5 {
                    push(&mut values, value).unwrap();
                }
                assert_eq!(values.as_slice(), &[0, 1, 2, 3, 4]);
                drop(values);
            })
        });
        assert!(!overflow);
        assert_eq!(refusal, None);
        assert_eq!(charged, exact);
        assert!(!observed.invalid);
        assert_eq!(observed.allocated, backing);
        assert_eq!(observed.live, 0);
        let ((first, retry, observed), overflow, charged, refusal) =
            with_limit_usage_refusal(exact - 1, || {
                let ((first, retry), observed) = measure(|| {
                    let mut values = new_vec().unwrap();
                    for value in 0usize..4 {
                        push(&mut values, value).unwrap();
                    }
                    let first = push(&mut values, 4).unwrap_err();
                    let retry = reserve_bytes(0).unwrap_err();
                    assert_eq!(values.as_slice(), &[0, 1, 2, 3]);
                    assert!(active_refused());
                    drop(values);
                    (first, retry)
                });
                (first, retry, observed)
            });
        assert!(overflow);
        assert_eq!((first.code, retry.code), ("SPX-H006", "SPX-H006"));
        assert!(first.message.is_empty() && retry.message.is_empty());
        assert_eq!(charged, header + (2 + 4) * std::mem::size_of::<usize>());
        let refusal = refusal.unwrap();
        assert_eq!(
            (refusal.requested, refusal.remaining),
            (
                8 * std::mem::size_of::<usize>(),
                8 * std::mem::size_of::<usize>() - 1
            )
        );
        assert!(!observed.invalid);
        assert_eq!(observed.allocated, (2 + 4) * std::mem::size_of::<usize>());
        assert_eq!(observed.live, 0);
        let (((), observed), overflow, charged, first) =
            with_limit_usage_refusal(header + 2 * std::mem::size_of::<usize>(), || {
                measure(|| {
                    let mut values = new_vec().unwrap();
                    push(&mut values, 7usize).unwrap(); // one spare slot remains
                    assert!(reserve_bytes(1).is_err());
                    assert!(push(&mut values, 8).is_err());
                    assert_eq!(values.as_slice(), &[7]);
                    drop(values);
                })
            });
        assert!(overflow);
        assert_eq!(charged, header + 2 * std::mem::size_of::<usize>());
        let first = first.unwrap();
        assert_eq!((first.requested, first.remaining), (1, 0));
        assert_eq!(observed.allocated, 2 * std::mem::size_of::<usize>());
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    #[test]
    fn first_semantic_text_is_paid_before_allocation_and_short_retry_is_inline() {
        const TEXT: &str = "row loan children are not exact places";
        let exact = Layout::new::<Diagnostic>().size() + TEXT.len();
        let (((), allocations), overflow, used, receipt) = with_limit_usage_refusal(exact, || {
            measure(|| {
                let report = diagnostic(
                    "SPX-H006",
                    None,
                    &["row loan ", "children are not exact places"],
                );
                assert_eq!(report.message, TEXT);
                drop(report);
            })
        });
        assert!(!overflow);
        assert_eq!(receipt, None);
        assert_eq!(used, exact);
        assert_eq!(allocations.allocated, TEXT.len());
        assert_eq!(allocations.live, 0);
        let (((), allocations), overflow, used, receipt) =
            with_limit_usage_refusal(exact - 1, || {
                measure(|| {
                    let report = diagnostic("SPX-H006", None, &[TEXT]);
                    assert_eq!(report.code, "SPX-H006");
                    assert!(report.message.is_empty());
                    drop(report);
                    let retry = diagnostic("SPX-T284", None, &[TEXT]);
                    assert_eq!(retry.code, "SPX-H006");
                    assert!(retry.message.is_empty());
                    drop(retry);
                })
            });
        assert!(overflow);
        assert_eq!(used, 0);
        assert_eq!(receipt.unwrap().requested, exact);
        assert_eq!(allocations.allocated, 0);
        assert_eq!(allocations.live, 0);
    }
}
