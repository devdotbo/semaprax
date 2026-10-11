use super::*;

#[test]
fn contiguous_checked_views_allocate_nothing_and_materialize_only_at_owned_boundary() {
    for cache_first in [false, true] {
        let (pass, _, owner) = crate::project::incremental::checked_hir::tests::cold();
        let pointer = owner.functions.as_ptr();
        let wire = crate::cache_codec::encode(&*owner).unwrap();
        let (view, observed) = crate::test_allocation_observer::measure(|| {
            Retained::select(owner.clone(), |program| &program.functions, |_| true).unwrap()
        });
        assert_eq!(observed.allocations, 0);
        assert!(!observed.invalid);
        assert_eq!(pointer, view.as_ptr());
        assert!(matches!(view, Retained::Checked { .. }));
        if cache_first {
            drop(pass);
            drop(owner);
            assert_eq!(view.len(), 3);
            let owned = view.into_owned();
            assert_ne!(pointer, owned.as_ptr());
        } else {
            drop(view);
            assert_eq!(crate::cache_codec::encode(&*owner).unwrap(), wire);
            drop(pass);
            assert_eq!(pointer, owner.functions.as_ptr());
        }
    }
}

#[test]
fn noncontiguous_output_carrier_reservation_is_exact_sticky_and_precedes_clone() {
    let (_, _, owner) = crate::project::incremental::checked_hir::tests::cold();
    let expected = owner
        .functions
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .map(|(_, function)| function.clone())
        .collect::<Vec<_>>();
    let debit = 2 * std::mem::size_of::<hir::ResolvedFunction>();
    let select = || {
        Retained::select(
            owner.clone(),
            |program| &program.functions,
            |function| function.id.as_str() != "local.middle",
        )
    };
    // This isolated boundary tests the additional output carrier only. The
    // production cached full-tree prebound still pays nested HIR clone backing.
    let (result, overflow, charged, _) =
        crate::bounded_output::with_limit_usage_refusal(debit, select);
    assert!(!overflow);
    assert_eq!(charged, debit);
    let result = result.unwrap();
    assert!(matches!(result, Retained::Owned(_)));
    assert_eq!(
        crate::cache_codec::encode(&result.into_owned()).unwrap(),
        crate::cache_codec::encode(&expected).unwrap()
    );
    let ((first, retry), observed) = crate::test_allocation_observer::measure(|| {
        let (pair, overflow, _, _) =
            crate::bounded_output::with_limit_usage_refusal(debit - 1, || {
                let first = select().is_err();
                let retry = reserve_builder_structure(0).is_err();
                (first, retry)
            });
        assert!(overflow);
        pair
    });
    assert!(first && retry);
    assert!(!observed.invalid);
    // Diagnostic allocation is retained in the observation; no selected HIR
    // body is published on the refused output-carrier boundary.
    assert_eq!(owner.functions.len(), 3);
}
