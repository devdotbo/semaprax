use super::*;
use crate::hir::declaration_index::flags::TypeFlags;
use crate::hir::{DeclarationId, DeclarationIndex, TypeFacts};
use crate::test_allocation_observer::measure;
fn nominal(name: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(name),
        arguments,
    }
}
fn cases() -> Vec<ResolvedType> {
    let mut types = vec![
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
        ResolvedType::StringMap,
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
        ResolvedType::ArrayU8(0),
        ResolvedType::ArrayU8(10),
        ResolvedType::ArrayU8(u32::MAX),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("é\0:owner"),
            index: 19,
        },
        nominal("", vec![]),
        nominal(
            "é✓\0:row",
            vec![ResolvedType::I64, ResolvedType::ArrayU8(8192)],
        ),
        ResolvedType::Function {
            parameters: vec![],
            result: Box::new(ResolvedType::Never),
        },
        ResolvedType::Function {
            parameters: vec![
                ResolvedType::String,
                nominal("row", vec![ResolvedType::Bool]),
            ],
            result: Box::new(ResolvedType::Bytes),
        },
    ];
    types.push(nominal(
        "outer:0:",
        vec![types[26].clone(), nominal("inner", vec![ResolvedType::Str])],
    ));
    types.push(ResolvedType::TypeParameter {
        owner: DeclarationId::new(""),
        index: u32::MAX,
    });
    types
}
#[test]
fn compound_key_bytes_cache_facts_and_order_match_the_full_saved_builder() {
    let types = cases();
    let mut index = DeclarationIndex::default();
    for (ordinal, ty) in types.iter().enumerate() {
        let key = ty.identity_key();
        let plan = Plan::new(ty).unwrap();
        assert_eq!(plan.nodes[0].unwrap().length, key.len());
        assert_eq!(plan.bytes().collect::<Vec<_>>(), key.as_bytes());
        index.type_facts_by_id.insert(
            key,
            TypeFacts {
                copy: ordinal % 2 == 0,
                contains_resource: true,
                sized: false,
                needs_drop: ordinal % 3 == 0,
                layout_key: "forged-layout-is-retained".to_owned(),
            },
        );
    }
    for key in [
        "",
        "function:v1:00:",
        "nominal:01:x:0:",
        "parameter:0::000",
        "zz",
    ] {
        index.type_facts_by_id.insert(
            key.to_owned(),
            TypeFacts {
                copy: false,
                contains_resource: false,
                sized: true,
                needs_drop: true,
                layout_key: "neighbor".to_owned(),
            },
        );
    }
    for ty in &types {
        let old = index.type_facts_owned_key_oracle(ty);
        assert_eq!(index.type_flags(ty), old.as_ref().map(TypeFlags::from));
        let query = super::super::Compound(Plan::new(ty).unwrap());
        for stored in index.type_facts_by_id.keys() {
            let stored_query: &dyn super::super::Key = stored;
            assert_eq!(
                (&query as &dyn super::super::Key).cmp(stored_query),
                ty.identity_key().cmp(stored)
            );
        }
    }
    // A real uncached compound miss retains the complete old recomputation.
    let missing = nominal("foreign", vec![ResolvedType::I64]);
    assert_eq!(
        index.type_flags(&missing),
        index
            .type_facts_owned_key_oracle(&missing)
            .as_ref()
            .map(TypeFlags::from)
    );
}
#[test]
fn compound_key_exact_node_byte_and_work_boundaries_keep_owned_fallback() {
    let mut exact = ResolvedType::Bool;
    for _ in 1..NODES {
        exact = nominal("n", vec![exact]);
    }
    let old = exact.identity_key();
    let plan = Plan::new(&exact).unwrap();
    assert_eq!(plan.count, NODES);
    assert_eq!(plan.bytes().collect::<Vec<_>>(), old.as_bytes());
    let over = nominal("n", vec![exact]);
    assert!(Plan::new(&over).is_none());
    let mut index = DeclarationIndex::default();
    index.type_facts_by_id.insert(
        over.identity_key(),
        TypeFacts {
            copy: true,
            contains_resource: true,
            sized: false,
            needs_drop: true,
            layout_key: "fallback".to_owned(),
        },
    );
    assert_eq!(
        index.type_flags(&over),
        index
            .type_facts_owned_key_oracle(&over)
            .as_ref()
            .map(TypeFlags::from)
    );
    let simple = nominal("row", vec![ResolvedType::I64]);
    const KEY: &str = "nominal:3:row:1:3:i64";
    assert_eq!(simple.identity_key(), KEY);
    // Independent census: root dispatch constructs the child in the same
    // visit, child finishes, root finishes, then its child length is folded.
    // These are four visits; constructing a child is not a separate dispatch.
    assert!(Plan::with_limits(&simple, 2, KEY.len(), 4).is_some());
    assert!(Plan::with_limits(&simple, 1, KEY.len(), 4).is_none());
    assert!(Plan::with_limits(&simple, 2, KEY.len() - 1, 4).is_none());
    assert!(Plan::with_limits(&simple, 2, KEY.len(), 3).is_none());
    let function = ResolvedType::Function {
        parameters: vec![ResolvedType::I64, ResolvedType::ArrayU8(10)],
        result: Box::new(ResolvedType::Bool),
    };
    const FUNCTION_KEY: &str = "function:v1:2:3:i6411:array:u8:104:bool";
    assert_eq!(function.identity_key(), FUNCTION_KEY);
    assert_eq!(
        Plan::new(&function).unwrap().bytes().collect::<Vec<_>>(),
        FUNCTION_KEY.as_bytes()
    );
    let wide = nominal("w", vec![ResolvedType::I64; NODES]);
    assert!(Plan::new(&wide).is_none());
    let huge = nominal(&"x".repeat(MAX_BYTES), vec![ResolvedType::I64]);
    assert!(Plan::new(&huge).is_none());
}
#[test]
fn cached_compound_query_has_zero_heap_and_removes_only_real_old_backing_through_drop() {
    let ty = nominal("row", vec![ResolvedType::I64]);
    const KEY: &str = "nominal:3:row:1:3:i64";
    const LAYOUT: &str = "owned-cache-layout";
    const QUERIES: usize = 32;
    assert_eq!(ty.identity_key(), KEY);
    let mut index = DeclarationIndex::default();
    index.type_facts_by_id.insert(
        KEY.to_owned(),
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
    let (_, old) = measure(|| {
        for _ in 0..QUERIES {
            let facts = index.type_facts_owned_key_oracle(std::hint::black_box(&ty));
            assert_eq!(facts.as_ref().map(TypeFlags::from), expected);
            drop(facts);
        }
    });
    let (_, new) = measure(|| {
        for _ in 0..QUERIES {
            assert_eq!(index.type_flags(std::hint::black_box(&ty)), expected);
        }
    });
    assert!(!old.invalid && !new.invalid);
    assert_eq!(old.live, 0);
    assert_eq!(old.allocated, old.freed);
    assert!(old.allocated >= QUERIES * (KEY.len() + LAYOUT.len()));
    assert!(old.allocations >= QUERIES * 2);
    assert_eq!(
        (
            new.allocations,
            new.reallocations,
            new.allocated,
            new.freed,
            new.live
        ),
        (0, 0, 0, 0, 0)
    );
}

#[test]
fn compound_lookup_does_not_reset_an_existing_physical_first_refusal() {
    let ty = nominal("row", vec![ResolvedType::I64]);
    let mut index = DeclarationIndex::default();
    index.type_facts_by_id.insert(
        ty.identity_key(),
        TypeFacts {
            copy: true,
            contains_resource: false,
            sized: true,
            needs_drop: false,
            layout_key: "held".to_owned(),
        },
    );
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
    // Fixed ledger construction is outside the query observer. The query has
    // no physical allowance and cannot turn the caller's failed payer usable.
    let (((), observed), failed, used, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            assert!(!reserve_active_required(1));
            assert!(index.type_flags(&ty).unwrap().copy);
            assert!(!reserve_active_required(0));
        })
    });
    assert!(failed);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, 1);
    assert_eq!(
        (observed.allocations, observed.allocated, observed.live),
        (0, 0, 0)
    );
}
