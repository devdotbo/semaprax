//! Separate complete original receiver and exact static prelude owner receipt.
//! This does not rewrite measurements or subtract process-lifetime backing.
use super::{linked, old, LayoutRegistry};
use crate::ast::{FieldDeclaration, Program, Type, TypeDeclarationKind};
use crate::test_allocation_observer::{current_metrics, live_bytes_matching, measure};
use crate::workspace_graph::index_authored;

fn string_owns(value: &String, pointer: usize) -> bool {
    value.capacity() != 0 && value.as_ptr() as usize == pointer
}
fn vec_owns<T>(values: &Vec<T>, pointer: usize) -> bool {
    values.capacity() != 0 && values.as_ptr() as usize == pointer
}
fn type_owns(ty: &Type, pointer: usize) -> bool {
    match ty {
        Type::Named { name, arguments } => {
            string_owns(name, pointer)
                || vec_owns(arguments, pointer)
                || arguments.iter().any(|ty| type_owns(ty, pointer))
        }
        Type::Function { parameters, result } => {
            vec_owns(parameters, pointer)
                || result.as_ref() as *const Type as usize == pointer
                || parameters.iter().any(|ty| type_owns(ty, pointer))
                || type_owns(result, pointer)
        }
        _ => false,
    }
}
fn fields_own(fields: &Vec<FieldDeclaration>, pointer: usize) -> bool {
    vec_owns(fields, pointer)
        || fields.iter().any(|field| {
            string_owns(&field.stable_id, pointer)
                || string_owns(&field.name, pointer)
                || type_owns(&field.ty, pointer)
        })
}
fn prelude_owns(pointer: usize) -> bool {
    // Resolution has already requested this exact OnceLock owner. Do not call
    // it before the complete original receiver or initialize a proxy schema.
    let declarations = crate::prelude::declarations();
    declarations.as_ptr() as usize == pointer
        || declarations.iter().any(|declaration| {
            string_owns(&declaration.stable_id, pointer)
                || string_owns(&declaration.name, pointer)
                || vec_owns(&declaration.type_parameters, pointer)
                || declaration
                    .type_parameters
                    .iter()
                    .any(|parameter| string_owns(&parameter.name, pointer))
                || match &declaration.kind {
                    TypeDeclarationKind::Record { fields } => fields_own(fields, pointer),
                    TypeDeclarationKind::Variant { cases } => {
                        vec_owns(cases, pointer)
                            || cases.iter().any(|case| {
                                string_owns(&case.stable_id, pointer)
                                    || string_owns(&case.name, pointer)
                                    || fields_own(&case.fields, pointer)
                            })
                    }
                    TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Class { .. } => {
                        false
                    }
                }
        })
}

/// First measure the entire independent saved source constructor, ordinary HIR
/// receiver, validation and Drop. Prove every survivor belongs to the compiler
/// prelude static; repeat the complete original receiver and require zero live.
/// The caller's subsequent complete new receiver measurement stays untouched.
pub(super) fn complete_original_receiver_before_measurement(programs: &[Program]) {
    let authored = index_authored(programs).unwrap();
    let run = || {
        let registry = LayoutRegistry::new(programs, &authored);
        let owned = old::construct(&programs[0], &authored, programs, Some(&registry)).unwrap();
        let layout = registry.get(&programs[0], &authored, programs).unwrap();
        let resolved = linked(|| {
            crate::hir::resolve_with_function_reuse(
                &owned,
                None,
                None,
                None,
                layout.presentation(&programs[0], &owned).unwrap(),
            )
        })
        .unwrap();
        crate::hir::validate(&resolved.0).unwrap();
        drop(resolved);
        drop(owned);
        drop(layout);
        drop(registry);
    };
    let ((receipt, attributed), first) = measure(|| {
        run();
        // Tripwires run before the observer registry is borrowed. The pointer
        // predicate itself must never allocate, including a panic payload.
        for declaration in crate::prelude::declarations() {
            assert!(
                declaration.extends.is_none() && declaration.invariants.is_none(),
                "prelude owner census must include new payloads"
            );
            assert!(
                matches!(
                    declaration.kind,
                    TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Variant { .. }
                ),
                "prelude owner census must include new declaration kind"
            );
        }
        (current_metrics().live, live_bytes_matching(prelude_owns))
    });
    assert!(
        !first.invalid,
        "complete original initialization receipt: {first:?}"
    );
    assert_eq!(receipt, first.live);
    assert_eq!(
        first.allocated,
        first.freed.checked_add(first.live).unwrap()
    );
    assert_eq!(
        first.live, attributed,
        "unowned survivor after full original Drop"
    );
    // Preserve the exact frozen-source cold receipt seen in selector 22. Warm
    // test processes may have initialized this immutable process owner earlier.
    if first.live != 0 && cfg!(target_pointer_width = "64") {
        assert_eq!(
            first.live, 6420,
            "changed prelude cold initialization receipt"
        );
    }
    let (_, repeated) = measure(run);
    assert!(
        !repeated.invalid && repeated.live == 0,
        "complete repeated original: {repeated:?}"
    );
    assert_eq!(repeated.allocated, repeated.freed);
}

#[test]
fn complete_original_cold_prelude_pointer_receipt_and_repeated_drop_are_independent() {
    let programs = super::sources(super::SCALAR_CALLER, super::SCALAR_PROVIDER);
    complete_original_receiver_before_measurement(&programs);
}
