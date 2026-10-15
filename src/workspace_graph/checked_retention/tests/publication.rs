//! The final owned boundary remains a real deep clone. This scope measures
//! only that boundary and its Drop; the actual cold owner and old clone oracle
//! are held beforehand. It does not claim that final publication is copy-free.
use super::super::*;

#[test]
fn final_owned_materialization_is_independent_and_settles_its_actual_backings() {
    let (pass, _, owner) = crate::project::incremental::checked_hir::tests::cold();
    let original = owner.functions.as_ptr();
    let expected = owner.functions.clone();
    let expected_wire = crate::cache_codec::encode(&expected).unwrap();
    let ((), observed) = crate::test_allocation_observer::measure(|| {
        let view = Retained::select(owner.clone(), |program| &program.functions, |_| true).unwrap();
        assert_eq!(view.as_ptr(), original);
        let published = view.into_owned();
        assert_ne!(published.as_ptr(), original);
        assert_eq!(published, expected);
        assert_eq!(
            crate::cache_codec::encode(&published).unwrap(),
            expected_wire
        );
        for (published, original) in published.iter().zip(&owner.functions) {
            assert!(!std::ptr::eq(&published.body, &original.body));
        }
        drop(published);
    });
    assert!(!observed.invalid);
    assert!(observed.allocations > 0);
    assert!(observed.allocated >= observed.highwater);
    assert_eq!(observed.allocated, observed.freed);
    assert_eq!(observed.live, 0);
    println!("OPT777 requested-layout scope=final_owned_materialization_drop allocations={} reallocations={} requested={} highwater={} freed={} live={} held_owner_and_observer_metadata=excluded", observed.allocations, observed.reallocations, observed.allocated, observed.highwater, observed.freed, observed.live);
    // No publication mutation may escape into the current checked owner.
    assert_eq!(owner.functions.as_ptr(), original);
    assert_eq!(owner.functions, expected);
    crate::hir::validate(&owner).unwrap();
    drop(pass);
    crate::hir::validate(&owner).unwrap();
}
