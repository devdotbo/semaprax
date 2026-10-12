//! Full identity state, actual requested backing, Drop and each paid boundary.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
const LIMIT: usize = 64 * 1024 * 1024;
fn keys() -> (Vec<ExpressionId>, Vec<ValueId>) {
    let execution = FunctionExecutionId::Monomorphic(DeclarationId::new("identity.owner"));
    let expressions = (0..64)
        .map(|index| ExpressionId::new(&execution, &format!("body.arg.{index:04}")))
        .collect();
    let values = (0..64)
        .map(|index| ValueId::new(format!("identity.value.{index:04}")))
        .collect();
    (expressions, values)
}
fn lifetime<K: Ord + Clone + std::fmt::Debug>(
    keys: &[K],
    limit: usize,
    insert: fn(&mut BTreeSet<K>, &K) -> Result<bool, Diagnostic>,
) -> (crate::test_allocation_observer::Metrics, bool, usize, usize) {
    // Inputs and the complete standard-library insertion oracle are outside the
    // observation. Every current constructor and the entire current Drop is in it.
    let expected = keys.iter().cloned().collect::<BTreeSet<_>>();
    let ((accepted, actual), refused, paid) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let mut current = BTreeSet::new();
                let mut accepted = 0;
                for id in keys {
                    match insert(&mut current, id) {
                        Ok(true) => accepted += 1,
                        Ok(false) => panic!("distinct owning fixture identities"),
                        Err(diagnostic) => {
                            assert!(crate::bounded_output::active_refused());
                            assert!(current.iter().eq(expected.iter().take(accepted)));
                            drop(diagnostic);
                            let before = current_metrics();
                            let retry = insert(&mut current, id);
                            assert!(retry.is_err());
                            drop(retry);
                            let after = current_metrics();
                            assert_eq!(after.allocations, before.allocations);
                            assert_eq!(after.allocated, before.allocated);
                            assert!(current.iter().eq(expected.iter().take(accepted)));
                            break;
                        }
                    }
                }
                assert!(current.iter().eq(expected.iter().take(accepted)));
                if accepted == keys.len() {
                    assert_eq!(current, expected);
                }
                drop(current);
                accepted
            })
        });
    assert!(!actual.invalid);
    assert_eq!(actual.live, 0);
    assert_eq!(actual.allocated, actual.freed);
    assert!(
        actual.allocated <= paid && actual.highwater <= paid,
        "actual {actual:?}, paid {paid}"
    );
    (actual, refused, paid, accepted)
}
fn check<K: Ord + Clone + std::fmt::Debug>(
    keys: &[K],
    insert: fn(&mut BTreeSet<K>, &K) -> Result<bool, Diagnostic>,
) {
    let node = crate::cleanup_plan::replay_btree_node_upper::<K, ()>().unwrap();
    for count in [1, 2, 17, 64] {
        let keys = &keys[..count];
        let (whole, refused, paid, accepted) = lifetime(keys, LIMIT, insert);
        assert!(!refused);
        assert_eq!(accepted, count);
        assert_eq!(paid, node * count);
        assert!(whole.allocations > 0 && whole.allocated > 0);
        let (exact, refused, exact_paid, accepted) = lifetime(keys, paid, insert);
        assert!(!refused);
        assert_eq!(accepted, count);
        assert_eq!(exact_paid, paid);
        assert_eq!(exact.allocated, whole.allocated);
        // Each constructor's boundary keeps the complete already-admitted prefix.
        for step in 1..=count {
            let (_, refused, short_paid, accepted) = lifetime(keys, node * step - 1, insert);
            assert!(refused);
            assert_eq!(accepted, step - 1);
            assert_eq!(short_paid, node * (step - 1));
            let (_, refused, boundary_paid, accepted) = lifetime(keys, node * step, insert);
            assert_eq!(refused, step < count);
            assert_eq!(accepted, step);
            assert_eq!(boundary_paid, node * step);
        }
        let (zero, refused, paid, accepted) = lifetime(keys, 0, insert);
        assert!(refused);
        assert_eq!(paid, 0);
        assert_eq!(accepted, 0);
        assert_eq!(zero.allocations, 0);
        assert_eq!(zero.allocated, 0);
    }
}
#[test]
fn expression_and_value_insertions_preserve_full_old_ordered_identity_state_and_paid_drop() {
    let (expressions, values) = keys();
    check(&expressions, expression);
    check(&values, value);
}
#[test]
fn duplicate_identity_keeps_original_priority_without_a_new_constructor() {
    let (expressions, values) = keys();
    let mut expressions_set = expressions.iter().cloned().collect::<BTreeSet<_>>();
    let mut values_set = values.iter().cloned().collect::<BTreeSet<_>>();
    let before_expressions = expressions_set.clone();
    let before_values = values_set.clone();
    let (((), actual), refused, paid) = crate::bounded_output::with_limit_usage(0, || {
        measure(|| {
            assert!(!expression(&mut expressions_set, &expressions[0]).unwrap());
            assert!(!value(&mut values_set, &values[0]).unwrap());
        })
    });
    assert!(!refused && !actual.invalid);
    assert_eq!(paid, 0);
    assert_eq!(actual.allocations, 0);
    assert_eq!(actual.allocated, 0);
    assert_eq!(actual.live, 0);
    assert_eq!(expressions_set, before_expressions);
    assert_eq!(values_set, before_values);
}
