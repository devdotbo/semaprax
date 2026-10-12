//! Saved full prechange TypeFacts query and scoped test-only receiver policy.
//! This policy has no production build, authority, cache entry or heap carrier.
use super::*;
use std::cell::Cell;
thread_local! { static OLD: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn enabled() -> bool { OLD.with(Cell::get) }
struct Restore(bool);
impl Drop for Restore {
    fn drop(&mut self) { OLD.with(|old| old.set(self.0)); }
}
fn with_old<R>(run: impl FnOnce() -> R) -> R {
    let previous = OLD.with(|old| old.replace(true));
    let _restore = Restore(previous);
    run()
}
impl DeclarationIndex {
    // Original public API body, unchanged. Miss computation remains shared,
    // and its own full semantic/recursive oracles remain independently owned.
    pub(crate) fn type_facts_owned_key_oracle(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.type_facts_by_id
            .get(&ty.identity_key())
            .cloned()
            .or_else(|| self.recompute_type_facts(ty))
    }
}
#[test]
fn full_facts_retains_exact_layout_clone_and_hostile_cache_precedence_through_drop() {
    use crate::test_allocation_observer::measure;
    let mut index = DeclarationIndex::default();
    const LAYOUT: &str = "retained-layout-payload";
    const QUERIES: usize = 32;
    let types = [
        ResolvedType::Never,
        ResolvedType::Nominal { declaration: super::super::DeclarationId::new("row"), arguments: Vec::new() },
        ResolvedType::Nominal { declaration: super::super::DeclarationId::new("é\0:row"), arguments: vec![ResolvedType::I64] },
        ResolvedType::Function { parameters: vec![ResolvedType::String], result: Box::new(ResolvedType::ArrayU8(8192)) },
    ];
    let facts = TypeFacts { copy: false, contains_resource: true, sized: false, needs_drop: true, layout_key: LAYOUT.to_owned() };
    for ty in &types { index.type_facts_by_id.insert(ty.identity_key(), facts.clone()); }
    let retained = index.type_facts_by_id.clone();
    for ty in &types {
        let cached_pointer = index.type_facts_by_id.get(&ty.identity_key()).unwrap().layout_key.as_ptr();
        let (_, old) = measure(|| {
            for _ in 0..QUERIES {
                let actual = index.type_facts_owned_key_oracle(ty).unwrap();
                assert_eq!(actual, facts);
                assert_ne!(actual.layout_key.as_ptr(), facts.layout_key.as_ptr());
                drop(actual);
            }
        });
        let (_, keys) = measure(|| {
            for _ in 0..QUERIES { drop(std::hint::black_box(ty.identity_key())); }
        });
        let (_, new) = measure(|| {
            for _ in 0..QUERIES {
                let actual = index.type_facts(ty).unwrap();
                assert_eq!(actual, facts);
                assert_ne!(actual.layout_key.as_ptr(), cached_pointer);
                drop(actual);
            }
        });
        assert!(!old.invalid && !keys.invalid && !new.invalid);
        assert_eq!(old.allocations, keys.allocations + QUERIES);
        assert_eq!(old.allocated, keys.allocated + QUERIES * LAYOUT.len());
        assert_eq!(new.allocations, QUERIES);
        assert_eq!(new.allocated, QUERIES * LAYOUT.len());
        for observed in [old, keys, new] {
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
    assert_eq!(index.type_facts_by_id, retained);
    for ty in [ResolvedType::TypeParameter { owner: super::super::DeclarationId::new("missing"), index: 9 }, ResolvedType::Nominal { declaration: super::super::DeclarationId::new("missing"), arguments: vec![ResolvedType::Bool] }] {
        assert_eq!(index.type_facts(&ty), index.type_facts_owned_key_oracle(&ty));
    }
}
#[test]
fn ordinary_generic_and_invariant_sources_preserve_full_owned_query_diagnostics_and_hir() {
    const GENERIC: &str = r#"module test.generic_template_closure;
@id("gc.fill") fn fill<T>(values:own Vec<T>,replacement:T)->Vec<T>{
    let length=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(length);
    let mut index=0usize;
    while index<length {
        let callback=fn(item:T)->T{replacement};
        output=vec_push<T>(output,callback(vec_get<T>(values,index)));
        index=index+1usize;
        index<length
    }
    output
}
@id("gc.main") fn main()->i64{0}
"#;
    const INVARIANT: &str = r#"module identity.ordinary;
@id("range.type") record Range {
    @id("range.lo") lo:i64,
    @id("range.hi") hi:i64,
} requires lo <= hi
@id("app.main") fn main()->i64 {
    let row = Range { lo:1, hi:2 };
    row.hi
}
"#;
    for source in [GENERIC, INVARIANT] {
        let program = crate::parse(source, "full-fact-key.spx").unwrap();
        let old = with_old(|| crate::hir::analyze(&program));
        let new = crate::hir::analyze(&program);
        assert!(old.resolved.is_some());
        assert_eq!(new.diagnostics, old.diagnostics);
        assert_eq!(new.resolved, old.resolved);
        let mut hostile = new.resolved.unwrap();
        let ty = hostile.functions[0].return_type.clone();
        hostile.declarations.type_facts_by_id.insert(ty.identity_key(), TypeFacts {
            copy: false, contains_resource: true, sized: false, needs_drop: true,
            layout_key: "stale".to_owned(),
        });
        let old = with_old(|| crate::hir::validate(&hostile));
        let new = crate::hir::validate(&hostile);
        assert!(old.is_err());
        assert_eq!(new, old);
    }
    // Preserve a source-first invariant diagnostic before any fact query.
    let invalid = crate::parse(&INVARIANT.replace("lo <= hi", "lo"), "full-fact-key.spx").unwrap();
    let old = with_old(|| crate::hir::analyze(&invalid));
    let new = crate::hir::analyze(&invalid);
    assert!(old.resolved.is_none());
    assert_eq!(new.diagnostics, old.diagnostics);
    assert_eq!(new.resolved, old.resolved);
}
