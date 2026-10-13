use super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(
            "module raw.summary; @id(\"main\") fn main()->i64 {7}",
            "raw-summary.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn substitute(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<ResolvedType, Diagnostic> {
    inventory_substitute::substitute(
        template,
        owner,
        arguments,
        &mut tests::payer(function, budget),
    )
}

fn run(
    limit: usize,
    function: &ResolvedFunction,
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
    expected: Option<&Result<ResolvedType, Diagnostic>>,
) -> ((bool, bool), Metrics, bool, usize) {
    let ((value, metrics), refused, paid) = crate::bounded_output::with_limit_usage(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = budget
                .charge(function, 0, "inventory substitution receiver setup")
                .and_then(|()| substitute(template, owner, arguments, function, &mut budget));
            let matched = expected.is_none_or(|old| &result == old);
            let failed = result.is_err();
            if crate::bounded_output::active_refused() {
                let before = current_metrics();
                let sticky = budget.reserve_bytes(function, 0, "raw substitution sticky receiver");
                assert!(sticky.is_err());
                assert_eq!(current_metrics().allocated, before.allocated);
                drop(sticky);
            }
            drop(result);
            drop(budget);
            (matched, failed)
        })
    });
    (value, metrics, refused, paid)
}
#[test]
fn full_zero_one_seventeen_nested_arguments_keep_ordinary_substitution_and_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let parameter = ResolvedType::TypeParameter {
        owner: function.id.clone(),
        index: 0,
    };
    for width in [0, 1, 17] {
        let argument = ResolvedType::Nominal {
            declaration: DeclarationId::new("raw.argument.λ"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::TypeParameter {
                        owner: DeclarationId::new("foreign.argument.owner"),
                        index: 17
                    }],
                    result: Box::new(ResolvedType::Bytes),
                };
                width
            ],
        };
        for template in [
            parameter.clone(),
            ResolvedType::Function {
                parameters: vec![parameter.clone(); width],
                result: Box::new(parameter.clone()),
            },
        ] {
            let expected = crate::hir::substitute_type(
                &template,
                &function.id,
                std::slice::from_ref(&argument),
            );
            let (outcome, measured, refused, paid) = run(
                usize::MAX,
                function,
                &template,
                &function.id,
                std::slice::from_ref(&argument),
                Some(&expected),
            );
            assert_eq!(outcome, (true, false));
            assert!(!refused && !measured.invalid);
            assert_eq!(measured.live, 0);
            assert_eq!(measured.allocated, measured.freed);
            assert!(
                measured.allocated <= paid,
                "actual constructors through receiving Drop exceed caller debits"
            );
            assert!(paid > 0);
            let (exact, measured, refused, used) = run(
                paid,
                function,
                &template,
                &function.id,
                std::slice::from_ref(&argument),
                Some(&expected),
            );
            assert_eq!(exact, (true, false));
            assert!(!refused && !measured.invalid);
            assert_eq!(used, paid);
            assert_eq!(measured.live, 0);
            let (short, measured, refused, used) = run(
                paid - 1,
                function,
                &template,
                &function.id,
                std::slice::from_ref(&argument),
                None,
            );
            assert!(short.1 && refused && !measured.invalid);
            assert!(used < paid);
            assert_eq!(measured.live, 0);
        }
    }
}
#[test]
fn foreign_then_missing_parameter_keeps_complete_raw_first_error_and_zero_sticky() {
    let program = fixture();
    let function = &program.functions[0];
    for index in [0, 17] {
        let template = ResolvedType::Function {
            parameters: vec![
                ResolvedType::TypeParameter {
                    owner: DeclarationId::new("foreign.λ"),
                    index,
                },
                ResolvedType::TypeParameter {
                    owner: function.id.clone(),
                    index: 99,
                },
            ],
            result: Box::new(ResolvedType::Unit),
        };
        let expected = crate::hir::substitute_type(&template, &function.id, &[]);
        assert!(expected.is_err());
        let (outcome, measured, refused, _) = run(
            usize::MAX,
            function,
            &template,
            &function.id,
            &[],
            Some(&expected),
        );
        assert_eq!(outcome, (true, true));
        assert!(!refused && !measured.invalid);
        assert_eq!(measured.live, 0);
        let (zero, measured, refused, paid) = run(0, function, &template, &function.id, &[], None);
        assert!(zero.1 && refused && !measured.invalid);
        assert_eq!(paid, 0);
        assert_eq!(measured.allocated, 0);
        assert_eq!(measured.live, 0);
    }
}
#[test]
fn substitution_uses_existing_cumulative_work_without_allowance_or_reset() {
    let program = fixture();
    let function = &program.functions[0];
    let template = ResolvedType::Function {
        parameters: vec![ResolvedType::Bytes; 17],
        result: Box::new(ResolvedType::Unit),
    };
    let ((spent, remaining), refused, _) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "inventory substitution work setup")
                .unwrap();
            let initial = budget.remaining;
            drop(substitute(&template, &function.id, &[], function, &mut budget).unwrap());
            let spent = initial - budget.remaining;
            drop(substitute(&template, &function.id, &[], function, &mut budget).unwrap());
            assert_eq!(initial - budget.remaining, 2 * spent);
            (spent, budget.remaining)
        });
    assert!(!refused);
    assert!(spent > 17 && remaining > 0);
    let ((sticky, measured), _outer_refused, _) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget
                    .charge(function, 0, "inventory substitution short work setup")
                    .unwrap();
                budget.remaining = spent - 1;
                let result = substitute(&template, &function.id, &[], function, &mut budget);
                assert!(result.is_err());
                let before = current_metrics();
                let retry = substitute(&template, &function.id, &[], function, &mut budget);
                assert!(retry.is_err());
                assert_eq!(current_metrics().allocated, before.allocated);
                let sticky = budget.refusal_spent;
                drop(retry);
                drop(result);
                drop(budget);
                sticky
            })
        });
    assert!(sticky);
    assert!(!measured.invalid);
    assert_eq!(measured.live, 0);
}

#[test]
fn held_substitution_payer_refuses_cross_invocation_before_construction() {
    let program = fixture();
    let function = &program.functions[0];
    let templates = [
        ResolvedType::TypeParameter {
            owner: function.id.clone(),
            index: 0,
        },
        ResolvedType::Function {
            parameters: vec![ResolvedType::Bytes],
            result: Box::new(ResolvedType::Unit),
        },
    ];
    let (((), measured), _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "held inventory payer setup")
                .unwrap();
            let mut held = tests::payer(function, &mut budget);
            for template in &templates {
                let ((), _, _) = crate::bounded_output::with_limit_usage(0, || {
                    let before = current_metrics();
                    let result = inventory_substitute::substitute(
                        template,
                        &function.id,
                        &[ResolvedType::Bytes],
                        &mut held,
                    );
                    assert!(result.is_err());
                    drop(result);
                    assert_eq!(current_metrics().allocated, before.allocated);
                    assert_eq!(current_metrics().allocations, before.allocations);
                });
            }
            drop(held);
            drop(budget);
        })
    });
    assert!(!measured.invalid);
    assert_eq!(measured.live, 0);
    assert_eq!(measured.allocated, measured.freed);
}

#[test]
fn raw_missing_parameter_and_each_actual_reservation_boundary_keep_sticky_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let argument = ResolvedType::Function {
        parameters: vec![ResolvedType::Bytes; 17],
        result: Box::new(ResolvedType::Unit),
    };
    let templates = [
        ResolvedType::TypeParameter {
            owner: function.id.clone(),
            index: u32::MAX,
        },
        ResolvedType::TypeParameter {
            owner: function.id.clone(),
            index: 0,
        },
        ResolvedType::Function {
            parameters: vec![
                ResolvedType::TypeParameter {
                    owner: function.id.clone(),
                    index: 0
                };
                17
            ],
            result: Box::new(ResolvedType::Bytes),
        },
    ];
    for template in &templates {
        let args = std::slice::from_ref(&argument);
        let expected = crate::hir::substitute_type(template, &function.id, args);
        let mut limit = 0usize;
        loop {
            let (((matched, failed), measured), refused, paid, first) =
                crate::bounded_output::with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let result = budget
                            .charge(function, 0, "inventory boundary setup")
                            .and_then(|()| {
                                substitute(template, &function.id, args, function, &mut budget)
                            });
                        let matched = result == expected;
                        let failed = result.is_err();
                        if crate::bounded_output::active_refused() {
                            let before = current_metrics();
                            let retry =
                                substitute(template, &function.id, args, function, &mut budget);
                            assert!(retry.is_err());
                            assert_eq!(current_metrics().allocated, before.allocated);
                            drop(retry);
                        }
                        drop(result);
                        drop(budget);
                        (matched, failed)
                    })
                });
            assert!(!measured.invalid);
            assert_eq!(measured.live, 0);
            assert_eq!(measured.allocated, measured.freed);
            assert!(
                measured.allocated <= paid,
                "unpaid acquisition before a real reservation boundary"
            );
            if !refused {
                assert!(matched);
                break;
            }
            assert!(failed);
            let refusal = first.expect("actual first sticky reservation");
            // At this next finite limit, the actual first refused request fits.
            // Follow genuine producer requests, never a guessed payload allowance.
            let next = paid
                .checked_add(refusal.requested)
                .and_then(|n| n.checked_add(refusal.floor))
                .unwrap();
            assert!(next > limit);
            limit = next;
        }
    }
}

#[test]
fn raw_utf8_owner_comparison_and_diagnostic_writes_use_the_same_finite_work() {
    let program = fixture();
    let function = &program.functions[0];
    let owner = DeclarationId::new("target.λ");
    let template = ResolvedType::TypeParameter {
        owner: DeclarationId::new("forged.λ"),
        index: 0,
    };
    let expected = crate::hir::substitute_type(&template, &owner, &[]);
    let message_bytes = expected.as_ref().unwrap_err().message.len();
    let ((spent, measured), refused, paid) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget
                    .charge(function, 0, "raw text work receiver setup")
                    .unwrap();
                let initial = budget.remaining;
                let first = substitute(&template, &owner, &[], function, &mut budget);
                assert_eq!(first, expected);
                let spent = initial - budget.remaining;
                assert!(spent >= message_bytes + owner.as_str().len() + 2);
                let second = substitute(&template, &owner, &[], function, &mut budget);
                assert_eq!(second, expected);
                assert_eq!(initial - budget.remaining, 2 * spent);
                drop(second);
                drop(first);
                drop(budget);
                spent
            })
        });
    assert!(!refused && !measured.invalid);
    assert!(measured.allocated <= paid);
    assert_eq!(measured.allocated, measured.freed);
    assert_eq!(measured.live, 0);
    for work in [
        spent,
        spent - 1,
        0,
        1,
        3 + owner.as_str().len() - 1,
        3 + owner.as_str().len(),
    ] {
        let ((matched, measured), refused, paid) =
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    budget
                        .charge(function, 0, "raw text finite work setup")
                        .unwrap();
                    assert!(budget.pays_allocations() && budget.refusal_text.is_some());
                    let construction_start = current_metrics();
                    budget.remaining = work;
                    let result = substitute(&template, &owner, &[], function, &mut budget);
                    let matched = result == expected;
                    if work <= 3 + owner.as_str().len() {
                        assert_eq!(current_metrics().allocated, construction_start.allocated);
                    }
                    if work < spent {
                        assert!(!matched && result.is_err() && budget.refusal_spent);
                        let before = current_metrics();
                        let retry = substitute(&template, &owner, &[], function, &mut budget);
                        assert!(retry.is_err());
                        assert_eq!(current_metrics().allocated, before.allocated);
                        drop(retry);
                    }
                    drop(result);
                    drop(budget);
                    matched
                })
            });
        assert_eq!(matched, work == spent);
        assert!(!refused && !measured.invalid);
        assert!(measured.allocated <= paid);
        assert_eq!(measured.allocated, measured.freed);
        assert_eq!(measured.live, 0);
    }
}

#[test]
fn detached_paid_constructor_refuses_without_affecting_complete_ordinary_compatibility() {
    let program = fixture();
    let function = &program.functions[0];
    let argument = ResolvedType::Function {
        parameters: vec![ResolvedType::Bytes; 17],
        result: Box::new(ResolvedType::Unit),
    };
    let template = ResolvedType::TypeParameter {
        owner: function.id.clone(),
        index: 0,
    };
    assert!(
        crate::hir::substitute_type(&template, &function.id, std::slice::from_ref(&argument))
            .is_ok()
    );
    let (refused, observed) = measure(|| {
        let mut budget = ReplayBudget::unobserved();
        let result = substitute(
            &template,
            &function.id,
            std::slice::from_ref(&argument),
            function,
            &mut budget,
        );
        let error = result.unwrap_err();
        assert_eq!(
            error,
            Diagnostic::io_static(
                "SPX-H006",
                "ordinary inventory constructors require a live backing payer"
            )
        );
        drop(error);
        drop(budget);
        true
    });
    assert!(refused && !observed.invalid);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
}
