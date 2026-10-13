//! Actual full constructor/receiving/Drop; old standalone receiver is retained.
use super::super::initialization;
use super::{
    construct, linked, old, reserve_builder_structure, sources, LayoutRegistry, BOX_CALLER,
    BOX_PROVIDER, CLASS_CALLER, CLASS_PROVIDER, INVARIANT_CALLER, INVARIANT_PROVIDER, VEC_CALLER,
    VEC_PROVIDER,
};
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::{active_limit, active_remaining, with_limit_usage_refusal};
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
use crate::workspace_graph::expected_projection::borrowed_imports::construct_receiving;
use crate::workspace_graph::expected_projection::borrowed_leaf::Input;
use crate::workspace_graph::index_authored;

// Allocation-free phase snapshots: label, metrics, paid ledger bytes.
type Snap = Option<(&'static str, Metrics, usize)>;
fn snap(slots: &mut [Snap; 9], index: usize, label: &'static str) {
    let paid = match (active_limit(), active_remaining()) {
        (Some(limit), Some(remaining)) => {
            limit.checked_sub(remaining).expect("valid active ledger")
        }
        _ => panic!("phase requires an active ledger"),
    };
    slots[index] = Some((label, current_metrics(), paid));
}

#[test]
fn full_receiving_overlay_moves_exact_paid_directories_and_preserves_original_order() {
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_CALLER, INVARIANT_PROVIDER),
    ] {
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        let old = old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap();
        let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let costs = super::super::super::super::synthetic_builder_bytes_scoped_with_registry(
                &programs[0],
                &authored,
                &programs,
                None,
                4,
                true,
                Some(&registry),
            )
            .unwrap();
            reserve_builder_structure(costs.raw_clone_and_hir).unwrap();
            let view = construct_receiving(&programs[0], &authored, &programs, &registry).unwrap();
            assert_eq!(view.functions().len(), old.functions.len());
            assert_eq!(view.types().len(), old.types.len());
            assert_eq!(
                view.functions().iter().collect::<Vec<_>>(),
                old.functions.iter().collect::<Vec<_>>()
            );
            assert_eq!(
                view.types().iter().collect::<Vec<_>>(),
                old.types.iter().collect::<Vec<_>>()
            );
            let key = Input::Borrowed(view).into_receiving_key().unwrap();
            assert_eq!(key.functions, old.functions);
            assert_eq!(key.types, old.types);
            assert_eq!(key.path, old.path);
            assert_eq!(key.module, old.module);
            assert_eq!(key.permits, old.permits);
            assert_eq!(key.interfaces, old.interfaces);
            assert_eq!(key.protocols, old.protocols);
            assert_eq!(key.session_protocols, old.session_protocols);
            assert_eq!(key.agents, old.agents);
            assert_eq!(key.module_uses, old.module_uses);
            assert_eq!(key.implementations, old.implementations);
        });
        assert!(!refused);
    }
}

#[test]
fn full_standalone_and_receiving_overlay_construction_retention_and_drop_tradeoffs() {
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_CALLER, INVARIANT_PROVIDER),
    ] {
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        // Exact pointer-attributed prelude owner control, before measurements.
        initialization::complete_original_receiver_before_measurement(&programs);
        let observe = |limit, current, retain| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut snaps: [Snap; 9] = [None; 9];
                    let registry = LayoutRegistry::new(&programs, &authored);
                    let result = (|| {
                        let costs = super::super::super::super::synthetic_builder_bytes_scoped_with_registry(
                    &programs[0], &authored, &programs, None, 4, true, Some(&registry),
                )?;
                        snap(&mut snaps, 0, "costs-before-reserve");
                        reserve_builder_structure(costs.raw_clone_and_hir)?;
                        let view = if current {
                            construct_receiving(&programs[0], &authored, &programs, &registry)?
                        } else {
                            construct(&programs[0], &authored, &programs, &registry)?
                        };
                        snap(&mut snaps, 1, "constructor");
                        let layout = registry.get(&programs[0], &authored, &programs)?;
                        let presentation = layout.presentation(&programs[0], &view)?;
                        snap(&mut snaps, 2, "presentation");
                        let resolved = linked(|| {
                            hir::identity_resolution::resolve_source(&view, presentation, |bytes| {
                                reserve_builder_structure(bytes)
                            })
                        })
                        .map_err(|error| match error {
                            hir::identity_resolution::Failure::Source(errors) => errors,
                            hir::identity_resolution::Failure::Semantic(error) => vec![error],
                            hir::identity_resolution::Failure::Identity(errors) => errors,
                        })?;
                        snap(&mut snaps, 3, "resolution");
                        hir::validate(&resolved.0).map_err(|error| vec![error])?;
                        snap(&mut snaps, 4, "validation");
                        drop(resolved);
                        snap(&mut snaps, 5, "resolved-drop");
                        if retain {
                            let input = Input::Borrowed(view);
                            drop(if current {
                                input.into_receiving_key()?
                            } else {
                                input.into_checked_key()?
                            });
                        } else {
                            drop(view);
                        }
                        snap(&mut snaps, 6, "key-or-view-drop");
                        Ok::<(), Vec<Diagnostic>>(())
                    })();
                    if crate::bounded_output::active_refused() {
                        assert!(result.is_err());
                        assert!(reserve_builder_structure(0).is_err());
                    }
                    assert!(result.is_ok() || crate::bounded_output::active_refused());
                    drop(result);
                    snap(&mut snaps, 7, "result-drop");
                    drop(registry);
                    snap(&mut snaps, 8, "registry-drop");
                    snaps
                })
            })
        };
        for retain in [false, true] {
            let mut observations = Vec::new();
            for current in [false, true] {
                let ((whole_snaps, whole), refused, required, _) =
                    observe(usize::MAX, current, retain);
                assert!(!refused && !whole.invalid && whole.live == 0,
                    "current={current} retain={retain} refused={refused} required={required} whole={whole:?} phases={whole_snaps:?}");
                assert_eq!(whole.allocated, whole.freed, "phases={whole_snaps:?}");
                assert!(whole.allocated <= required && whole.highwater <= required,
                    "current={current} retain={retain} required={required} whole={whole:?} phases={whole_snaps:?}");
                // Snapshots were taken inside the observer; report only now.
                assert!(
                    whole_snaps.iter().all(|slot| slot.is_some()),
                    "missing phase snapshot: {whole_snaps:?}"
                );
                assert!(
                    whole_snaps
                        .windows(2)
                        .all(|pair| pair[0].unwrap().2 <= pair[1].unwrap().2),
                    "paid ledger decreased across phases: {whole_snaps:?}"
                );
                assert!(
                    whole_snaps[8].unwrap().2 <= required,
                    "paid beyond required {required}: {whole_snaps:?}"
                );
                eprintln!("phase snapshots retain={retain} current={current}: {whole_snaps:?}");
                let ((exact_snaps, exact), refused, used, _) = observe(required, current, retain);
                assert!(!refused && !exact.invalid && exact.live == 0 && used == required,
                    "current={current} retain={retain} refused={refused} used={used} required={required} exact={exact:?} phases={exact_snaps:?}");
                let ((short_snaps, short), refused, _, first) =
                    observe(required - 1, current, retain);
                assert!(refused && first.is_some() && !short.invalid && short.live == 0,
                    "current={current} retain={retain} refused={refused} first={first:?} short={short:?} phases={short_snaps:?}");
                observations.push((whole, required));
            }
            // Report both complete paths; no presumed savings or counter discount.
            eprintln!(
                "cache directory tradeoff retain={retain}: old={:?} current={:?}",
                observations[0], observations[1]
            );
        }
    }
}
