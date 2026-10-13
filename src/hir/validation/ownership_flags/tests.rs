use super::*;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let owner = include_str!("../function_scope/tests.rs");
    let source = owner
        .split_once("const CONTRACT_SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    crate::hir::resolve(&crate::check(source, "ownership-copy.spx").unwrap()).unwrap()
}
fn compare(program: &ResolvedProgram, ty: &ResolvedType) {
    let current = HirValidator::new(program).unwrap();
    let old = HirValidator::new(program).unwrap();
    for ownership in [
        OwnershipMode::Value,
        OwnershipMode::Borrow,
        OwnershipMode::Own,
        OwnershipMode::Shared,
    ] {
        assert_eq!(
            current.expected_ownership(ty, ownership),
            old.expected_ownership_before(ty, ownership)
        );
        assert_eq!(
            current.is_owned_resource(ty, ownership),
            old.is_owned_resource_before(ty, ownership)
        );
    }
}
#[test]
fn complete_saved_copy_receivers_keep_all_modes_cache_precedence_and_corpus_types() {
    let program = program();
    for ty in [
        ResolvedType::Never,
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::String,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::ArrayU8(0),
        ResolvedType::ArrayU8(257),
        ResolvedType::Function {
            parameters: vec![ResolvedType::I64],
            result: Box::new(ResolvedType::I64),
        },
        ResolvedType::Function {
            parameters: vec![ResolvedType::Bytes],
            result: Box::new(ResolvedType::Str),
        },
    ] {
        compare(&program, &ty);
    }
    for function in &program.functions {
        compare(&program, &function.return_type);
        compare(&program, &function.body.ty);
        for param in &function.params {
            compare(&program, &param.ty);
        }
    }
    let mut poisoned = program.clone();
    let mut facts = poisoned
        .declarations
        .type_facts(&ResolvedType::I64)
        .unwrap();
    facts.copy = !facts.copy;
    poisoned
        .declarations
        .type_facts_by_id
        .insert("i64".into(), facts);
    compare(&poisoned, &ResolvedType::I64);
}
#[test]
fn primitive_copy_queries_remove_actual_layout_acquisition_and_authenticate_sticky_zero() {
    let program = program();
    let current = HirValidator::new(&program).unwrap();
    let old = HirValidator::new(&program).unwrap();
    let (((), actual), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            measure(|| {
                for _ in 0..17 {
                    assert_eq!(
                        current
                            .expected_ownership(&ResolvedType::I64, OwnershipMode::Own)
                            .unwrap(),
                        OwnershipMode::Value
                    );
                    assert!(
                        !current
                            .is_owned_resource(&ResolvedType::I64, OwnershipMode::Own)
                            .unwrap()
                    );
                }
            })
        });
    assert!(!refused && paid == 0 && first.is_none());
    assert_eq!(actual.allocations, 0);
    assert_eq!(actual.allocated, 0);
    assert!(!actual.invalid && actual.live == 0);
    let ((), saved) = measure(|| {
        for _ in 0..17 {
            assert_eq!(
                old.expected_ownership_before(&ResolvedType::I64, OwnershipMode::Own)
                    .unwrap(),
                OwnershipMode::Value
            );
            assert!(
                !old.is_owned_resource_before(&ResolvedType::I64, OwnershipMode::Own)
                    .unwrap()
            );
        }
    });
    assert!(saved.allocated > actual.allocated);
    assert!(!saved.invalid && saved.live == 0 && saved.allocated == saved.freed);
    let (((), sticky), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            measure(|| {
                assert!(
                    current
                        .expected_ownership(&ResolvedType::I64, OwnershipMode::Own)
                        .is_err()
                );
                assert!(
                    current
                        .is_owned_resource(&ResolvedType::I64, OwnershipMode::Own)
                        .is_err()
                );
            })
        });
    assert!(refused && paid == 0 && first.unwrap().requested == 1);
    assert_eq!(sticky.allocated, 0);
    assert!(!sticky.invalid && sticky.live == 0);
}
#[test]
fn original_dispatch_is_nested_unwind_safe_and_complete_methods_stay_frozen() {
    assert!(!original_selected());
    let payload: Box<dyn std::any::Any + Send> = Box::new("ownership control unwind");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _one = original_guard();
        {
            let _two = original_guard();
            assert!(original_selected());
        }
        assert!(original_selected());
        std::panic::resume_unwind(payload);
    }));
    assert!(result.is_err() && !original_selected());
    drop(result);
    let before = include_str!("before.rs");
    for (name, old) in [
        (
            "expected_ownership",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/expected_ownership.before.txt"
            )),
        ),
        (
            "is_owned_resource",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/is_owned_resource.before.txt"
            )),
        ),
    ] {
        assert!(before.contains(&old.replace(
            &format!("fn {name}("),
            &format!("pub(in crate::hir::validation) fn {name}_before(")
        )));
    }
}

#[test]
fn full_nominal_fixture_keeps_missing_generic_forged_cache_and_unknown_copy_routes() {
    let owner = include_str!("../nominal_facts/tests.rs");
    let source = owner
        .split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    let mut program =
        crate::hir::resolve(&crate::parse(source, "ownership-nominal.spx").unwrap()).unwrap();
    let nominal = |id: &str, arguments| ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    };
    let missing = nominal("facts.record", Vec::new());
    program
        .declarations
        .type_facts_by_id
        .remove(&missing.identity_key());
    let empty_generic = nominal("facts.generic", Vec::new());
    let forged = program.declarations.type_facts(&ResolvedType::I64).unwrap();
    program
        .declarations
        .type_facts_by_id
        .insert(empty_generic.identity_key(), forged);
    for ty in [
        missing,
        empty_generic,
        nominal("facts.class", Vec::new()),
        nominal("facts.variant", Vec::new()),
        nominal("facts.owned", Vec::new()),
        nominal("facts.generic", vec![ResolvedType::I64]),
        nominal("facts.generic", vec![ResolvedType::U8]),
        nominal(crate::prelude::VEC_ID, vec![ResolvedType::I64]),
        nominal(crate::prelude::BOX_ID, vec![ResolvedType::U8]),
        nominal("facts.unknown", Vec::new()),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("facts.generic"),
            index: 0,
        },
    ] {
        compare(&program, &ty);
    }
}

#[test]
fn full_original_ownership_methods_and_all_hooked_sources_are_exactly_bound() {
    use sha2::{Digest, Sha256};
    let proof: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ownership-copy-flags/source-provenance.json"
    )))
    .unwrap();
    let sources = [
        (
            "src/hir/validation.rs",
            include_str!("../../validation.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/validation.rs.before.txt"
            )),
            "9e878bead4d3d748764a55e10c51edd93146f1998d5eafd896205b31f1067ad6",
        ),
        (
            "src/hir/validation/nominal_facts.rs",
            include_str!("../nominal_facts.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/nominal_facts.rs.before.txt"
            )),
            "f85363333cd6d13d361f1e41c967d7033d2934627d30758d8188b66ff6a9a873",
        ),
        (
            "src/hir/validation/identity_directory/oracle_entries.rs",
            include_str!("../identity_directory/oracle_entries.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/oracle_entries.rs.before.txt"
            )),
            "3b42c48e7dd26991eb4e773952d69c7934118f2fea666ff8b687947e4fa6ece1",
        ),
        (
            "src/hir/validation/call_parameters.rs",
            include_str!("../call_parameters.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/call_parameters.rs.before.txt"
            )),
            "3be63c3a568b23436a6e5897669266202ea71033003b963891de196094b9b4aa",
        ),
    ];
    for (path, current, original, digest) in sources {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            digest
        );
        assert_eq!(
            proof["original_sources"][path]["sha256"].as_str().unwrap(),
            digest
        );
        let mut projected = current.to_owned();
        for op in proof["full_current_projection"][path].as_array().unwrap() {
            let from = op["from"].as_str().unwrap();
            let to = op["to"].as_str().unwrap();
            assert_eq!(projected.matches(from).count(), 1, "{}", op["name"]);
            projected = projected.replace(from, to);
        }
        assert_eq!(projected, original);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/ownership-copy-flags/expected_ownership.before.txt"
                ))
                .as_bytes()
            ))
        ),
        "70d1af41ec7068192db12503d5db6ac92922ce6dcef36a635721844b30b7aac5"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/ownership-copy-flags/is_owned_resource.before.txt"
                ))
                .as_bytes()
            ))
        ),
        "bffbf162d46f6c7e15339f00332f69018c07d44b738736ebb453b215ac70c5e4"
    );
}

#[test]
fn generic_copy_fallback_pays_complete_backing_drop_exact_short_and_sticky() {
    let owner = include_str!("../nominal_facts/tests.rs");
    let source = owner
        .split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    let program =
        crate::hir::resolve(&crate::parse(source, "ownership-generic.spx").unwrap()).unwrap();
    let ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("facts.generic"),
        arguments: vec![ResolvedType::I64],
    };
    let observe = |limit| {
        let validator = HirValidator::new(&program).unwrap();
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = validator.expected_ownership(&ty, OwnershipMode::Own);
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = crate::bounded_output::active_remaining();
                    drop(validator.expected_ownership(&ty, OwnershipMode::Own));
                    drop(validator.is_owned_resource(&ty, OwnershipMode::Own));
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(
                        (before.allocations, before.allocated),
                        (after.allocations, after.allocated)
                    );
                    assert_eq!(remaining, crate::bounded_output::active_remaining());
                } else {
                    assert_eq!(result.as_ref().unwrap(), &OwnershipMode::Value);
                }
                drop(result);
            })
        })
    };
    let (((), wide), refused, required, first) = observe(usize::MAX);
    assert!(!refused && first.is_none() && required > 0);
    assert!(!wide.invalid && wide.live == 0 && wide.allocated == wide.freed);
    assert!(wide.allocated <= required && wide.highwater <= required);
    let (((), exact), refused, used, first) = observe(required);
    assert!(!refused && used == required && first.is_none());
    assert_eq!(exact.allocated, wide.allocated);
    assert!(!exact.invalid && exact.live == 0 && exact.allocated == exact.freed);
    for limit in [0, required - 1] {
        let (((), short), refused, used, first) = observe(limit);
        assert!(refused && first.is_some() && used <= limit);
        assert!(!short.invalid && short.live == 0 && short.allocated == short.freed);
        assert!(short.allocated <= used && short.highwater <= used);
    }
    let old = HirValidator::new(&program).unwrap();
    let ((), physical) = measure(|| {
        assert_eq!(
            old.expected_ownership_before(&ty, OwnershipMode::Own)
                .unwrap(),
            OwnershipMode::Value
        )
    });
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
}
