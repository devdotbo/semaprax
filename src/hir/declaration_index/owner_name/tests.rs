//! Full old tuple lookups, exact reference parity and literal allocation controls.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::hir::{CompilerDeclaration, DeclarationIndex};
use crate::test_allocation_observer::measure;

// Complete prior receivers retain their independent key acquisition and Drop.
fn old_field_id<'a>(
    index: &'a DeclarationIndex,
    owner: &DeclarationId,
    name: &str,
) -> Option<&'a DeclarationId> {
    index
        .fields_by_owner_name
        .get(&(owner.clone(), name.to_owned()))
}
fn old_case_id<'a>(
    index: &'a DeclarationIndex,
    owner: &DeclarationId,
    name: &str,
) -> Option<&'a DeclarationId> {
    index
        .cases_by_owner_name
        .get(&(owner.clone(), name.to_owned()))
}
fn pointer(value: Option<&DeclarationId>) -> Option<*const DeclarationId> {
    value.map(|value| value as *const DeclarationId)
}
fn probes(index: &DeclarationIndex, owner: &DeclarationId, name: &str) {
    for case in [false, true] {
        let old = || {
            if case {
                old_case_id(index, owner, name)
            } else {
                old_field_id(index, owner, name)
            }
        };
        let new = || {
            if case {
                index.case_id(owner, name)
            } else {
                index.field_id(owner, name)
            }
        };
        let (before, old_metrics) = measure(old);
        let ((after, new_metrics), refused, used, first) =
            with_limit_usage_refusal(0, || measure(new));
        assert_eq!(
            pointer(before),
            pointer(after),
            "exact returned map value reference"
        );
        assert!(!refused && used == 0 && first.is_none());
        assert!(
            !new_metrics.invalid && new_metrics.allocations == 0 && new_metrics.reallocations == 0
        );
        assert!(new_metrics.allocated == 0 && new_metrics.live == 0 && new_metrics.freed == 0);
        assert!(!old_metrics.invalid && old_metrics.live == 0 && old_metrics.reallocations == 0);
        assert_eq!(old_metrics.allocations, usize::from(!name.is_empty()));
        assert_eq!(
            old_metrics.allocated,
            name.len(),
            "original name-copy Layout"
        );
        assert_eq!(
            old_metrics.freed,
            name.len(),
            "complete old temporary key Drop"
        );
        // Shared/static identity clone preserves its carrier and allocates no
        // backing already at this base; only discarded name bytes are removed.
        let (_, clone_metrics) = measure(|| drop(owner.clone()));
        assert!(!clone_metrics.invalid && clone_metrics.allocated == 0 && clone_metrics.live == 0);
    }
}
#[test]
fn borrowed_owner_name_preserves_hostile_tuple_order_duplicates_and_exact_references() {
    let owners = [
        DeclarationId::new(""),
        DeclarationId::new("owner"),
        DeclarationId::new("owner.prefix"),
        DeclarationId::new("owner.é"),
        DeclarationId::new("owner.e\u{301}"),
        DeclarationId::compiler(CompilerDeclaration::Option),
        DeclarationId::new("core.option"),
    ];
    let names = ["", "a", "aa", "a\0", "é", "e\u{301}", "完全", "missing"];
    let mut index = DeclarationIndex::default();
    for (ordinal, owner) in owners.iter().enumerate() {
        for (name_ordinal, name) in names[..names.len() - 1].iter().enumerate() {
            let key = (owner.clone(), (*name).to_owned());
            let value = DeclarationId::new(format!("field.{ordinal}.{name_ordinal}"));
            index
                .fields_by_owner_name
                .insert(key.clone(), value.clone());
            index.cases_by_owner_name.insert(key, value);
        }
    }
    // Duplicate text from a distinct identity carrier replaces exactly as the
    // unchanged BTree tuple key dictates, retaining its actual stored reference.
    let replacement = DeclarationId::new("duplicate.last");
    index
        .fields_by_owner_name
        .insert((owners[1].clone(), "a".to_owned()), replacement.clone());
    index
        .cases_by_owner_name
        .insert((owners[1].clone(), "a".to_owned()), replacement.clone());
    assert_eq!(index.field_id(&owners[1], "a"), Some(&replacement));
    assert_eq!(index.case_id(&owners[1], "a"), Some(&replacement));
    for owner in &owners {
        for name in names {
            probes(&index, owner, name);
        }
        probes(&index, owner, &"hostile.long.完全".repeat(512));
    }
    for owner in [DeclarationId::new("foreign"), DeclarationId::new("owner.")] {
        for name in names {
            probes(&index, &owner, name);
        }
    }
    let keys: Vec<_> = index.fields_by_owner_name.keys().collect();
    for left in &keys {
        for right in &keys {
            let left_query = Query(&left.0, &left.1);
            let right_query = Query(&right.0, &right.1);
            let borrowed_left: &dyn OwnerName = &left_query;
            let borrowed_right: &dyn OwnerName = &right_query;
            let stored_left: &dyn OwnerName = *left;
            let stored_right: &dyn OwnerName = *right;
            assert_eq!(borrowed_left.cmp(borrowed_right), left.cmp(right));
            assert_eq!(stored_left.cmp(borrowed_right), left.cmp(right));
            assert_eq!(borrowed_left.cmp(stored_right), left.cmp(right));
            assert_eq!(stored_left.cmp(stored_right), left.cmp(right));
        }
    }
}
const SOURCE: &str = r#"module lookup.real;
@id("lookup.generic") record Generic<T> { @id("lookup.generic.value") value:T, }
@id("lookup.choice") variant Choice<T> {
 @id("lookup.choice.none") None,
 @id("lookup.choice.some") Some { @id("lookup.choice.value") value:T, },
}
@id("lookup.counter") class Counter { @id("lookup.counter.value") value:i64, }
@id("lookup.main") fn main()->i64 {0}
"#;
#[test]
fn borrowed_owner_name_real_class_generic_variant_and_compiler_owners_keep_full_old_lookup() {
    let ast = crate::check(SOURCE, "owner-name.spx").unwrap();
    let resolved = crate::hir::resolve(&ast).unwrap();
    let index = &resolved.declarations;
    for ((owner, name), value) in &index.fields_by_owner_name {
        assert_eq!(index.field_id(owner, name), Some(value));
        probes(index, owner, name);
        probes(index, owner, "missing");
    }
    for ((owner, name), value) in &index.cases_by_owner_name {
        assert_eq!(index.case_id(owner, name), Some(value));
        probes(index, owner, name);
        probes(index, owner, "missing");
    }
    // Check production text directly: the assertion's own literals must not
    // satisfy this drift guard. The complete source owner joins both children.
    let production = include_str!("../owner_name.rs");
    assert!(production.contains("Borrow<dyn OwnerName + 'a>"));
    assert!(production.contains("values.get::<dyn OwnerName + '_>"));
    for forbidden in [
        ".iter()",
        "unsafe",
        "to_owned",
        ".clone()",
        "BTreeMap::new",
        "collect(",
    ] {
        assert!(
            !production.contains(forbidden),
            "lookup carrier drift: {forbidden}"
        );
    }
}
