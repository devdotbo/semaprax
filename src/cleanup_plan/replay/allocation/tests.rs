//! The active builder pays storage; it never authenticates selected replay.
use super::*;
mod old;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};
fn program() -> ResolvedProgram {
    let source = crate::check(
        r#"module replay.payer;
@id("payer.main") fn main()->i64 {
 let data=bytes_zeroed(1usize); let text="x";
 if byte_len(bytes_as_slice(data))==1usize && str_len_bytes(string_as_str(text))==1 {7}else{0}
}"#,
        "ordinary-replay-payer.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn indexes(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
    let facts = expression_facts::with_budget(function, budget)?;
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    assert_eq!(facts.len(), expression_facts(function)?.len());
    assert_eq!(storage, old::validate_slots(function)?);
    let old = old::collect_leaves(function)?;
    assert_eq!(leaves.len(), old.len());
    for (flag, leaf) in leaves.iter() {
        assert_eq!(leaf.place, old[flag].place);
        assert_eq!(leaf.lifecycle, old[flag].lifecycle);
    }
    Ok(())
}
#[test]
fn ordinary_builder_payment_never_grants_selected_admission_and_covers_index_drop() {
    let program = program();
    let function = &program.functions[0];
    // Reference collections are constructed outside both the builder and the
    // observer. Their original arbitrary-shape semantics remain independent.
    let old_facts = expression_facts(function).unwrap();
    let old_storage = old::validate_slots(function).unwrap();
    let old_leaves = old::collect_leaves(function).unwrap();
    let (((), metrics), refused, charged, receipt) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            assert_eq!(budget.admission, AdmissionMode::Ordinary);
            assert_eq!(budget.payer, AllocationPayer::ActiveBuilder);
            assert!(budget.physical.is_none());
            let facts = expression_facts::with_budget(function, &mut budget).unwrap();
            let storage = validate_slots_paid(function, &mut budget).unwrap();
            let leaves = collect_leaves_paid(function, &mut budget).unwrap();
            assert_eq!(facts.len(), old_facts.len());
            assert_eq!(storage, old_storage);
            for (flag, leaf) in leaves.iter() {
                assert_eq!(leaf.place, old_leaves[flag].place);
                assert_eq!(leaf.lifecycle, old_leaves[flag].lifecycle);
            }
            assert!(!budget.selected());
            drop((facts, storage, leaves, budget));
        })
    });
    assert!(!refused && receipt.is_none() && !metrics.invalid);
    assert!(metrics.allocations > 0);
    assert!(metrics.allocated <= charged);
    assert!(metrics.highwater <= charged);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(metrics.live, 0);
    // The actual ordinary replay entry keeps its complete authority checks.
    let mut legacy = ReplayBudget::unobserved();
    validate_structure_with_budget(&program, function, &mut legacy, None).unwrap();
    let (checked, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut active = ReplayBudget::new();
        let result = validate_structure_with_budget(&program, function, &mut active, None);
        assert!(!active.selected() && active.physical.is_none());
        result
    });
    checked.unwrap();
    assert!(!refused);
    let mut hostile = function.clone();
    hostile.cleanup_plan.slots[0].id.0 += 1;
    let old = old::validate_slots(&hostile).err().unwrap();
    let (new, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        validate_slots_paid(&hostile, &mut ReplayBudget::new())
            .err()
            .unwrap()
    });
    assert_eq!(new, old);
    assert!(!refused);
}
#[test]
fn active_builder_zero_exact_short_refusal_is_sticky_and_moves_paid_text() {
    let program = program();
    let function = &program.functions[0];
    // Independent literal: one prepaid String request, then a u64 array.
    let text_bytes = function.id.as_str().len() + 512;
    let full = text_bytes + 3 * std::mem::size_of::<u64>();
    for limit in [0, text_bytes - 1, full - 1, full] {
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget.reserve_array::<u64>(function, 3, "ordinary literal array");
                if limit == full {
                    result.unwrap();
                    let values = vec![0u64; 3];
                    drop(values);
                } else {
                    let error = result.err().expect("exactly short builder refuses");
                    assert_eq!(error.code, "SPX-H006");
                    if limit >= text_bytes {
                        assert!(!error.message.is_empty());
                    } else {
                        assert!(error.message.is_empty());
                    }
                    let before = current_metrics();
                    assert!(budget.reserve_bytes(function, 0, "sticky zero").is_err());
                    assert!(budget
                        .reserve_vec(function, &mut Vec::<u64>::new(), 0, "sticky spare")
                        .is_err());
                    assert_eq!(current_metrics(), before);
                    assert!(!budget.selected());
                    drop(error);
                }
                drop(budget);
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(refused, limit != full);
        if limit == full {
            assert_eq!(used, full);
            assert!(receipt.is_none());
        } else {
            assert!(receipt.is_some());
            assert!(used <= limit);
        }
    }
    // Complete ordinary verdict comparison remains outside allocation evidence.
    let mut unobserved = ReplayBudget::unobserved();
    indexes(function, &mut unobserved).unwrap();
}
