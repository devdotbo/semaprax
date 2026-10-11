use super::*;
#[test]
fn slot_reads_aliases_and_owned_call_expiry_consume_only_original_string_budget() {
    let source = include_str!("../../../tests/fixtures/string-slot-views.spx");
    let program = hir::resolve(&crate::check(source, "slots.spx").unwrap()).unwrap();
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let admitted = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect();
    let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
        entry,
        &[],
        &admitted,
        &program,
        1_000_000,
        false,
        Utf8MaterializationBudget::Fixed {
            used_materializations: MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS - 3,
            used_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 5,
        },
    );
    assert!(matches!(outcome, Ok(Value::Int(42))), "{outcome:?}");
    assert_eq!(
        usage,
        (
            MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS,
            MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES
        )
    );
}
#[test]
fn slot_backing_is_same_allocation_and_alias_bytes_enforce_internal_extent() {
    let vector = Arc::new(owned_vec::OwnedVecValue {
        element: ResolvedType::String,
        values: vec![Value::String("x".repeat(131_073))],
        capacity: 1,
        generation: 7,
    });
    let Value::String(original) = &vector.values[0] else {
        unreachable!()
    };
    let pointer = original.as_ptr();
    let backing = Utf8Backing::VectorStringSlot {
        vector: Arc::clone(&vector),
        index: 0,
        generation: 7,
    };
    assert_eq!(backing.as_ref().as_ptr(), pointer);
    assert_eq!(backing.as_ref().len(), 131_073); // Str extent is not the Slice limit.
    assert!(validate_slice_extent(&backing).is_err());
    assert!(validate_slice_extent(&backing.clone()).is_err());
    drop(backing);
    assert_eq!(Arc::strong_count(&vector), 1);
    let exact = Arc::new(owned_vec::OwnedVecValue {
        element: ResolvedType::String,
        values: vec![Value::String("x".repeat(131_072))],
        capacity: 1,
        generation: 8,
    });
    assert!(
        validate_slice_extent(&Utf8Backing::VectorStringSlot {
            vector: exact,
            index: 0,
            generation: 8
        })
        .is_ok()
    );
}
