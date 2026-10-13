use super::*;
use crate::hir::{DeclarationId, TypeFacts};
fn index() -> DeclarationIndex {
    let source = crate::parse(
        "module borrowed.results; @id(\"app.main\") fn main() -> i64 {0}",
        std::path::Path::new("results.spx"),
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap().declarations
}
fn kinds() -> Vec<ResolvedImportResultKind> {
    use ResolvedImportResultKind::*;
    vec![
        Unit,
        I64,
        Bool,
        ResultI64I64,
        OwnedString,
        OwnedOptionString,
        OwnedResultStringI64,
        OwnedResultStringOptionI64,
        OwnedResultResourceI64 {
            resource: DeclarationId::new("hostile.λ:resource"),
        },
        BorrowedStr {
            resource: DeclarationId::new("hostile.resource"),
        },
        OwnedResource {
            resource: DeclarationId::new("hostile.λ:resource"),
        },
    ]
}
#[test]
fn borrowed_import_result_comparison_preserves_all_owned_carriers_and_forged_identities() {
    let mut index = index();
    let kinds = kinds();
    let mut types = kinds
        .iter()
        .map(|kind| kind.value_type(&index).unwrap())
        .collect::<Vec<_>>();
    types.extend([
        ResolvedType::Never,
        ResolvedType::U8,
        ResolvedType::Nominal {
            declaration: DeclarationId::new("forged.result"),
            arguments: vec![ResolvedType::String, ResolvedType::I64],
        },
    ]);
    for kind in &kinds {
        for ty in &types {
            assert_eq!(
                kind.matches_value_type(&index, ty),
                kind.value_type(&index).is_ok_and(|actual| &actual == ty)
            );
        }
    }
    for ty in &types {
        let old = [
            ResolvedImportResultKind::OwnedOptionString,
            ResolvedImportResultKind::OwnedResultStringI64,
            ResolvedImportResultKind::OwnedResultStringOptionI64,
        ]
        .iter()
        .any(|kind| kind.value_type(&index).is_ok_and(|actual| &actual == ty));
        assert_eq!(
            ResolvedImportResultKind::is_owned_container_type(ty, &index),
            old
        );
    }
    // Missing compiler identity must remain a comparison miss, even if a
    // same-shaped retained TypeFacts row is forged in the index.
    index.type_facts_by_id.insert(
        "forged.result".into(),
        TypeFacts {
            copy: false,
            contains_resource: false,
            sized: true,
            needs_drop: true,
            layout_key: "forged".into(),
        },
    );
    for name in ["Option", "Result"] {
        index.types_by_name.remove(name);
        for kind in &kinds {
            for ty in &types {
                assert_eq!(
                    kind.matches_value_type(&index, ty),
                    kind.value_type(&index).is_ok_and(|actual| &actual == ty)
                );
            }
        }
    }
}
#[test]
fn borrowed_native_container_comparisons_remove_actual_type_payload_acquisition_through_drop() {
    let index = index();
    let kinds = kinds();
    let types = kinds
        .iter()
        .map(|kind| kind.value_type(&index).unwrap())
        .collect::<Vec<_>>();
    for count in [1, 32, 256] {
        let (_, new) = crate::test_allocation_observer::measure(|| {
            for _ in 0..count {
                for kind in &kinds {
                    for ty in &types {
                        std::hint::black_box(kind.matches_value_type(&index, ty));
                    }
                }
            }
        });
        assert!(!new.invalid);
        assert_eq!((new.allocations, new.allocated, new.live), (0, 0, 0));
        let (_, old) = crate::test_allocation_observer::measure(|| {
            for _ in 0..count {
                for kind in &kinds {
                    for ty in &types {
                        std::hint::black_box(
                            kind.value_type(&index).is_ok_and(|actual| &actual == ty),
                        );
                    }
                }
            }
        });
        assert!(!old.invalid && old.allocated > 0);
        assert_eq!(old.live, 0);
        assert_eq!(old.allocated, old.freed);
    }
}
