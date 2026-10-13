use super::*;
use crate::hir::DeclarationId;

fn leaves() -> Vec<(ResolvedType, bool, bool)> {
    use ResolvedType::*;
    vec![
        (Unit, true, false),
        (I64, true, false),
        (I32, true, false),
        (Char, true, false),
        (U8, true, false),
        (Usize, true, false),
        (F32, true, false),
        (F64, true, false),
        (Bool, true, false),
        (String, false, true),
        (Bytes, false, true),
        (Str, false, false),
        (SliceU8, false, false),
        (StringMap, false, true),
        (OnceFunction, false, true),
        (OnceFunctionI64, false, true),
        (OnceFunctionI64Pair, false, true),
        (MutFunctionI64, false, false),
    ]
}
fn old(index: &DeclarationIndex, ty: &ResolvedType) -> Option<TypeFlags> {
    index
        .type_facts_owned_key_oracle(ty)
        .as_ref()
        .map(TypeFlags::from)
}
#[test]
fn leaf_flags_preserve_literal_facts_and_current_cached_precedence_without_query_allocations() {
    let mut index = DeclarationIndex::default();
    let types = leaves();
    for (ty, copy, needs_drop) in &types {
        let expected = Some(TypeFlags {
            copy: *copy,
            contains_resource: false,
            sized: true,
            needs_drop: *needs_drop,
        });
        assert_eq!(old(&index, ty), expected);
        assert_eq!(index.type_flags(ty), expected);
    }
    let (_, observed) = crate::test_allocation_observer::measure(|| {
        for _ in 0..128 {
            for (ty, copy, needs_drop) in &types {
                assert_eq!(
                    index.type_flags(ty),
                    Some(TypeFlags {
                        copy: *copy,
                        contains_resource: false,
                        sized: true,
                        needs_drop: *needs_drop
                    })
                );
            }
        }
    });
    assert!(!observed.invalid);
    assert_eq!(
        (observed.allocations, observed.allocated, observed.live),
        (0, 0, 0)
    );
    let (_, owned) = crate::test_allocation_observer::measure(|| {
        for (ty, _, _) in &types {
            std::hint::black_box(index.type_facts_owned_key_oracle(ty));
        }
    });
    assert!(!owned.invalid && owned.allocations > 0 && owned.allocated > 0);
    assert_eq!(owned.live, 0);
    // A cached entry takes precedence even when independently invalid. The
    // ordinary HIR validator, not this projection, owns cache-fact validation.
    for (ty, _, _) in types
        .iter()
        .chain(std::iter::once(&(ResolvedType::Never, false, false)))
    {
        let key = ty.identity_key();
        index.type_facts_by_id.insert(
            key.clone(),
            TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "hostile layout".into(),
            },
        );
        assert_eq!(index.type_flags(ty), old(&index, ty));
        index.type_facts_by_id.get_mut(&key).unwrap().layout_key =
            "different layout with unchanged flags".into();
        let (_, observed) = crate::test_allocation_observer::measure(|| {
            assert_eq!(
                index.type_flags(ty),
                Some(TypeFlags {
                    copy: false,
                    contains_resource: true,
                    sized: false,
                    needs_drop: true
                })
            )
        });
        assert!(!observed.invalid);
        assert_eq!(
            (observed.allocations, observed.allocated, observed.live),
            (0, 0, 0)
        );
        index.type_facts_by_id.remove(&key);
    }
    assert_eq!(index.type_flags(&ResolvedType::Never), None);
}
#[test]
fn dynamic_flags_keep_owned_layout_unknown_generic_and_malformed_fallback_verdicts() {
    let source = crate::check(
        r#"module facts;
@id("leaf") record Leaf {@id("leaf.text") text:string,}
@id("row") record Row {@id("row.leaf") leaf:Leaf,@id("row.bytes") bytes:Bytes,@id("row.n") n:i64,}
@id("main") fn main()->i64 {0}
"#,
        "flag-facts.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let index = &program.declarations;
    let types = vec![
        ResolvedType::ArrayU8(0),
        ResolvedType::ArrayU8(65536),
        ResolvedType::Nominal {
            declaration: DeclarationId::new("row"),
            arguments: vec![],
        },
        ResolvedType::Nominal {
            declaration: DeclarationId::new("missing"),
            arguments: vec![],
        },
        ResolvedType::Nominal {
            declaration: DeclarationId::new("row"),
            arguments: vec![ResolvedType::I64],
        },
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("missing"),
            index: 0,
        },
        ResolvedType::Function {
            parameters: vec![ResolvedType::I64],
            result: Box::new(ResolvedType::I64),
        },
        ResolvedType::Function {
            parameters: vec![ResolvedType::Never],
            result: Box::new(ResolvedType::I64),
        },
    ];
    for ty in &types {
        assert_eq!(index.type_flags(ty), old(index, ty), "{ty:?}");
    }
    assert_eq!(
        index.type_flags(&types[2]),
        Some(TypeFlags {
            copy: false,
            contains_resource: false,
            sized: true,
            needs_drop: true
        })
    );
    assert!(index.type_flags(&types[3]).is_none());
    let cached = index.type_facts_by_id.clone();
    let leaf_types = leaves();
    let (_, warm) = crate::test_allocation_observer::measure(|| {
        for (ty, copy, needs_drop) in &leaf_types {
            assert_eq!(
                index.type_flags(ty),
                Some(TypeFlags {
                    copy: *copy,
                    contains_resource: false,
                    sized: true,
                    needs_drop: *needs_drop,
                })
            );
        }
    });
    assert!(!warm.invalid);
    assert_eq!((warm.allocations, warm.allocated, warm.live), (0, 0, 0));
    for (ty, _, _) in leaves() {
        assert_eq!(index.type_flags(&ty), old(index, &ty));
    }
    assert_eq!(
        index.type_facts_by_id, cached,
        "queries publish no new memo or layout facts"
    );
}

#[test]
fn dynamic_cached_flags_remove_only_retained_layout_clones_through_drop() {
    // Current cached facts are intentionally independent of language constants:
    // the full owned API remains the oracle even for hostile layout/flag facts.
    let mut index = DeclarationIndex::default();
    let ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("current.row"),
        arguments: vec![ResolvedType::I64],
    };
    let key = ty.identity_key();
    const LAYOUT: &str = "retained-layout-payload";
    const QUERIES: usize = 32;
    index.type_facts_by_id.insert(
        key,
        TypeFacts {
            copy: true,
            contains_resource: true,
            sized: false,
            needs_drop: true,
            layout_key: LAYOUT.to_owned(),
        },
    );
    let expected = Some(TypeFlags {
        copy: true,
        contains_resource: true,
        sized: false,
        needs_drop: true,
    });
    let (_, owned) = crate::test_allocation_observer::measure(|| {
        for _ in 0..QUERIES {
            let facts = std::hint::black_box(index.type_facts_owned_key_oracle(&ty));
            assert_eq!(facts.as_ref().map(TypeFlags::from), expected);
            drop(facts);
        }
    });
    let (_, borrowed) = crate::test_allocation_observer::measure(|| {
        for _ in 0..QUERIES {
            assert_eq!(index.type_flags(&ty), expected);
        }
    });
    // Independent saved key-only construction isolates every original scratch
    // request; the streamed query now removes those AND the owned layout clone.
    let (_, keys) = crate::test_allocation_observer::measure(|| {
        for _ in 0..QUERIES {
            drop(std::hint::black_box(ty.identity_key()));
        }
    });
    assert!(!owned.invalid && !borrowed.invalid && !keys.invalid);
    assert_eq!(owned.live, 0);
    assert_eq!(keys.live, 0);
    assert_eq!(owned.allocated, owned.freed);
    assert_eq!(keys.allocated, keys.freed);
    assert_eq!(owned.allocations, keys.allocations + QUERIES);
    assert_eq!(owned.allocated, keys.allocated + QUERIES * 23);
    assert_eq!(
        (
            borrowed.allocations,
            borrowed.reallocations,
            borrowed.allocated,
            borrowed.freed,
            borrowed.live
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(LAYOUT.len(), 23);
    assert_eq!(index.type_flags(&ty), old(&index, &ty));
}

#[test]
fn uncached_array_and_scalar_callable_flags_preserve_full_carriers_and_hostile_cache_precedence() {
    let mut index = DeclarationIndex::default();
    let mut types = vec![ResolvedType::ArrayU8(0), ResolvedType::ArrayU8(u32::MAX)];
    for count in [0, 1, 8, 9] {
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::I64; count],
            result: Box::new(ResolvedType::Bool),
        });
    }
    types.push(ResolvedType::Function {
        parameters: vec![ResolvedType::Bytes],
        result: Box::new(ResolvedType::I64),
    });
    for ty in &types {
        assert_eq!(index.type_flags(ty), old(&index, ty));
    }
    let (_, new) = crate::test_allocation_observer::measure(|| {
        for _ in 0..128 {
            for ty in &types {
                std::hint::black_box(index.type_flags(ty));
            }
        }
    });
    assert!(!new.invalid);
    assert_eq!((new.allocations, new.allocated, new.live), (0, 0, 0));
    let (_, original) = crate::test_allocation_observer::measure(|| {
        for _ in 0..128 {
            for ty in &types {
                std::hint::black_box(old(&index, ty));
            }
        }
    });
    assert!(!original.invalid && original.allocated > 0);
    assert_eq!(original.live, 0);
    assert_eq!(original.allocated, original.freed);
    for ty in &types {
        index.type_facts_by_id.insert(
            ty.identity_key(),
            TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "forged:λ".into(),
            },
        );
        assert_eq!(index.type_flags(ty), old(&index, ty));
    }
}
