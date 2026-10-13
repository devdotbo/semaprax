//! Funded declaration-only collection scan controls against its complete owned receiver.
use super::*;
use crate::test_allocation_observer::{measure, Metrics};
const LIMIT: usize = 64 * 1024 * 1024;
fn program() -> ResolvedProgram {
    let source = crate::hir::iterative_validator_tests::SOURCE;
    crate::hir::resolve(&crate::check(source, "collection-scope.spx").unwrap()).unwrap()
}
// The original hostile fixture has no Vec reference and therefore no
// authenticated compiler-owned Vec declaration. Preserve it unchanged; use
// a checked, additive fixture for positive collection/profile boundaries.
fn collection_program() -> ResolvedProgram {
    let source = format!(
        "{}\n{}",
        crate::hir::iterative_validator_tests::SOURCE,
        r#"@id("collection.seed") fn collection_seed(value: borrow Vec<i64>) -> i64 { 0 }
@id("collection.text") record CollectionText { @id("collection.text.value") value: string, }
@id("collection.carrier") record CollectionCarrier { @id("collection.carrier.rows") rows: Vec<CollectionText>, }
"#,
    );
    crate::hir::resolve(&crate::check(&source, "collection-positive.spx").unwrap()).unwrap()
}
fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap()
}
fn vector_of(element: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(crate::prelude::VEC_ID),
        arguments: vec![element],
    }
}
fn callable(parameters: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Function {
        parameters,
        result: Box::new(ResolvedType::Bool),
    }
}
fn types(program: &ResolvedProgram) -> Vec<ResolvedType> {
    let mut types = vec![
        vector_of(ResolvedType::String),
        vector_of(ResolvedType::I64),
    ];
    for function in &program.functions {
        types.push(function.return_type.clone());
        types.extend(function.params.iter().map(|p| p.ty.clone()));
    }
    let wrapped = types
        .iter()
        .map(|ty| callable(vec![ty.clone(), ty.clone()]))
        .collect::<Vec<_>>();
    types.extend(wrapped);
    types
}
type Observed = (Option<bool>, Metrics, bool, usize, bool);
fn lifetime(program: &ResolvedProgram, ty: &ResolvedType, limit: usize) -> Observed {
    let ((verdict, metrics), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result =
                    crate::hir::declared_collection::contains_funded(&program.declarations, ty);
                let verdict = result.as_ref().ok().copied();
                if verdict.is_none() {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                drop(result);
                verdict
            })
        });
    (verdict, metrics, refused, paid, receipt.is_some())
}
fn chain_lifetime(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    limit: usize,
) -> Observed {
    let ((verdict, metrics), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = crate::hir::declared_collection::validate_function_funded(
                    &program.declarations,
                    function,
                );
                let failed = result.is_err();
                if failed {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                drop(result);
                Some(!failed)
            })
        });
    (verdict, metrics, refused, paid, receipt.is_some())
}
#[test]
fn funded_scan_matches_owned_contains_with_exact_short_sticky_and_complete_drop() {
    let program = program();
    for ty in types(&program) {
        let expected = crate::hir::declared_collection::contains(&program.declarations, &ty);
        let (verdict, actual, refused, paid, receipt) = lifetime(&program, &ty, LIMIT);
        assert_eq!(verdict, Some(expected));
        assert!(!refused && !receipt && !actual.invalid);
        assert_eq!(actual.live, 0);
        assert!(
            actual.allocated <= paid && actual.highwater <= paid,
            "type {}: actual {actual:?}, paid {paid}",
            ty.identity_key()
        );
        let (exact_verdict, exact, refused, exact_paid, receipt) = lifetime(&program, &ty, paid);
        assert_eq!(exact_verdict, Some(expected));
        assert!(!refused && !receipt && !exact.invalid);
        assert_eq!(
            (exact.live, exact_paid, exact.allocated),
            (0, paid, actual.allocated)
        );
        if paid > 0 {
            let (short_verdict, short, refused, short_paid, receipt) =
                lifetime(&program, &ty, paid - 1);
            assert!(short_verdict.is_none() && refused && receipt && !short.invalid);
            assert_eq!(short.live, 0);
            assert!(short_paid < paid);
        }
    }
}
#[test]
fn trivial_resource_signature_constructs_no_scan_backing() {
    let program = program();
    let token = function(&program, "token.consume").params[0].ty.clone();
    let (verdict, actual, refused, paid, receipt) = lifetime(&program, &token, LIMIT);
    assert_eq!(verdict, Some(false));
    assert!(!refused && !receipt && !actual.invalid);
    assert_eq!(
        (actual.allocations, actual.allocated, actual.live, paid),
        (0, 0, 0, 0)
    );
}
#[test]
fn funded_chain_scan_is_exact_short_sticky_and_drop_complete_for_every_function() {
    let program = program();
    for function in &program.functions {
        let (verdict, actual, refused, paid, receipt) = chain_lifetime(&program, function, LIMIT);
        assert_eq!(verdict, Some(true));
        assert!(!refused && !receipt && !actual.invalid);
        assert_eq!(actual.live, 0);
        assert!(
            actual.allocated <= paid && actual.highwater <= paid,
            "function {}: actual {actual:?}, paid {paid}",
            function.id
        );
        let (verdict, exact, refused, exact_paid, receipt) =
            chain_lifetime(&program, function, paid);
        assert_eq!(verdict, Some(true));
        assert!(!refused && !receipt && !exact.invalid);
        assert_eq!((exact.live, exact_paid), (0, paid));
        if paid > 0 {
            let (verdict, short, refused, short_paid, receipt) =
                chain_lifetime(&program, function, paid - 1);
            assert!(verdict == Some(false) && refused && receipt && !short.invalid);
            assert_eq!(short.live, 0);
            assert!(short_paid < paid);
        }
    }
}
#[test]
fn funded_function_scan_preserves_owned_reference_diagnostics_and_refusal_order() {
    let program = program();
    let index = &program.declarations;
    let hostile = vector_of(ResolvedType::String);
    for id in ["token.consume", "hostile.construct", "hostile.if"] {
        let original = function(&program, id);
        for mask in 0..8u8 {
            let mut f = original.clone();
            if mask & 1 != 0 {
                f.return_type = hostile.clone();
            }
            if mask & 2 != 0 {
                f.params[0].ty = hostile.clone();
            }
            if mask & 4 != 0 {
                f.body.ty = hostile.clone();
            }
            let expected = crate::hir::declared_collection::validate_function(index, &f);
            let actual = crate::hir::declared_collection::validate_function_funded(index, &f);
            assert_eq!(
                actual.as_ref().err().map(Diagnostic::json),
                expected.as_ref().err().map(Diagnostic::json),
                "{id} mask {mask}"
            );
            assert_eq!(actual.is_ok(), expected.is_ok());
        }
    }
}

mod funded_early;

#[test]
fn original_resource_with_forged_collection_fields_has_no_record_authority() {
    let mut program = program();
    let token = function(&program, "token.consume").params[0].ty.clone();
    let root_id = token.nominal_id().unwrap().clone();
    let mut field = program.declarations.record_fields[&DeclarationId::new("owned.box")][0].clone();
    field.ty = vector_of(ResolvedType::String);
    program
        .declarations
        .record_fields
        .insert(root_id, vec![field]);
    assert!(!crate::hir::declared_collection::contains(
        &program.declarations,
        &token
    ));
    assert!(
        !crate::hir::declared_collection::contains_funded(&program.declarations, &token).unwrap()
    );
}

#[test]
fn funded_scan_keeps_positive_transitive_refusal_and_full_diagnostic_bytes() {
    let program = collection_program();
    let token = ResolvedType::Nominal {
        declaration: DeclarationId::new("collection.carrier"),
        arguments: Vec::new(),
    };
    assert!(crate::hir::declared_collection::contains(
        &program.declarations,
        &token
    ));
    assert!(
        crate::hir::declared_collection::contains_funded(&program.declarations, &token).unwrap()
    );
    let mut function = function(&program, "token.consume").clone();
    function.return_type = token;
    let expected =
        crate::hir::declared_collection::validate_function(&program.declarations, &function)
            .unwrap_err();
    let actual =
        crate::hir::declared_collection::validate_function_funded(&program.declarations, &function)
            .unwrap_err();
    assert_eq!(actual.json(), expected.json());
    assert_eq!(actual.code, "SPX-H006");
    assert_eq!(
        actual.message,
        "declaration-only text collection cannot occur in an executable signature or body"
    );
}

#[test]
fn complete_original_function_and_collection_receivers_keep_source_provenance() {
    use sha2::{Digest, Sha256};
    let old_scope = include_str!("old.rs");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old_scope.as_bytes()))
        ),
        "b712e23fef515940e7202d3321d98a9bbb1ff16a3b52f6935c01cdad8f4b1799"
    );
    let collection = include_str!("../../declared_collection.rs");
    // Only the two saved receiver entries gained test-only ownership; their
    // complete original bodies and every other byte still bind the frozen SHA.
    let mut collection = collection.to_owned();
    for entry in ["contains", "validate_function"] {
        let guarded = format!("#[cfg(test)]\npub(crate) fn {entry}(");
        assert_eq!(collection.matches(&guarded).count(), 1);
        collection = collection.replacen(&guarded, &format!("pub(crate) fn {entry}("), 1);
    }
    let original = collection
        .split_once("\nmod paid;")
        .unwrap()
        .0
        .trim_end_matches('\n')
        .to_owned()
        + "\n";
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "a0577d08a3ab8b3b1b7f289720ae0faf9d5ba0457c0181f2663dc5b38cf801fc"
    );
}

#[test]
fn allocation_free_retries_preserve_same_owner_first_refusal() {
    let program = program();
    let token = &function(&program, "token.consume").params[0].ty;
    let (((), actual), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(!crate::bounded_output::reserve_active_required(1));
                let result =
                    crate::hir::declared_collection::contains_funded(&program.declarations, token);
                assert!(result.is_err());
                drop(result);
                let result = lifecycle::declared_effects(&[]);
                assert!(result.is_err());
                drop(result);
                assert!(!crate::bounded_output::reserve_active_required(0));
            })
        });
    assert!(refused && !actual.invalid);
    assert_eq!(paid, 0);
    assert_eq!((actual.allocated, actual.freed, actual.live), (0, 0, 0));
    let first = receipt.unwrap();
    assert_eq!((first.requested, first.remaining), (1, 0));
}

#[test]
fn complete_seven_a_function_receiver_matches_current_and_drops_actual_backing() {
    let program = program();
    for function in &program.functions {
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        let mut current = HirValidator::new(&program).unwrap();
        let mut saved = current.clone();
        let expected = saved.validate_function_backing_saved(function, &execution);
        let actual = current.validate_function(function, &execution);
        HirValidator::assert_validation_oracle(
            &actual,
            &expected,
            &current,
            &saved,
            &BTreeMap::new(),
            &BTreeMap::new(),
            function.id.as_str(),
        );
        HirValidator::assert_scope_auxiliary_oracle(&current, &saved);
        drop((actual, expected, current, saved));
        for old in [false, true] {
            let mut validator = HirValidator::new(&program).unwrap();
            let (((), observed), refused, _, receipt) =
                crate::bounded_output::with_limit_usage_refusal(LIMIT, || {
                    measure(|| {
                        let result = if old {
                            validator.validate_function_backing_saved(function, &execution)
                        } else {
                            validator.validate_function(function, &execution)
                        };
                        assert!(result.is_ok());
                        drop(result);
                        drop(validator);
                    })
                });
            assert!(!refused && receipt.is_none() && !observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}

#[test]
fn complete_seven_a_function_receiver_keeps_full_source_provenance() {
    use sha2::{Digest, Sha256};
    let original = include_str!("backing_saved.rs")
        .replace("#[rustfmt::skip]\n", "")
        .replace("validate_function_backing_saved(", "validate_function(")
        .replace(
            "validate_expr_persistent_backing_saved(",
            "validate_expr_persistent(",
        )
        + "\n";
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "833b60ff7dde03cd0578c620e053c58733bcdb8958336fc66969f9f62205a986"
    );
}
