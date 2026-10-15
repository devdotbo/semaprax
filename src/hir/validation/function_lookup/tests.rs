//! Exact current function lookup and caller-owned backing, not full-validator payment.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::hir::function_directory::caller;
use crate::test_allocation_observer::{current_metrics, measure};
mod old;
const SOURCE: &str = r#"module validator.functions;
@id("lookup.z") fn zed(value:i64)->i64 {value+1}
@id("lookup.a") fn alpha()->i64 {2}
@id("lookup.main") fn main()->i64 {zed(alpha())}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "validator-functions.spx").unwrap()).unwrap()
}
#[test]
fn borrowed_lookup_matches_owned_order_duplicates_and_exact_program_members() {
    let program = program();
    let expected = old::authenticate(&program).unwrap();
    let directory = caller::prepare(&program).unwrap_or_else(|_| panic!("directory fits"));
    let lookup = authenticate(&program, Some(&directory)).unwrap();
    assert_eq!(lookup.len(), expected.len());
    assert_eq!(
        lookup.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    let ((), metrics) = measure(|| {
        for _ in 0..1000 {
            for function in &program.functions {
                let actual = lookup.get(&function.id).unwrap();
                assert!(std::ptr::eq(*actual, function));
                assert!(std::ptr::eq(*actual, *expected.get(&function.id).unwrap()));
            }
        }
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 0);
    assert_eq!(metrics.live, 0);
    let cloned = program.clone();
    let error = authenticate(&cloned, Some(&directory)).err().unwrap();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "function directory is not bound to the current resolved program"
    );
    for mutation in 0..3 {
        let mut hostile = program.clone();
        match mutation {
            0 => hostile.functions[1].id = hostile.functions[0].id.clone(),
            1 => {
                hostile.functions[0].name = "different".into();
                hostile.functions[1].id = hostile.functions[0].id.clone();
            }
            _ => hostile.functions[0].id = DeclarationId::new("unknown.current"),
        }
        let expected = old::authenticate(&hostile).err().unwrap();
        let directory = caller::prepare(&hostile).unwrap_or_else(|_| panic!("directory fits"));
        let actual = authenticate(&hostile, Some(&directory)).err().unwrap();
        assert_eq!(actual, expected);
        if mutation == 1 {
            assert!(actual
                .message
                .contains("disagrees with its declaration index"));
            assert!(!actual.message.contains("duplicate"));
        }
    }
    // Both routes retain the complete independent core checks beyond lookup.
    assert_eq!(
        crate::hir::validation::owner_renewal::validate_core(&program),
        crate::hir::validate_core(&program)
    );
    crate::hir::validate(&program).unwrap();
    let mut hostile = program.clone();
    hostile.functions[0].return_type = ResolvedType::Bool;
    assert_eq!(
        crate::hir::validation::owner_renewal::validate_core(&hostile),
        crate::hir::validate_core(&hostile)
    );
    assert!(crate::hir::validate_core(&hostile).is_err());
}
#[test]
fn caller_function_directory_has_literal_exact_short_sticky_payment_and_full_drop() {
    let program = program();
    // One usize ordinal and one current-function pointer per retained entry.
    // Program, ledger context and observer metadata are outside observation.
    let bytes = program.functions.len()
        * (std::mem::size_of::<usize>() + std::mem::size_of::<&ResolvedFunction>());
    for limit in [0, bytes - 1, bytes] {
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(limit, || {
            measure(|| match caller::prepare(&program) {
                Ok(directory) => {
                    assert_eq!(limit, bytes);
                    let lookup = authenticate(&program, Some(&directory)).unwrap();
                    for function in &program.functions {
                        assert!(std::ptr::eq(*lookup.get(&function.id).unwrap(), function));
                    }
                    drop(lookup);
                    drop(directory);
                }
                Err(reason) => {
                    assert_eq!(reason, caller::Refused::Budget);
                    let before = current_metrics();
                    assert_eq!(
                        caller::prepare(&program).err(),
                        Some(caller::Refused::Budget)
                    );
                    assert_eq!(
                        caller::exercise_capacity_refusal(bytes * 2, 0),
                        caller::Refused::Capacity
                    );
                    assert_eq!(current_metrics(), before);
                }
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(refused, limit != bytes);
        if limit == bytes {
            assert_eq!(metrics.allocations, 1);
            assert_eq!(metrics.allocated, bytes);
            assert_eq!(used, bytes);
            assert!(receipt.is_none());
        } else {
            assert_eq!(metrics.allocations, 0);
            assert_eq!(used, 0);
            let receipt = receipt.unwrap();
            assert_eq!(receipt.requested, bytes);
            assert_eq!(receipt.remaining, limit);
        }
    }
    // Exercise both allocator-refusal continuations after the same actual
    // directory frontier has reserved and allocated its backing. No fake
    // oversized request substitutes for the original capacity receipt.
    for excess in [
        0,
        std::mem::size_of::<usize>() + std::mem::size_of::<&ResolvedFunction>(),
    ] {
        let (((), metrics), refused, used, receipt) =
            with_limit_usage_refusal(bytes + excess, || {
                measure(|| {
                    let directory = caller::prepare(&program).unwrap();
                    assert_eq!(
                        caller::exercise_capacity_refusal(bytes, excess),
                        caller::Refused::Capacity
                    );
                    let before = current_metrics();
                    assert_eq!(
                        caller::prepare(&program).err(),
                        Some(caller::Refused::Budget)
                    );
                    assert!(!crate::bounded_output::reserve_active_required(0));
                    assert_eq!(current_metrics(), before);
                    drop(directory);
                })
            });
        assert!(refused);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.allocated, bytes);
        assert_eq!(used, bytes + excess);
        let receipt = receipt.unwrap();
        assert_eq!(receipt.requested, bytes);
        assert_eq!(receipt.remaining, 0);
    }
    let ((), old_metrics) = measure(|| {
        drop(old::authenticate(&program).unwrap());
    });
    assert!(!old_metrics.invalid);
    assert_eq!(old_metrics.live, 0);
    assert_eq!(old_metrics.allocated, old_metrics.freed);
    assert!(old_metrics.allocated > bytes);
    // The caller frontier is also used by the actual independent core entry.
    let (result, refused, used, receipt) =
        with_limit_usage_refusal(bytes - 1, || crate::hir::validate_core(&program));
    assert_eq!(result.unwrap_err().code, "SPX-H006");
    assert!(refused && receipt.is_some());
    assert_eq!(used, 0);
}
