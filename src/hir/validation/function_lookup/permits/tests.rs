//! Whole current directory and all-function effect verdicts, not full validator funding.
use super::*;
use crate::hir::function_directory::caller;
use crate::test_allocation_observer::{current_metrics, measure};

fn program(functions: usize, permits: usize) -> ResolvedProgram {
    let mut source = "module validator.permits;\n".to_owned();
    for index in 0..functions {
        source.push_str(&std::format!(
            "@id(\"permit.function.{index}\") fn function_{index}()->i64 {{ {index} }}\n"
        ));
    }
    source.push_str("@id(\"permit.main\") fn main()->i64 { 0 }\n");
    let mut program =
        crate::hir::resolve(&crate::check(&source, "validator-permits.spx").unwrap()).unwrap();
    // Current HIR is independently revalidated below; strings are not authority.
    program.permits = (0..permits)
        .map(|index| std::format!("effect.{index}"))
        .collect();
    program
}

#[test]
fn shared_permits_match_all_function_old_effect_diagnostics_duplicates_and_current_program() {
    let mut program = program(64, 64);
    // Unsorted and repeated authored values retain exact old membership.
    program.permits.reverse();
    program.permits.push(program.permits[0].clone());
    let directory = caller::prepare(&program).unwrap();
    let lookup = authenticate(&program, Some(&directory)).unwrap();
    let actual = lookup.permits(&program);
    let expected = Permits::Legacy(program.permits.iter().map(String::as_str).collect());
    for effect in ["effect.0", "effect.63", "unsafe", "effect", "effect.64", ""] {
        assert_eq!(actual.contains(effect), expected.contains(effect));
    }
    let foreign = program.clone();
    assert!(authenticate(&foreign, Some(&directory)).is_err());
    for mutation in 0..4 {
        let mut current = program.clone();
        match mutation {
            0 => {}
            1 => current.functions[0].effects = vec!["effect.63".into(), "effect.0".into()],
            2 => current.functions[0].effects = vec!["missing.z".into(), "missing.a".into()],
            _ => {
                // Existing earlier signature refusal still precedes later effect refusal.
                current.functions[0].return_type = ResolvedType::Unit;
                current.functions[1].effects = vec!["missing.z".into()];
            }
        }
        let expected = crate::hir::validation::owner_renewal::validate_core(&current);
        let actual = crate::hir::validate_core(&current);
        assert_eq!(actual, expected);
        if mutation < 2 {
            actual.unwrap();
        } else if mutation == 2 {
            let error = actual.unwrap_err();
            assert!(error.message.contains("missing.z"));
            assert!(!error.message.contains("missing.a"));
        } else if mutation == 3 {
            assert!(actual.unwrap_err().message.contains("unit result"));
        }
    }
}

#[test]
fn caller_paid_permit_directory_replaces_per_function_nodes_with_exact_short_sticky_and_drop() {
    let program = program(64, 64);
    let function_bytes = program.functions.len()
        * (std::mem::size_of::<usize>() + std::mem::size_of::<&ResolvedFunction>());
    let permit_bytes = program.permits.len() * std::mem::size_of::<&str>();
    let bytes = function_bytes + permit_bytes;
    for limit in [0, bytes - 1, bytes] {
        let (((), metrics), refused, used, receipt) =
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                // Fixed ledger, immutable Program and observer metadata are excluded.
                // Actual directory construction, all function queries and Drop are one scope.
                measure(|| match caller::prepare(&program) {
                    Ok(directory) => {
                        let lookup = authenticate(&program, Some(&directory)).unwrap();
                        let before = current_metrics();
                        for _function in &program.functions {
                            let permits = lookup.permits(&program);
                            assert!(permits.contains("effect.0"));
                            assert!(permits.contains("effect.63"));
                            assert!(!permits.contains("effect.64"));
                        }
                        assert_eq!(current_metrics(), before);
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
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        assert_eq!(current_metrics(), before);
                    }
                })
            });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(refused, limit != bytes);
        if limit == bytes {
            assert_eq!(metrics.allocations, 2);
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
    let ((), old) = measure(|| {
        for _function in &program.functions {
            let permits = Permits::Legacy(program.permits.iter().map(String::as_str).collect());
            assert!(permits.contains("effect.63"));
            drop(permits);
        }
    });
    assert!(!old.invalid);
    assert_eq!(old.live, 0);
    assert_eq!(old.allocated, old.freed);
    // Each nonempty old set allocates at least one real node. No guessed
    // standard-library node layout is used as a payment or expected byte count.
    assert!(old.allocations >= program.functions.len());
    assert!(old.allocated > permit_bytes);
}
