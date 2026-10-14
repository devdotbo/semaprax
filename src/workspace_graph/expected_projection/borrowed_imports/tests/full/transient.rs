//! Real non-retaining production input, with the complete receiving route intact.
use super::*;
use crate::test_allocation_observer::Metrics;
use crate::workspace_graph::expected_projection::borrowed_leaf::Input;

fn observe(
    programs: &[crate::ast::Program],
    authored: &std::collections::BTreeMap<&str, super::super::AuthoredDeclaration<'_>>,
    registry: &LayoutRegistry<'_>,
    limit: usize,
    transient: bool,
) -> (bool, Metrics, bool, usize, bool) {
    let ((ok, metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let input = if transient {
                Input::construct_transient(&programs[0], authored, programs, registry)
            } else {
                Input::construct_receiving(&programs[0], authored, programs, registry, true)
            };
            let ok = input.is_ok();
            if !ok {
                assert!(reserve_builder_structure(0).is_err());
            }
            drop(input);
            ok
        })
    });
    (ok, metrics, refused, used, first.is_some())
}

#[test]
fn actual_transient_import_constructor_removes_only_authored_cache_spare_and_pays_drop() {
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_CALLER, INVARIANT_PROVIDER),
    ] {
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        drop(registry.get(&programs[0], &authored, &programs).unwrap());
        let (ok, current, refused, paid, first) =
            observe(&programs, &authored, &registry, usize::MAX, true);
        let (old_ok, old, old_refused, old_paid, old_first) =
            observe(&programs, &authored, &registry, usize::MAX, false);
        assert!(ok && old_ok && !refused && !old_refused && !first && !old_first);
        let removed = programs[0].functions.len() * std::mem::size_of::<crate::ast::Function>()
            + programs[0].types.len() * std::mem::size_of::<crate::ast::TypeDeclaration>();
        assert_eq!(old.allocated, current.allocated + removed);
        let generated =
            Input::construct_transient(&programs[0], &authored, &programs, &registry).unwrap();
        let view = generated.borrowed().unwrap();
        let removed_allocations = usize::from(
            view.functions().len() == programs[0].functions.len()
                && !programs[0].functions.is_empty(),
        ) + usize::from(
            view.types().len() == programs[0].types.len() && !programs[0].types.is_empty(),
        );
        assert_eq!(old.allocations, current.allocations + removed_allocations);
        drop(generated);
        assert!(paid < old_paid);
        for (actual, charged) in [(current, paid), (old, old_paid)] {
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            assert!(actual.allocated <= charged && actual.highwater <= charged);
        }
        let (ok, actual, refused, used, first) =
            observe(&programs, &authored, &registry, paid, true);
        assert!(ok && !refused && !first && used == paid && !actual.invalid && actual.live == 0);
        for limit in [0, paid - 1] {
            let (ok, actual, refused, used, first) =
                observe(&programs, &authored, &registry, limit, true);
            assert!(
                !ok && refused
                    && first
                    && used <= limit
                    && !actual.invalid
                    && actual.live == 0
                    && actual.allocated == actual.freed,
                "module={} limit={limit} paid={paid} used={used} ok={ok} refused={refused} first={first} actual={actual:?}",
                programs[0].module
            );
        }
    }
}

#[test]
fn actual_transient_full_projection_and_resolution_match_complete_old_receivers() {
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_CALLER, INVARIANT_PROVIDER),
    ] {
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        let original = old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap();
        let input =
            Input::construct_transient(&programs[0], &authored, &programs, &registry).unwrap();
        let view = input.borrowed().unwrap();
        assert_eq!(
            view.functions().iter().collect::<Vec<_>>(),
            original.functions.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            view.types().iter().collect::<Vec<_>>(),
            original.types.iter().collect::<Vec<_>>()
        );
        for (ordinal, function) in programs[0].functions.iter().enumerate() {
            assert!(std::ptr::eq(
                view.functions().get(ordinal).unwrap(),
                function
            ));
        }
        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
        let current =
            linked(|| hir::resolve_source(view, layout.presentation(&programs[0], view).unwrap()));
        let expected = linked(|| {
            hir::resolve_with_function_reuse(
                &original,
                None,
                None,
                None,
                layout.presentation(&programs[0], &original).unwrap(),
            )
        });
        match (current, expected) {
            (Ok((current, _)), Ok((expected, _))) => {
                assert_eq!(current, expected);
                let a = hir::validate(&current);
                let b = hir::validate(&expected);
                assert_eq!(
                    a.as_ref().err().map(Diagnostic::json),
                    b.as_ref().err().map(Diagnostic::json)
                );
            }
            (Err(current), Err(expected)) => assert_eq!(
                current.iter().map(Diagnostic::json).collect::<Vec<_>>(),
                expected.iter().map(Diagnostic::json).collect::<Vec<_>>()
            ),
            _ => panic!("full receiver mismatch"),
        }
        drop(layout);
        // Test-only publication of a transient input retains its full old fallback.
        assert_eq!(input.into_checked_key().unwrap(), original);
    }
}
