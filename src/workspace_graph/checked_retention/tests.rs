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
        select_owned_oracle(
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

#[test]
fn noncontiguous_three_and_five_backings_have_no_growth_and_input_headers_pay_actual_delta() {
    let source = (0..9).fold(String::from("module local;"), |mut source, index| {
        let name = if index == 8 {
            "main".to_owned()
        } else {
            format!("f{index}")
        };
        source.push_str(&format!(
            " @id(\"local.f{index}\") fn {name}() -> i64 {{ {index} }}"
        ));
        source
    });
    let (_, _, owner) = crate::project::incremental::checked_hir::tests::cold_source(&source);
    for count in [3usize, 5] {
        let keep = |function: &hir::ResolvedFunction| {
            let index = function
                .id
                .as_str()
                .strip_prefix("local.f")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            index % 2 == 0 && index / 2 < count
        };
        // Independent output construction has an authored exact count and no
        // iterator growth strategy. Both observations include nested HIR clone
        // backing and destruction; this is not a fixed-header-only heap claim.
        let (_, old) = crate::test_allocation_observer::measure(|| {
            let mut output = Vec::with_capacity(count);
            for function in owner.functions.iter().filter(|function| keep(function)) {
                output.push(function.clone());
            }
            assert_eq!(output.capacity(), count);
            drop(output);
        });
        let debit = count * std::mem::size_of::<hir::ResolvedFunction>();
        let (_, observed) = crate::test_allocation_observer::measure(|| {
            let (view, overflow, charged, _) =
                crate::bounded_output::with_limit_usage_refusal(debit, || {
                    select_owned_oracle(owner.clone(), |program| &program.functions, keep)
                });
            assert!(!overflow);
            assert_eq!(charged, debit);
            let Retained::Owned(output) = view.unwrap() else {
                panic!("noncontiguous selection")
            };
            assert_eq!((output.len(), output.capacity()), (count, count));
            drop(output);
        });
        // with_limit owns one real Budget Rc allocation; observe the same
        // infrastructure separately rather than guessing its private layout.
        let (_, ledger) = crate::test_allocation_observer::measure(|| {
            crate::bounded_output::with_limit_usage_refusal(0, || ());
        });
        assert!(!observed.invalid && !old.invalid && !ledger.invalid);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.allocated, old.allocated + ledger.allocated);
        assert_eq!(observed.live, 0);
    }
    let old = std::mem::size_of::<(String, hir::ResolvedProgram)>();
    let new = std::mem::size_of::<(String, ResolvedModuleInput)>();
    let expected = 3
        * (std::mem::size_of::<String>()
            + GRAPH_ACCOUNTED_RESOLVED_PROGRAM_BYTES
            + new.saturating_sub(old));
    let (result, overflow, charged, _) =
        crate::bounded_output::with_limit_usage_refusal(expected, || reserve_input_carriers(3));
    assert!(result.is_ok());
    assert!(!overflow);
    assert_eq!(charged, expected);
    let (result, overflow, _, _) =
        crate::bounded_output::with_limit_usage_refusal(expected - 1, || reserve_input_carriers(3));
    assert!(result.is_err() && overflow);
}

#[path = "tests/publication.rs"]
mod publication;

// Frozen old noncontiguous owning path; the production ordinal witness is
// independent of this carrier/cloning oracle.
fn select_owned_oracle<T: Clone>(
    owner: CheckedModuleLease,
    read: fn(&hir::ResolvedProgram) -> &[T],
    keep: impl Fn(&T) -> bool,
) -> Result<Retained<T>, Vec<Diagnostic>> {
    let items = read(&owner);
    let mut start = None;
    let mut end = 0;
    let mut gap = false;
    let mut contiguous = true;
    for (index, item) in items.iter().enumerate() {
        if keep(item) {
            if gap {
                contiguous = false;
            }
            start.get_or_insert(index);
            end = index + 1;
        } else if start.is_some() {
            gap = true;
        }
    }
    if contiguous {
        // Range and owner handles are inline, covered by the actual module
        // carrier. Arc clones allocate no extra control block.
        Ok(Retained::Checked {
            owner,
            selection: Selection::Range(start.unwrap_or(0)..end),
            read,
        })
    } else {
        // Truthful compatibility fallback: the unchanged cached two-copy
        // prebound still funds the copied HIR. The actual output carrier is
        // reserved before allocation, as with ordinary retained filtering.
        let count = items.iter().filter(|item| keep(item)).count();
        let reserved = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
        reserve_builder_structure(reserved)?;
        let mut output = Vec::with_capacity(count);
        let actual = output
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
        if actual > reserved {
            reserve_builder_structure(actual - reserved)?;
        }
        for item in items.iter().filter(|item| keep(item)) {
            output.push(item.clone());
        }
        Ok(Retained::Owned(output))
    }
}

#[path = "tests/ordinals.rs"]
mod ordinals;
