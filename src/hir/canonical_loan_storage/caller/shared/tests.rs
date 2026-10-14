use super::*;
use crate::test_allocation_observer::measure;
fn empty(program: &ResolvedProgram) -> Storage<'_> {
    Storage {
        program,
        ids: Vec::new(),
        liveness: Vec::new(),
        origins: Vec::new(),
        _scratch: Scratch::default(),
    }
}
#[test]
fn exact_rc_carrier_prepaid_before_acquisition_zero_short_sticky_and_weak_receiving_drop() {
    let program = super::super::super::tests::fixture();
    let model = bytes::<Storage<'_>>(Payer::LegacyUnobserved).unwrap();
    let observe = |limit| {
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let result = seal(empty(&program), Payer::current());
                let ok = result.is_ok();
                if let Ok(owner) = result {
                    let weak = Rc::downgrade(&owner);
                    let shared = Rc::clone(&owner);
                    assert!(Rc::ptr_eq(&owner, &shared));
                    drop(owner);
                    assert!(weak.upgrade().is_some());
                    drop(shared);
                    assert!(weak.upgrade().is_none());
                    drop(weak);
                } else {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                ok
            })
        })
    };
    for limit in [usize::MAX, model] {
        let ((ok, metrics), refused, paid) = observe(limit);
        assert!(ok && !refused && !metrics.invalid);
        assert_eq!(paid, model);
        assert_eq!(metrics.allocated, model);
        assert_eq!(metrics.freed, model);
        assert_eq!(metrics.live, 0);
    }
    for limit in [0, model - 1] {
        let ((ok, metrics), refused, paid) = observe(limit);
        assert!(!ok && refused && !metrics.invalid);
        assert_eq!(paid, 0);
        assert_eq!(metrics.allocated, 0);
        assert_eq!(metrics.live, 0);
    }
}
#[test]
fn complete_original_preparation_and_sealed_identity_route_keep_full_storage_and_diagnostics() {
    let program = super::super::super::tests::fixture();
    let seed = ConstructorSeed::authenticate(&program, None).unwrap();
    let (old, _) = super::super::owned_before::prepare_original(&seed, &program).unwrap();
    let (current, _) = super::super::prepare_with_comparison(&seed, &program).unwrap();
    assert_eq!(old.ids, current.ids);
    assert_eq!(old.liveness, current.liveness);
    assert_eq!(old.origins, current.origins);
    let other = program.clone();
    let old = super::super::owned_before::prepare_original(&seed, &other)
        .err()
        .unwrap();
    let current = super::super::prepare_with_comparison(&seed, &other)
        .err()
        .unwrap();
    assert_eq!(current, old);
}
