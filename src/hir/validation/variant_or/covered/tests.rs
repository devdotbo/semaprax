//! Full BTreeSet and complete old dispatch controls; actual receiving Drop.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
fn keys() -> Vec<DeclarationId> {
    (0..40)
        .map(|n| DeclarationId::new(format!("case:{n:02}:完整")))
        .collect()
}
fn observe(
    keys: &[DeclarationId],
    limit: usize,
) -> (
    bool,
    crate::test_allocation_observer::Metrics,
    bool,
    usize,
    bool,
) {
    let ((ok, metrics), refused, paid, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut set = Set::new();
            let result = (|| {
                for key in keys {
                    set.insert(key)?;
                }
                let mut copy = set.checkpoint()?;
                for key in keys {
                    assert!(!copy.insert(key)?);
                }
                Ok::<(), Diagnostic>(())
            })();
            let ok = result.is_ok();
            if !ok {
                assert!(set.checkpoint().is_err());
                assert!(set.insert(&keys[0]).is_err());
            }
            drop(result);
            drop(set);
            ok
        })
    });
    (ok, metrics, refused, paid, first.is_some())
}
#[test]
fn borrowed_covered_sorted_membership_matches_full_original_equal_id_and_duplicate_set() {
    let keys = keys();
    let mut old = BTreeSet::new();
    let mut current = Set::new();
    for ordinal in 0..keys.len() * 3 {
        let key = &keys[(ordinal * 7) % keys.len()];
        assert_eq!(current.insert(key).unwrap(), old.insert(key.clone()));
    }
    let equal = DeclarationId::new(keys[0].as_str().to_owned());
    assert!(current.contains(&equal));
    assert_eq!(
        current.iter().collect::<Vec<_>>(),
        old.iter().collect::<Vec<_>>()
    );
    let snapshot = current.checkpoint().unwrap();
    assert_eq!(
        snapshot.iter().collect::<Vec<_>>(),
        old.iter().collect::<Vec<_>>()
    );
    assert_eq!(current.len(), old.len());
}
#[test]
fn covered_growth_checkpoint_and_receiving_drop_pay_exact_short_zero_and_sticky() {
    let keys = keys();
    let (ok, actual, refused, paid, first) = observe(&keys, LIMIT);
    assert!(
        ok && !refused
            && !first
            && !actual.invalid
            && actual.live == 0
            && actual.allocated == actual.freed
    );
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    let (ok, exact, refused, used, first) = observe(&keys, paid);
    assert!(ok && !refused && !first && used == paid && exact.live == 0 && !exact.invalid);
    for limit in [0, paid - 1] {
        let (ok, actual, refused, used, first) = observe(&keys, limit);
        assert!(
            !ok && refused
                && first
                && used <= limit
                && !actual.invalid
                && actual.live == 0
                && actual.allocated == actual.freed
        );
    }
    let mut set = Set::new();
    for key in &keys {
        set.insert(key).unwrap();
    }
    let (((), actual), refused, used) = with_limit_usage(0, || {
        measure(|| {
            for key in &keys {
                assert!(!set.insert(key).unwrap());
            }
        })
    });
    assert!(!refused && used == 0 && actual.allocated == 0);
}
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            crate::hir::iterative_validator_tests::SOURCE,
            "covered-or.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn alternatives(
    cases: &[ResolvedVariantCaseDeclaration],
    variant: &DeclarationId,
) -> Vec<ResolvedMatchPattern> {
    cases
        .iter()
        .map(|case| ResolvedMatchPattern::Variant {
            variant: variant.clone(),
            case: case.id.clone(),
            fields: Vec::new(),
        })
        .collect()
}
#[test]
fn or_checkpoint_preserves_full_original_partial_candidate_diagnostics_and_guarded_discard() {
    let program = fixture();
    let declaration = program
        .types
        .iter()
        .find(|d| d.id.as_str() == "choice.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind else {
        panic!("variant");
    };
    let plain = alternatives(cases, &declaration.id);
    let mut duplicate = plain.clone();
    duplicate.push(plain[0].clone());
    let mut foreign = plain.clone();
    let ResolvedMatchPattern::Variant { case, .. } = &mut foreign[1] else {
        unreachable!()
    };
    *case = DeclarationId::new("missing.case");
    let noncase = vec![plain[0].clone(), ResolvedMatchPattern::Wildcard];
    for values in [&plain, &duplicate, &foreign, &noncase] {
        for mode in [
            ResolvedMatchMode::Value,
            ResolvedMatchMode::Own,
            ResolvedMatchMode::Borrow,
        ] {
            for wildcard in [false, true] {
                let original = Set::new();
                let mut current = original.checkpoint().unwrap();
                let mut old = BTreeSet::new();
                let expected = super::super::cover_variant_or_pattern(
                    values,
                    &declaration.id,
                    cases,
                    mode,
                    wildcard,
                    &mut old,
                );
                let actual = super::super::cover_borrowed(
                    values,
                    &declaration.id,
                    cases,
                    mode,
                    wildcard,
                    &mut current,
                );
                assert_eq!(
                    actual.as_ref().err().map(Diagnostic::json),
                    expected.as_ref().err().map(Diagnostic::json)
                );
                assert_eq!(
                    current.iter().collect::<Vec<_>>(),
                    old.iter().collect::<Vec<_>>()
                );
                drop(current);
                assert_eq!(original.len(), 0);
            }
        }
    }
}
#[test]
fn failed_checkpoint_keeps_complete_prior_sorted_root_and_receiving_drop() {
    let keys = keys();
    let mut original = Set::new();
    for key in &keys {
        original.insert(key).unwrap();
    }
    let expected = original.iter().collect::<Vec<_>>();
    let (((), actual), refused, used, first) = with_limit_usage_refusal(
        keys.len() * std::mem::size_of::<&DeclarationId>() - 1,
        || {
            measure(|| {
                assert!(original.checkpoint().is_err());
                assert!(original.checkpoint().is_err());
            })
        },
    );
    assert!(refused && used == 0 && first.is_some() && !actual.invalid && actual.allocated == 0);
    assert_eq!(original.iter().collect::<Vec<_>>(), expected);
}
#[test]
fn actual_variant_match_receiver_matches_complete_old_dispatch_and_auxiliary_state() {
    let source = format!(
        "{}\n{}",
        crate::hir::iterative_validator_tests::SOURCE,
        r#"@id("covered.or") fn covered_or(choice:Choice,flag:bool)->i64 {match choice {Choice::A {} | Choice::B {} if flag => 1, _ => 0,}}"#
    );
    let program =
        crate::hir::resolve(&crate::check(&source, "covered-dispatch.spx").unwrap()).unwrap();
    for function in &program.functions {
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        let mut current = HirValidator::new(&program).unwrap();
        let mut old = current.clone();
        let expected = old.validate_function_owned_reference(function, &execution);
        let actual = current.validate_function(function, &execution);
        HirValidator::assert_validation_oracle(
            &actual,
            &expected,
            &current,
            &old,
            &BTreeMap::new(),
            &BTreeMap::new(),
            function.id.as_str(),
        );
        HirValidator::assert_scope_auxiliary_oracle(&current, &old);
    }
}

#[test]
fn actual_or_candidate_success_duplicate_and_late_refusal_pay_all_receiving_backing() {
    let program = fixture();
    let declaration = program
        .types
        .iter()
        .find(|d| d.id.as_str() == "choice.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind else {
        panic!("variant");
    };
    let plain = alternatives(cases, &declaration.id);
    let mut duplicate = plain.clone();
    duplicate.push(plain[0].clone());
    for values in [&plain, &duplicate] {
        let observe = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let original = Set::new();
                    let result = original.checkpoint().and_then(|mut candidate| {
                        super::super::cover_borrowed(
                            values,
                            &declaration.id,
                            cases,
                            ResolvedMatchMode::Value,
                            false,
                            &mut candidate,
                        )
                    });
                    let ok = result.is_ok();
                    drop(result);
                    drop(original);
                    ok
                })
            })
        };
        let ((ok, actual), refused, paid, first) = observe(LIMIT);
        assert_eq!(ok, std::ptr::eq(values, &plain));
        assert!(
            !refused
                && first.is_none()
                && !actual.invalid
                && actual.live == 0
                && actual.allocated == actual.freed
        );
        assert!(actual.allocated <= paid && actual.highwater <= paid);
        let ((exact_ok, actual), refused, used, first) = observe(paid);
        assert_eq!(exact_ok, ok);
        assert!(!refused && first.is_none() && used == paid && actual.live == 0);
        for limit in [0, paid - 1] {
            let ((ok, actual), refused, used, first) = observe(limit);
            assert!(
                !ok && refused
                    && first.is_some()
                    && used <= limit
                    && !actual.invalid
                    && actual.live == 0
                    && actual.allocated == actual.freed
            );
        }
    }
}

#[test]
fn complete_original_or_helper_is_preserved_with_exact_frozen_digest() {
    use sha2::{Digest, Sha256};
    let original = include_str!(
        "../../../../../evidence/variant-covered-owner/src__hir__validation__variant_or.rs.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "bdfba90863b8a1001bb919e08bd53958c6267d8f3b749311da8cd74cfd96a23d"
    );
    let current = include_str!("../../variant_or.rs");
    let prefix = current.split_once("\nmod covered;").unwrap().0;
    assert_eq!(
        prefix.replace(
            "#[cfg(test)]\npub(super) fn cover_variant_or_pattern(",
            "pub(super) fn cover_variant_or_pattern("
        ),
        original
    );
}
