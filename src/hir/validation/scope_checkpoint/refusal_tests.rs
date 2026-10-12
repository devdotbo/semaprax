//! Original complete rich binding fixture; physical error message storage only.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::borrow::Cow;

fn rich(count: usize) -> BTreeMap<ValueId, ValidationBinding> {
    (0..count)
        .map(|index| {
            let path = vec![PlaceProjection::Field(DeclarationId::new("scope.field")); 3];
            let mut moved_places = BTreeMap::new();
            moved_places.insert(path.clone(), Availability::MaybeMoved);
            let mut definitely_partial = BTreeSet::new();
            definitely_partial.insert(path);
            let mut active_loans = BTreeSet::new();
            active_loans.insert(LoanId(index as u16));
            (
                ValueId::new(format!("scope.binding.{index:04}")),
                ValidationBinding {
                    contract_local: index % 2 == 0,
                    mutable: index % 3 == 0,
                    ty: ResolvedType::Function {
                        parameters: vec![
                            ResolvedType::Nominal {
                                declaration: DeclarationId::new("scope.nominal"),
                                arguments: vec![ResolvedType::I64, ResolvedType::Bool],
                            };
                            2
                        ],
                        result: Box::new(ResolvedType::I64),
                    },
                    ownership: OwnershipMode::Own,
                    availability: Availability::Available,
                    moved_places,
                    definitely_partial,
                    active_loans,
                },
            )
        })
        .collect()
}
const BUDGET: &str = "ownership scope checkpoint exceeds the active validation budget";
const OVERFLOW: &str = "ownership scope checkpoint layout overflowed";
fn assert_error(error: &Diagnostic, message: &str) {
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(error.severity, crate::diagnostic::Severity::Error);
    assert_eq!(error.message, message);
    assert!(matches!(error.message, Cow::Borrowed(_)));
    assert_eq!(error.message_owned_capacity(), 0);
    assert!(error.path.is_none() && error.span.is_none() && error.help.is_none());
}
#[test]
fn zero_scope_and_arithmetic_refusal_keep_original_full_diagnostics_without_message_backing() {
    let input = rich(17);
    for overflow in [false, true] {
        let expected = Diagnostic::io("SPX-H006", if overflow { OVERFLOW } else { BUDGET });
        let (((), observed), refused, paid, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                let error = if overflow {
                    charge::reserve(None).err().unwrap()
                } else {
                    Scope::fresh_baseline(&input).err().unwrap()
                };
                assert_error(&error, if overflow { OVERFLOW } else { BUDGET });
                assert_eq!(error, expected);
                for _ in 0..32 {
                    drop(error.clone());
                }
                assert!(!crate::bounded_output::reserve_active_required(0));
                drop(error);
            })
        });
        assert!(refused);
        assert!(first.is_some());
        if overflow {
            assert_eq!(first.unwrap().requested, usize::MAX);
        }
        assert_eq!(paid, 0);
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.allocated, 0);
        assert_eq!(observed.freed, 0);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    }
}

#[test]
fn whole_rich_scope_exact_short_sticky_refusal_includes_error_clone_and_drop() {
    let input = rich(17);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| match Scope::fresh_baseline(&input) {
                Ok(scope) => {
                    assert_eq!(scope.len(), input.len());
                    assert!(scope.iter().eq(input.iter()));
                    let checkpoint = scope.clone();
                    assert_eq!(scope, checkpoint);
                    drop(checkpoint);
                    drop(scope);
                    true
                }
                Err(error) => {
                    assert_error(&error, BUDGET);
                    assert!(!crate::bounded_output::reserve_active_required(0));
                    let sticky = Scope::fresh_baseline(&input).err().unwrap();
                    assert_eq!(sticky, error);
                    drop(sticky.clone());
                    drop(sticky);
                    drop(error.clone());
                    drop(error);
                    false
                }
            })
        })
    };
    let ((success, observed), refused, exact, first) = run(usize::MAX);
    assert!(success);
    assert!(!refused);
    assert_eq!(first, None);
    assert!(observed.allocated <= exact);
    assert!(observed.highwater <= exact);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((success, repeated), refused, paid, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(paid, exact);
    assert_eq!(first, None);
    assert_eq!(repeated, observed);
    let ((success, short), refused, paid, first) = run(exact - 1);
    assert!(!success);
    assert!(refused);
    assert!(first.is_some());
    assert!(short.allocated <= paid);
    assert_eq!(short.freed, short.allocated);
    assert_eq!(short.live, 0);
    assert!(!short.invalid);
    // Caller/input authority and complete source payload remain intact.
    assert_eq!(input, rich(17));
}
