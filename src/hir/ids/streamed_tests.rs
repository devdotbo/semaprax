//! The formatting encoder `scoped_identity` is the independent oracle here.
use super::super::{
    scoped_identity, DeclarationId, FunctionExecutionId, FunctionInstanceId, ResolvedType,
};
use super::*;
use crate::bounded_output::{active_refused, with_limit_usage, with_limit_usage_refusal};

fn owners() -> Vec<FunctionExecutionId> {
    let mut owners = vec![
        FunctionExecutionId::Monomorphic(DeclarationId::new("app.main")),
        FunctionExecutionId::Monomorphic(DeclarationId::new("λ.函数.main")),
        FunctionExecutionId::Monomorphic(DeclarationId::new("")),
        FunctionExecutionId::Generic(FunctionInstanceId::derive(
            &DeclarationId::new("identity"),
            &[ResolvedType::I64],
        )),
        FunctionExecutionId::Generic(FunctionInstanceId("λ-instance".to_owned())),
    ];
    // Owner and generic-key decimal widths cross 9/10, 99/100 and 999/1000.
    for length in [9, 10, 99, 100, 999, 1000] {
        owners.push(FunctionExecutionId::Monomorphic(DeclarationId::new(
            "d".repeat(length),
        )));
        owners.push(FunctionExecutionId::Generic(FunctionInstanceId(
            "g".repeat(length),
        )));
    }
    owners
}

fn paths() -> Vec<String> {
    let mut paths = vec![
        String::new(),
        "body".to_owned(),
        "body.λ.arg.0".to_owned(),
        "contract.requires.0".to_owned(),
    ];
    for length in [9, 10, 99, 100, 999, 1000, 9999, 10_000] {
        paths.push("p".repeat(length));
    }
    paths.push("λ-prefix-".repeat(64));
    paths
}

#[test]
fn streamed_scoped_identity_matches_formatting_oracle_for_every_owner_kind_and_width() {
    for owner in owners() {
        for kind in ["expression", "value:local", "value:param", "value:result"] {
            for path in paths() {
                let oracle = scoped_identity(&owner, kind, &path);
                assert_eq!(scoped_len(&owner, kind, path.len()), Some(oracle.len()));
                let streamed = emit(&owner, kind, path.len(), |bytes| {
                    bytes.copy_from_slice(path.as_bytes())
                });
                assert_eq!(streamed, oracle, "{owner:?} {kind} {}", path.len());
                assert_eq!(streamed.capacity(), streamed.len(), "exact final buffer");
            }
        }
    }
}

#[test]
fn streamed_identity_charges_exact_bytes_before_one_allocation() {
    let owner = FunctionExecutionId::Generic(FunctionInstanceId("λ-instance".to_owned()));
    let path = "body.s1000.value";
    let total = scoped_len(&owner, "expression", path.len()).unwrap();
    let fill = |bytes: &mut [u8]| bytes.copy_from_slice(path.as_bytes());

    let ((text, metrics), overflowed, used) = with_limit_usage(total, || {
        crate::test_allocation_observer::measure(|| emit(&owner, "expression", path.len(), fill))
    });
    assert!(!overflowed);
    assert_eq!(used, total);
    assert_eq!(text, scoped_identity(&owner, "expression", path));
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 1);
    assert_eq!(metrics.reallocations, 0);
    assert_eq!(metrics.allocated, total);

    // One short: nothing is written or allocated; the refusal is sticky and
    // requests the whole exact identity.
    let ((text, metrics), overflowed, used, refusal) = with_limit_usage_refusal(total - 1, || {
        let refused = crate::test_allocation_observer::measure(|| {
            emit(&owner, "expression", path.len(), fill)
        });
        // A zero-unit retry neither clears the refusal nor charges anything.
        assert!(crate::bounded_output::reserve_active(0));
        assert!(active_refused());
        refused
    });
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(text, "");
    assert_eq!(metrics.allocations, 0);
    let refusal = refusal.unwrap();
    assert_eq!((refusal.requested, refusal.remaining), (total, total - 1));
}

#[test]
fn unaddressable_lengths_refuse_without_allocating() {
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("app.main"));
    assert_eq!(scoped_len(&owner, "expression", usize::MAX), None);
    let (text, overflowed, used) = with_limit_usage(1024, || {
        emit(&owner, "expression", usize::MAX, |_| unreachable!())
    });
    assert_eq!(text, "");
    assert!(overflowed);
    assert_eq!(used, 0);
    let refused = std::panic::catch_unwind(refuse_unaddressable);
    assert!(
        refused.is_err(),
        "outside a budget the encoder aborts like an allocation"
    );
}

#[test]
fn decimal_helpers_cover_digit_boundaries() {
    for value in [0, 9, 10, 99, 100, 999, 1000, usize::MAX] {
        let text = value.to_string();
        assert_eq!(decimal_len(value), text.len());
        let mut bytes = vec![b'#'; text.len() + 2];
        let start = put_decimal_back(&mut bytes, text.len() + 1, value);
        assert_eq!(start, 1);
        assert_eq!(&bytes[1..=text.len()], text.as_bytes());
        assert_eq!((bytes[0], bytes[text.len() + 1]), (b'#', b'#'));
    }
}
