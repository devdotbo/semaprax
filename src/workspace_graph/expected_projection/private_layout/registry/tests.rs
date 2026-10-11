use super::LayoutRegistry;
use crate::workspace_graph::expected_projection::private_layout::{signature_owner, sorting};
use crate::workspace_graph::{
    expected_projection, import_validation, index_authored, index_modules,
};
const PROVIDER: &str = include_str!("../function_provider.spx");
const CALLER: &str = include_str!("../function_caller.spx");
fn sources() -> [crate::ast::Program; 2] {
    [
        crate::parse(CALLER, "caller.spx").unwrap(),
        crate::parse(PROVIDER, "provider.spx").unwrap(),
    ]
}
#[test]
fn invocation_reuses_exact_closure_across_import_forecasts_stubs_and_retry_modes() {
    let params = (0..64)
        .map(|n| format!("v{n}:borrow Report"))
        .collect::<Vec<_>>()
        .join(",");
    let provider = format!("{PROVIDER}\n@id(\"wide\") fn wide({params})->i64 {{0}}\n");
    let caller = CALLER.replacen(
        "module function_layout.app;",
        "module function_layout.app;\nuse function @id(\"wide\") from function_layout.provider as wide;",
        1,
    );
    assert_ne!(caller, CALLER);
    let parsed = [
        crate::parse(&caller, "caller.spx").unwrap(),
        crate::parse(&provider, "provider.spx").unwrap(),
    ];
    let authored = index_authored(&parsed).unwrap();
    let modules = index_modules(&parsed).unwrap();
    let registry = LayoutRegistry::new(&parsed, &authored);
    import_validation::validate_uses_with_registry(&parsed, &modules, &authored, Some(&registry))
        .unwrap();
    let backing = registry
        .get(&parsed[0], &authored, &parsed)
        .unwrap()
        .entries
        .as_ptr();
    let receipt = registry.receipt();
    assert!(receipt > 0);
    assert_eq!(registry.derivations.get(), 1);
    for retention in [
        expected_projection::cached_exact::Retention::Parsed,
        expected_projection::cached_exact::Retention::Semantic,
    ] {
        expected_projection::cached_exact::initial_with_registry(
            &parsed,
            &authored,
            false,
            retention,
            Some(&registry),
        )
        .unwrap();
        expected_projection::cached_exact::summed_with_registry(
            &parsed,
            &authored,
            retention,
            Some(&registry),
        )
        .unwrap();
    }
    for mode in 0..=4 {
        expected_projection::retention_prebound_mode_with_registry(
            &parsed,
            &authored,
            true,
            mode,
            Some(&registry),
        )
        .unwrap();
    }
    expected_projection::uncached_output_peak_prebound_with_registry(
        &parsed,
        &authored,
        Some(&registry),
    )
    .unwrap();
    let projected = expected_projection::synthetic_program_with_registry(
        &parsed[0],
        &authored,
        &parsed,
        Some(&registry),
    )
    .unwrap();
    let independent =
        expected_projection::synthetic_program(&parsed[0], &authored, &parsed).unwrap();
    assert_eq!(
        crate::format::canonical(&projected),
        crate::format::canonical(&independent)
    );
    for function in &parsed[1].functions {
        if !parsed[0]
            .module_uses
            .iter()
            .any(|u| u.persistent_id == function.stable_id)
        {
            continue;
        }
        let target = authored.get(function.stable_id.as_str()).unwrap();
        for ty in
            std::iter::once(&function.return_type).chain(function.params.iter().map(|p| &p.ty))
        {
            let closure = registry.get(&parsed[0], &authored, &parsed).unwrap();
            assert_eq!(
                closure
                    .signature_owner(&parsed[0], target, ty, &authored, &parsed)
                    .unwrap(),
                signature_owner(&parsed[0], target.module, ty, &authored, &parsed)
            );
        }
    }
    assert_eq!(
        registry
            .get(&parsed[0], &authored, &parsed)
            .unwrap()
            .entries
            .as_ptr(),
        backing
    );
    assert_eq!(
        registry.receipt(),
        receipt,
        "query/forecast does not pay for new closures"
    );
    assert_eq!(registry.derivations.get(), 1);
    let sources = || {
        parsed
            .iter()
            .map(|p| crate::workspace_graph::WorkspaceSource {
                path: p.path.clone(),
                source: crate::format::canonical(p),
            })
            .collect()
    };
    let (first, count) =
        super::observe_derivations(|| crate::workspace_graph::build_owned(sources()));
    let first = first.unwrap();
    assert_eq!(count, 1, "real invocation derives the selected caller once");
    let (second, count) =
        super::observe_derivations(|| crate::workspace_graph::build_owned(sources()));
    let second = second.unwrap();
    assert_eq!(count, 1, "new source generation gets its own directory");
    assert_eq!(first.edges, second.edges);
}
#[test]
fn preparation_has_exact_one_short_overlap_and_sticky_context_refusal() {
    let parsed = sources();
    let authored = index_authored(&parsed).unwrap();
    let registry = LayoutRegistry::new(&parsed, &authored);
    let (answer, overflowed, used) = crate::bounded_output::with_limit_usage(1 << 24, || {
        registry.get(&parsed[0], &authored, &parsed)
    });
    drop(answer.unwrap());
    assert!(!overflowed);
    assert_eq!(used, registry.receipt());
    for limit in [used, used - 1] {
        let candidate = LayoutRegistry::new(&parsed, &authored);
        let (answer, overflowed, consumed) = crate::bounded_output::with_limit_usage(limit, || {
            candidate.get(&parsed[0], &authored, &parsed)
        });
        assert_eq!(answer.is_ok(), limit == used);
        // Nested preparation refusal belongs to its child budget. The outer
        // ledger receives only actual successful debits (bounded_output's
        // nested_formatting_failures_stay_in_the_child_and_debit_the_parent).
        // Registry failure is independently sticky across later core attempts.
        assert!(!overflowed);
        assert_eq!(candidate.is_failed(), limit != used);
        if limit != used {
            assert!(answer
                .as_ref()
                .err()
                .unwrap()
                .iter()
                .any(|e| e.code == "SPX-G171"));
            assert!(consumed < used);
        }
        assert_eq!(candidate.receipt(), consumed);
        drop(answer);
        if limit == used {
            let (answer, overflowed, consumed) =
                crate::bounded_output::with_limit_usage(used, || {
                    crate::workspace_graph::reserve_builder_structure(candidate.receipt())?;
                    candidate.get(&parsed[0], &authored, &parsed)
                });
            drop(answer.unwrap());
            assert!(!overflowed);
            assert_eq!(
                consumed, used,
                "retained overlap is charged in every attempt"
            );
        } else {
            let (answer, _, consumed) = crate::bounded_output::with_limit_usage(0, || {
                candidate.get(&parsed[0], &authored, &parsed)
            });
            assert!(answer.is_err());
            assert_eq!(consumed, 0);
            assert_eq!(candidate.derivations.get(), 1);
        }
    }
    let stale = parsed.clone();
    let stale_authored = index_authored(&stale).unwrap();
    for attack in 0..3 {
        let candidate = LayoutRegistry::new(&parsed, &authored);
        candidate.get(&parsed[0], &authored, &parsed).unwrap();
        let before = candidate.receipt();
        let result = match attack {
            0 => candidate.get(&stale[0], &authored, &parsed),
            1 => candidate.get(&parsed[0], &stale_authored, &parsed),
            _ => candidate.get(&parsed[0], &authored, &stale),
        };
        assert!(result.is_err());
        assert!(candidate.get(&parsed[0], &authored, &parsed).is_err());
        assert_eq!(candidate.receipt(), before);
    }
    let explicit = [crate::parse(
        "module plain; @id(\"main\") fn main()->i64 {0}",
        "plain.spx",
    )
    .unwrap()];
    let index = index_authored(&explicit).unwrap();
    let empty = LayoutRegistry::new(&explicit, &index);
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(0, || empty.get(&explicit[0], &index, &explicit));
    assert!(result.unwrap().is_empty());
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    assert_eq!(empty.receipt(), 0);
}
#[test]
fn fallible_sort_matches_independent_order_and_stops_at_exact_work_boundary() {
    let original: Vec<_> = (0..64)
        .rev()
        .map(|n| format!("shared.long.provider.identity.{n:03}"))
        .collect();
    let mut expected = original.clone();
    expected.sort_unstable();
    let mut actual = original.clone();
    let mut work = 1 << 20;
    let (answer, overflowed, backing) = crate::bounded_output::with_limit_usage(0, || {
        sorting::sort(&mut actual, &mut work, |a, b, w| sorting::identity(a, b, w))
    });
    answer.unwrap();
    assert_eq!(actual, expected);
    assert!(!overflowed);
    assert_eq!(backing, 0);
    let used = (1 << 20) - work;
    for limit in [used, used - 1] {
        let mut actual = original.clone();
        let mut work = limit;
        let mut comparisons = 0;
        let answer = sorting::sort(&mut actual, &mut work, |a, b, w| {
            let order = sorting::identity(a, b, w)?;
            comparisons += 1;
            Ok(order)
        });
        assert_eq!(answer.is_ok(), limit == used);
        if limit == used {
            assert_eq!(actual, expected);
            assert_eq!(work, 0)
        } else {
            assert!(comparisons > 0);
            assert!(work < 2 * original[0].len() + 1)
        }
    }
}

#[test]
fn two_callers_share_one_derivation_query_ledger_and_sticky_refusal() {
    let second = CALLER
        .replace("function_layout.app", "function_layout.second")
        .replace("app.main", "second.main");
    let parsed = [
        crate::parse(CALLER, "caller.spx").unwrap(),
        crate::parse(&second, "second.spx").unwrap(),
        crate::parse(PROVIDER, "provider.spx").unwrap(),
    ];
    let authored = index_authored(&parsed).unwrap();
    let target = authored.get("make").unwrap();
    let ty = &target.function.unwrap().return_type;
    let run = |limit| {
        let registry = LayoutRegistry::new(&parsed, &authored);
        registry.remaining.set(limit);
        let result = (|| {
            let first = registry.get(&parsed[0], &authored, &parsed)?;
            assert_eq!(
                first.signature_owner(&parsed[0], target, ty, &authored, &parsed)?,
                Some(true)
            );
            drop(first);
            registry.get(&parsed[1], &authored, &parsed)?;
            let first = registry.get(&parsed[0], &authored, &parsed)?;
            let second = registry.get(&parsed[1], &authored, &parsed)?;
            assert!(std::rc::Rc::ptr_eq(
                &first.owners.as_ref().unwrap().ledger,
                &second.owners.as_ref().unwrap().ledger
            ));
            assert_eq!(
                second.signature_owner(&parsed[1], target, ty, &authored, &parsed)?,
                Some(true)
            );
            Ok::<_, Vec<crate::diagnostic::Diagnostic>>(())
        })();
        let used = limit - registry.work_remaining();
        if result.is_err() {
            assert!(registry.is_failed());
            let (answer, _, bytes) = crate::bounded_output::with_limit_usage(0, || {
                registry.get(&parsed[0], &authored, &parsed)
            });
            assert!(answer.is_err());
            assert_eq!(bytes, 0);
        }
        (result, used)
    };
    let (result, used) = run(1 << 20);
    result.unwrap();
    let (result, exact) = run(used);
    result.unwrap();
    assert_eq!(exact, used);
    assert!(run(used - 1).0.is_err());
}
#[test]
fn held_presentation_borrows_with_no_derivation_work_charge_or_allocation() {
    let parsed = sources();
    let authored = index_authored(&parsed).unwrap();
    let registry = LayoutRegistry::new(&parsed, &authored);
    // Presentation never derives: before the real lookup nothing is held.
    assert!(registry.held(&parsed[0]).is_none());
    assert_eq!(registry.derivations.get(), 0);
    let synthetic = expected_projection::synthetic_program_with_registry(
        &parsed[0],
        &authored,
        &parsed,
        Some(&registry),
    )
    .unwrap();
    assert_eq!(registry.derivations.get(), 1);
    let (work, receipt) = (registry.work_remaining(), registry.receipt());
    // A zero-byte builder budget refuses any charge; the observer counts every
    // requested backing through the view's and the borrow's drop.
    let ((bound, metrics), overflowed, charged) = crate::bounded_output::with_limit_usage(0, || {
        crate::test_allocation_observer::measure(|| {
            let held = registry.held(&parsed[0]).expect("held caller closure");
            let view = held.presentation(&parsed[0], &synthetic).unwrap();
            let bound = std::hint::black_box(view).is_some();
            drop(held);
            bound
        })
    });
    assert!(bound, "the caller's private copies are presented");
    assert!(!metrics.invalid);
    assert_eq!(
        (metrics.allocations, metrics.reallocations, metrics.allocated),
        (0, 0, 0),
        "binding the presentation allocates no slot backing or string copy"
    );
    assert_eq!((metrics.freed, metrics.live, metrics.highwater), (0, 0, 0));
    assert!(!overflowed);
    assert_eq!(charged, 0);
    assert_eq!(registry.work_remaining(), work, "no work debit");
    assert_eq!(registry.receipt(), receipt, "no preparation receipt");
    assert_eq!(registry.derivations.get(), 1, "no re-derivation");
    // A provider without imports is never held, so it has no presentation.
    assert!(registry.held(&parsed[1]).is_none());
    // One-short, renamed and foreign-identity copies are not the appended
    // declarations; the graph refuses them instead of presenting them.
    let mismatch = |synthetic: &crate::ast::Program| {
        let held = registry.held(&parsed[0]).unwrap();
        let errors = held.presentation(&parsed[0], synthetic).err().unwrap();
        assert_eq!(errors[0].code, "SPX-G173", "{errors:?}");
    };
    let mut short = synthetic.clone();
    short.types.pop();
    mismatch(&short);
    let mut renamed = synthetic.clone();
    renamed.types.last_mut().unwrap().name = "Row".to_owned();
    mismatch(&renamed);
    let mut foreign = synthetic.clone();
    foreign.types.last_mut().unwrap().stable_id = "sibling".to_owned();
    mismatch(&foreign);
}
