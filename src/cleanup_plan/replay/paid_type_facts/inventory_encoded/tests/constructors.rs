use super::*;

fn observe<V: PartialEq>(
    bytes: usize,
    work: usize,
    function: &ResolvedFunction,
    expected: Option<&Option<V>>,
    query: impl Fn(&mut Payer<'_>) -> Result<Option<V>, Diagnostic>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "full constructor receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        query(&mut super::super::super::tests::payer(
                            function,
                            &mut budget,
                        ))
                    });
                let matched =
                    expected.is_none_or(|old| result.as_ref().is_ok_and(|value| value == old));
                let failed = result.is_err();
                let used = work.saturating_sub(budget.remaining);
                if failed {
                    let before = current_metrics();
                    let retry = query(&mut super::super::super::tests::payer(
                        function,
                        &mut budget,
                    ));
                    assert!(retry.is_err());
                    assert_eq!(current_metrics().allocated, before.allocated);
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed, used)
            })
        });
    (outcome, observed, refused, paid)
}
fn control<V: PartialEq>(
    function: &ResolvedFunction,
    expected: &Option<V>,
    query: impl Fn(&mut Payer<'_>) -> Result<Option<V>, Diagnostic>,
) {
    let (ample, observed, refused, paid) = observe(
        usize::MAX,
        MAX_REPLAY_WORK_UNITS,
        function,
        Some(expected),
        &query,
    );
    assert!(ample.0 && !ample.1 && !refused && !observed.invalid);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    let (exact, observed, refused, used) = observe(paid, ample.2, function, Some(expected), &query);
    assert!(exact.0 && !exact.1 && !refused && !observed.invalid);
    assert_eq!(used, paid);
    assert_eq!(observed.live, 0);
    for (bytes, work) in [
        (0, MAX_REPLAY_WORK_UNITS),
        (paid - 1, MAX_REPLAY_WORK_UNITS),
        (paid, 0),
        (paid, ample.2 - 1),
    ] {
        let (short, observed, _, paid) = observe(bytes, work, function, None, &query);
        assert!(short.1 && !observed.invalid && observed.allocated <= paid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn complete_callable_nominal_identity_zero_one_seventeen_arguments_keep_paid_work_and_drop() {
    let program = fixture();
    let function = &program.functions[0];
    for width in [0, 1, 17] {
        let ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("type.é\0🦀"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::ArrayU8(u32::MAX), ResolvedType::I64],
                    result: Box::new(ResolvedType::TypeParameter {
                        owner: function.id.clone(),
                        index: 17,
                    }),
                };
                width
            ],
        };
        let (expected, refused) = crate::bounded_output::with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "complete old key receiver")
                .unwrap();
            program
                .declarations
                .classifier_key_for_test(
                    &ty,
                    super::super::super::tests::payer(function, &mut budget),
                )
                .unwrap()
        });
        assert!(!refused);
        control(function, &expected, |payer| {
            crate::hir::inventory_identity(&ty, payer)
        });
    }
}
#[test]
fn complete_inline_semantic_substitution_refusal_and_nested_argument_clone_keep_original() {
    let program = fixture();
    let function = &program.functions[0];
    let parameter = ResolvedType::TypeParameter {
        owner: function.id.clone(),
        index: 0,
    };
    for width in [0, 1, 17] {
        let arguments = vec![ResolvedType::Nominal {
            declaration: DeclarationId::new("held.argument.é\0suffix"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::Bytes; width],
                    result: Box::new(ResolvedType::I64),
                };
                width
            ],
        }];
        let template = ResolvedType::Function {
            parameters: vec![parameter.clone(); width],
            result: Box::new(parameter.clone()),
        };
        let (expected, refused) = crate::bounded_output::with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "complete old substitution receiver")
                .unwrap();
            program
                .declarations
                .classifier_substitute_for_test(
                    &template,
                    &function.id,
                    &arguments,
                    super::super::super::tests::payer(function, &mut budget),
                )
                .unwrap()
        });
        assert!(!refused);
        control(function, &expected, |payer| {
            crate::hir::inventory_substitute(&template, &function.id, &arguments, payer)
        });
    }
    for template in [
        parameter,
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("foreign.é\0owner"),
            index: 0,
        },
    ] {
        let expected = crate::hir::substitute_type(&template, &function.id, &[]).ok();
        assert!(expected.is_none());
        control(function, &expected, |payer| {
            crate::hir::inventory_substitute(&template, &function.id, &[], payer)
        });
    }
}
#[test]
fn complete_prior_key_substitution_and_storage_receivers_remain_independent_byte_exact() {
    assert_eq!(include_str!("../../../../../hir.rs")
        .replacen("pub(crate) use declaration_index::{inventory_identity, inventory_substitute, InventoryPayer};\n", "", 1)
        .replacen("#[cfg(test)]\npub(crate) use byte_capacity::byte_data_capacity_inputs;\n", "", 1),
        include_str!("../../../../../../docs/evidence/schema-summary/hir.rs.before"));
    assert_eq!(include_str!("../../../../../hir/declaration_index/flags/paid.rs")
        .strip_suffix("\nmod inventory_constructor;\npub(crate) use inventory_constructor::{\n    identity as inventory_identity, substitute as inventory_substitute, InventoryPayer,\n};\n").unwrap(),
        include_str!("../../../../../../docs/evidence/schema-summary/flags-paid.rs.before"));
    assert_eq!(
        include_str!("../../../../../hir/declaration_index/flags/paid/key.rs"),
        include_str!("../../../../../../docs/evidence/schema-summary/flags-paid-key.rs.before")
    );
    assert_eq!(
        include_str!("../../../../../hir/declaration_index/flags/paid/substitute.rs"),
        include_str!(
            "../../../../../../docs/evidence/schema-summary/flags-paid-substitute.rs.before"
        )
    );
    assert_eq!(
        include_str!("../../../../../hir/declaration_index/flags/paid/storage.rs"),
        include_str!("../../../../../../docs/evidence/schema-summary/flags-paid-storage.rs.before")
    );
}
