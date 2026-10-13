use super::*;
use crate::test_allocation_observer::{measure, Metrics};
const SOURCE: &str = include_str!("../../../../../tests/fixtures/owned-nested-outcome.spx");
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(SOURCE, "nested-paid.spx").unwrap()).unwrap()
}
fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}
fn run(
    limit: usize,
    work: usize,
    program: &ResolvedProgram,
    ty: &ResolvedType,
    expected: Option<bool>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "nested paid receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        admitted(
                            &program.declarations,
                            ty,
                            &mut super::super::tests::payer(function, &mut budget),
                        )
                    });
                let matched =
                    expected.is_none_or(|old| result.as_ref().is_ok_and(|actual| *actual == old));
                let failed = result.is_err();
                let used_work = work.saturating_sub(budget.remaining);
                if crate::bounded_output::active_refused() {
                    let before = crate::test_allocation_observer::current_metrics();
                    let retry = admitted(
                        &program.declarations,
                        ty,
                        &mut super::super::tests::payer(function, &mut budget),
                    );
                    assert!(retry.is_err());
                    assert_eq!(
                        before.allocated,
                        crate::test_allocation_observer::current_metrics().allocated
                    );
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed, used_work)
            })
        });
    (outcome, observed, refused, paid)
}
#[test]
fn full_nested_receiver_arguments_case_order_exact_work_backing_and_receiving_drop() {
    let program = fixture();
    let mut roots = vec![
        nominal("n.Outcome", vec![]),
        nominal("missing", vec![]),
        ResolvedType::Bytes,
    ];
    for count in [1, 17] {
        roots.push(nominal(
            "n.Outcome",
            vec![nominal("n.Payload", vec![ResolvedType::Bytes]); count],
        ));
    }
    for root in roots {
        let expected =
            crate::hir::collection_outcome::nested::admitted(&program.declarations, &root);
        let (outcome, observed, refused, paid) = run(
            usize::MAX,
            MAX_REPLAY_WORK_UNITS,
            &program,
            &root,
            Some(expected),
        );
        assert!(outcome.0 && !outcome.1 && !refused && !observed.invalid);
        assert!(observed.allocated <= paid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        let (exact, observed, refused, used) =
            run(paid, outcome.2, &program, &root, Some(expected));
        assert!(exact.0 && !exact.1 && !refused && !observed.invalid);
        assert_eq!(used, paid);
        assert_eq!(observed.live, 0);
        for (bytes, work) in [
            (0, MAX_REPLAY_WORK_UNITS),
            (paid - 1, MAX_REPLAY_WORK_UNITS),
            (paid, 0),
            (paid, outcome.2 - 1),
        ] {
            let (short, observed, _, used) = run(bytes, work, &program, &root, None);
            assert!(short.1 && !observed.invalid);
            assert!(observed.allocated <= used);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn borrowed_full_payload_field_identity_and_type_match_complete_original() {
    let program = fixture();
    let function = &program.functions[0];
    let container = nominal("n.Outcome", vec![]);
    let payload = nominal("n.Payload", vec![]);
    for (case, field) in [
        ("n.Outcome.ready", "n.Outcome.value"),
        ("n.Outcome.error", "n.Outcome.value"),
        ("foreign", "n.Outcome.value"),
        ("n.Outcome.ready", "n.Payload.items"),
    ] {
        let case = DeclarationId::new(case);
        let field = DeclarationId::new(field);
        for ty in [
            &payload,
            &nominal("n.Payload", vec![ResolvedType::Bytes]),
            &ResolvedType::Bytes,
        ] {
            let expected = crate::hir::collection_outcome::nested::record_field(
                &program.declarations,
                &container,
                &case,
                &field,
                ty,
            );
            crate::bounded_output::with_limit(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                budget.charge(function, 0, "nested field receiver").unwrap();
                assert_eq!(
                    record_field(
                        &program.declarations,
                        &container,
                        &case,
                        &field,
                        ty,
                        &mut super::super::tests::payer(function, &mut budget)
                    )
                    .unwrap(),
                    expected
                );
            });
        }
    }
}
#[test]
fn held_metadata_root_suffix_zero_one_short_and_sticky_work_keep_original_queries() {
    let program = fixture();
    let index = &program.declarations;
    let function = &program.functions[0];
    for text in ["n.Outcome", "n.Config", "missing", "é\0🦀suffix"] {
        let key = DeclarationId::new(text);
        let expected = (index.declaration(&key), index.has_record_invariant(&key));
        let (work, refused) = crate::bounded_output::with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget.charge(function, 0, "metadata receiver").unwrap();
            let before = budget.remaining;
            let mut payer = super::super::tests::payer(function, &mut budget);
            let value = metadata::declared(index, &key, &mut payer).unwrap();
            let present = metadata::has_invariant(index, &key, &mut payer).unwrap();
            assert_eq!((value, present), expected);
            before - payer.budget.remaining
        });
        assert!(!refused);
        for limit in [work, work - 1, 0] {
            // Each finite control starts a distinct held budget; never revive
            // a refused payer or transfer an unused work allowance to it.
            let ((failed, observed), _, paid) =
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        budget
                            .charge(function, 0, "metadata finite receiver")
                            .unwrap();
                        budget.remaining = limit;
                        let mut payer = super::super::tests::payer(function, &mut budget);
                        let result =
                            metadata::declared(index, &key, &mut payer).and_then(|value| {
                                metadata::has_invariant(index, &key, &mut payer)
                                    .map(|present| (value, present))
                            });
                        let failed = result.is_err();
                        if limit == work {
                            assert_eq!(result.as_ref().unwrap(), &expected);
                        } else {
                            assert!(failed);
                            let before = crate::test_allocation_observer::current_metrics();
                            let retry = metadata::declared(index, &key, &mut payer);
                            assert!(retry.is_err());
                            assert_eq!(
                                before.allocated,
                                crate::test_allocation_observer::current_metrics().allocated
                            );
                            drop(retry);
                        }
                        drop(result);
                        drop(budget);
                        failed
                    })
                });
            assert_eq!(failed, limit < work);
            assert!(!observed.invalid && observed.allocated <= paid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn complete_original_nested_and_index_sources_stay_independent_and_byte_exact() {
    assert_eq!(
        include_str!("../../../../hir/collection_outcome/nested.rs"),
        include_str!("../../../../../docs/evidence/schema-summary/nested.rs.before")
    );
    assert_eq!(
        include_str!("../../../../hir/declaration_index.rs").replacen(
            "mod inventory_lookup;\n",
            "",
            1
        ),
        include_str!("../../../../../docs/evidence/schema-summary/declaration_index.rs.before")
    );
}

#[test]
fn full_forged_cycle_missing_arity_and_case_metadata_keep_original_refusal_order() {
    let original = fixture();
    let root = nominal("n.Outcome", vec![]);
    for index in original.declarations.inventory_nested_forgeries_for_test() {
        let expected = crate::hir::collection_outcome::nested::admitted(&index, &root);
        assert!(!expected);
        crate::bounded_output::with_limit(usize::MAX, || {
            let function = &original.functions[0];
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "forged nested receiver setup")
                .unwrap();
            assert_eq!(
                admitted(
                    &index,
                    &root,
                    &mut super::super::tests::payer(function, &mut budget)
                )
                .unwrap(),
                expected
            );
        });
    }
}
