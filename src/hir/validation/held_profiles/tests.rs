use super::*;
use crate::bounded_output::{with_limit, with_limit_usage};
use crate::test_allocation_observer::measure;

const ORIGINAL_UNSUPPORTED_GENERIC_VECTOR_SOURCE: &str = r#"module held.profile;
@id("held.box") record Boxed<T> { @id("held.box.field") field: T, }
@id("held.bag") record Bag<T> { @id("held.bag.items") items: Vec<T>, }
@id("held.take") fn take(value: borrow Bag<i64>, other: Boxed<i64>) -> i64 { 0 }
@id("held.other") fn other(value: borrow Bag<i64>) -> i64 { 0 }
@id("held.id") fn identity<T>(value: own Box<T>) -> Box<T> { value }
@id("held.main") fn main() -> i64 { let wrapped = identity<i64>(box_new<i64>(0)); box_into_inner<i64>(wrapped) }
"#;

pub(in crate::hir::validation) fn fixture() -> ResolvedProgram {
    // Generic copy records admit scalar arguments by value. Keep an independently
    // checked Vec parameter for the hostile nested generic-argument receiver.
    let source = ORIGINAL_UNSUPPORTED_GENERIC_VECTOR_SOURCE
        .replace("items: Vec<T>", "items: T")
        .replace("borrow Bag<i64>", "Bag<i64>")
        .replace(
            "other: Boxed<i64>",
            "other: Boxed<i64>, rows: borrow Vec<i64>",
        );
    assert_eq!(source.matches("items: T").count(), 1);
    assert_eq!(source.matches("rows: borrow Vec<i64>").count(), 1);
    crate::hir::resolve(&crate::check(&source, "held-generic.spx").unwrap()).unwrap()
}

#[test]
fn sealed_lookup_keeps_full_function_instance_parameter_order_and_rejects_foreign_and_temporary_types(
) {
    let program = fixture();
    let foreign = program.clone();
    let bound = Bound::new(&program);
    for function in &program.functions {
        let call = bound.call(&function.id, None).unwrap();
        assert!(std::ptr::eq(
            call.function(),
            program.resolve_call_target(&function.id, None).unwrap()
        ));
        for (index, param) in function.params.iter().enumerate() {
            let carrier = call.parameter(index);
            assert!(std::ptr::eq(carrier.param(), param));
            assert!(bound.authenticate(&carrier, &param.ty).is_some());
            let temporary = param.ty.clone();
            assert!(bound.authenticate(&carrier, &temporary).is_none());
            let other = Bound::new(&foreign)
                .call(&function.id, None)
                .unwrap()
                .parameter(index);
            assert!(bound.authenticate(&other, &other.param().ty).is_none());
        }
    }
    for instance in &program.function_instances {
        let call = bound.call(&instance.template, Some(&instance.id)).unwrap();
        assert!(std::ptr::eq(
            call.function(),
            program
                .resolve_call_target(&instance.template, Some(&instance.id))
                .unwrap()
        ));
        for (index, param) in instance.function.params.iter().enumerate() {
            assert!(bound
                .authenticate(&call.parameter(index), &param.ty)
                .is_some());
        }
        assert!(bound
            .call(&DeclarationId::new("missing.template"), Some(&instance.id))
            .is_none());
    }
    assert!(bound.call(&DeclarationId::new("unknown"), None).is_none());
}

#[test]
fn bounded_directory_prepays_actual_replacement_drop_and_declines_optional_growth_without_refusal()
{
    let program = fixture();
    let key = Key {
        slot: Slot::Function(0),
        parameter: 0,
    };
    let (((), metrics), refused, paid) = with_limit_usage(usize::MAX, || {
        measure(|| {
            paid::fund(Some(std::mem::size_of::<Bound<'_>>())).unwrap();
            let mut bound = Bound::new(&program);
            bound.remember(key, true).unwrap();
            assert_eq!(bound.recall(key), Some(true));
            drop(bound);
        })
    });
    assert!(!refused && !metrics.invalid);
    assert!(metrics.allocated <= paid);
    assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
    let (((), metrics), refused, used) = with_limit_usage(0, || {
        measure(|| {
            let mut bound = Bound::new(&program);
            bound.remember(key, true).unwrap();
            assert_eq!(bound.recall(key), None);
            assert_eq!(bound.owned_capacity(), 0);
            drop(bound);
        })
    });
    assert!(!refused && !metrics.invalid);
    assert_eq!((used, metrics.allocated, metrics.live), (0, 0, 0));
    with_limit(usize::MAX, || {
        let mut bound = Bound::new(&program);
        for index in 0..MAX_CACHE_ENTRIES + 17 {
            bound
                .remember(
                    Key {
                        slot: Slot::Function(index),
                        parameter: 0,
                    },
                    true,
                )
                .unwrap();
        }
        assert_eq!(bound.held.len(), MAX_CACHE_ENTRIES);
    });
}

#[test]
fn complete_original_generic_vector_field_source_keeps_full_t223_refusal() {
    let source = ORIGINAL_UNSUPPORTED_GENERIC_VECTOR_SOURCE;
    let parsed = crate::parse(source, "held-generic.spx").unwrap();
    let expected = crate::verify::verify(&parsed);
    let actual = crate::check(source, "held-generic.spx").unwrap_err();
    assert_eq!(actual[0].code, "SPX-T223");
    assert_eq!(actual, expected);
}

#[test]
fn complete_previous_nested_generic_borrow_fixture_keeps_full_t223_and_o002_refusal() {
    let source = ORIGINAL_UNSUPPORTED_GENERIC_VECTOR_SOURCE
        .replace("items: Vec<T>", "items: T")
        .replace("borrow Bag<i64>", "borrow Bag<Vec<i64>>");
    let parsed = crate::parse(&source, "held-generic.spx").unwrap();
    let expected = crate::verify::verify(&parsed);
    let actual = crate::check(&source, "held-generic.spx").unwrap_err();
    assert_eq!(actual[0].code, "SPX-T223");
    assert!(actual
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-O002"));
    assert_eq!(actual, expected);
}
