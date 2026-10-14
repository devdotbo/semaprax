//! Complete old source classifier with real std source and hostile full signatures.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
fn program() -> crate::ast::Program {
    crate::parse(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/std/collections/src/collections.spx"
        )),
        "std-collections-wrapper.spx",
    )
    .unwrap()
}
fn parity(program: &crate::ast::Program, function: &Function) {
    let old = original::source_wrapper(program, function);
    let ((current, actual), refused, paid, first) =
        with_limit_usage_refusal(0, || measure(|| source_wrapper(program, function)));
    assert_eq!(current, old);
    assert!(!refused && paid == 0 && first.is_none());
    assert!(!actual.invalid && actual.allocated == 0 && actual.live == 0);
    let (old_again, actual) = measure(|| original::source_wrapper(program, function));
    assert_eq!(old_again, old);
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
}
#[test]
fn all_exact_std_source_wrappers_keep_original_body_signature_and_module_gates_without_backing() {
    let program = program();
    let mut accepted = 0;
    for function in &program.functions {
        parity(&program, function);
        if source_wrapper(&program, function).is_some() {
            accepted += 1;
        }
    }
    assert_eq!(accepted, 3);
    for function in program
        .functions
        .iter()
        .filter(|f| original::source_wrapper(&program, f).is_some())
    {
        for case in 0..14 {
            let mut changed = function.clone();
            match case {
                0 => changed.explicit_id = false,
                1 => changed.name.push_str(".λ"),
                2 => changed.stable_id.push('\0'),
                3 => changed.type_parameters[0].name = "U".to_owned(),
                4 => changed
                    .type_parameters
                    .push(changed.type_parameters[0].clone()),
                5 => changed.effects.push("hostile.effect".to_owned()),
                6 => changed.requires.push(changed.body.clone()),
                7 => changed.ensures.push(changed.body.clone()),
                8 => changed.params[0].name.push('\0'),
                9 => {
                    changed.params[0].mode =
                        if changed.params[0].mode == crate::ast::ParamMode::Value {
                            crate::ast::ParamMode::Own
                        } else {
                            crate::ast::ParamMode::Value
                        }
                }
                10 => {
                    changed.params[0].ty = Type::Named {
                        name: "T".to_owned(),
                        arguments: vec![Type::I64],
                    }
                }
                11 => {
                    changed.return_type = Type::Named {
                        name: "T".to_owned(),
                        arguments: vec![Type::I64],
                    }
                }
                12 => {
                    let copy = changed.params[0].clone();
                    changed.params.push(copy);
                }
                13 => {
                    let ExprKind::Block { tail, .. } = &mut changed.body.kind else {
                        panic!()
                    };
                    let ExprKind::Call { type_arguments, .. } = &mut tail.kind else {
                        panic!()
                    };
                    type_arguments.push(Type::I64);
                }
                _ => unreachable!(),
            }
            parity(&program, &changed);
            assert!(source_wrapper(&program, &changed).is_none());
        }
        let mut foreign = program.clone();
        foreign.module = "foreign.collections".to_owned();
        parity(&foreign, function);
        assert!(source_wrapper(&foreign, function).is_none());
        with_authenticated_linked_source(|| {
            parity(&foreign, function);
            assert_eq!(
                source_wrapper(&foreign, function),
                original::source_wrapper(&foreign, function)
            );
        });
    }
}
#[test]
fn pure_wrapper_observation_is_not_funding_authority_and_does_not_clear_sticky_refusal() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| original::source_wrapper(&program, f).is_some())
        .unwrap();
    let old = original::source_wrapper(&program, function);
    let (((), actual), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            assert!(!crate::bounded_output::reserve_active_required(1));
            assert_eq!(source_wrapper(&program, function), old);
        })
    });
    assert!(refused && paid == 0 && first.unwrap().requested == 1);
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert_eq!(actual.allocated, 0);
    // Pure truth does not clear or grant a receipt for the selected refusal.
}
