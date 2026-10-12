//! Full old BTree seed policy and literal actual paid cursor/node receivers.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use std::mem::size_of;

#[test]
fn paid_seed_clone_cursor_and_remove_literal_heap_debit_exact_short_full_drop() {
    let node = size_of::<tree::Node<u16, ()>>();
    // Independent layout: precisely two borrowed optional references.
    let cursor = 2 * size_of::<Option<&tree::Node<u16, ()>>>();
    assert_eq!(size_of::<tree::Keys<'_, u16, ()>>(), cursor);
    let exact = size_of::<CfgReservation>()
        + 46
        + 2 * size_of::<Set<u16>>()
        + 12 * node
        + 3 * cursor
        + size_of::<Vec<u16>>();
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                let mut seeds = Set::new(&owner).unwrap();
                for key in [2, 0, 1] {
                    set_insert(&mut seeds, key, Some(&owner)).unwrap();
                }
                let mut copy = clone_set(&seeds, Some(&owner)).unwrap();
                assert!(seeds.iter(&owner).unwrap().copied().eq([0, 1, 2]));
                assert!(copy.iter(&owner).unwrap().copied().eq([0, 1, 2]));
                for key in [1, 0, 2] {
                    assert!(copy.remove(&key, &owner).unwrap());
                    assert!(!copy.remove(&key, &owner).unwrap());
                    assert!(seeds.contains(&key));
                }
                assert!(copy.is_empty());
                assert_eq!(copy.len(), 0);
                let success = match copy.into_vec(&owner) {
                    Ok(values) => {
                        assert!(values.is_empty());
                        drop(values);
                        true
                    }
                    Err(_) => {
                        let first = owner.first().unwrap();
                        assert_eq!(owner.reserve(0), Err(first));
                        assert_eq!(owner.reserve(1), Err(first));
                        assert!(seeds.remove(&0, &owner).is_err());
                        assert_eq!(seeds.len(), 3);
                        assert!(seeds.contains(&0));
                        false
                    }
                };
                drop(seeds);
                drop(owner);
                success
            })
        })
    };
    let ((success, actual), refused, used, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(first, None);
    assert_eq!(used, exact);
    assert_eq!(actual.allocations, 7); // text plus three original and three copied nodes
    assert_eq!(actual.reallocations, 0);
    assert_eq!(actual.allocated, 46 + 6 * node);
    assert_eq!(actual.freed, actual.allocated);
    assert_eq!(actual.live, 0);
    assert!(!actual.invalid);
    let ((success, short), refused, used, first) = run(exact - 1);
    assert!(!success);
    assert!(refused);
    assert!(first.is_some());
    assert_eq!(used, exact - size_of::<Vec<u16>>());
    assert_eq!(short, actual);
}

#[test]
fn paid_cursor_refusal_and_sticky_removal_preserve_complete_original_set() {
    let setup = size_of::<CfgReservation>()
        + 46
        + size_of::<Set<u16>>()
        + 6 * size_of::<tree::Node<u16, ()>>();
    let cursor = 2 * size_of::<Option<&tree::Node<u16, ()>>>();
    for available in 0..cursor {
        let (((), actual), refused, used, first) =
            with_limit_usage_refusal(setup + available, || {
                measure(|| {
                    let owner = CfgReservation::acquire().unwrap();
                    let mut seeds = Set::new(&owner).unwrap();
                    for key in [2, 0, 1] {
                        set_insert(&mut seeds, key, Some(&owner)).unwrap();
                    }
                    assert!(seeds.iter(&owner).is_err());
                    let first = owner.first().unwrap();
                    assert_eq!(owner.reserve(0), Err(first));
                    assert_eq!(owner.reserve(1), Err(first));
                    for key in [0, 1, 2, 3] {
                        assert!(seeds.remove(&key, &owner).is_err());
                    }
                    assert_eq!(seeds.len(), 3);
                    for key in [0, 1, 2] {
                        assert!(seeds.contains(&key));
                    }
                    assert!(!seeds.contains(&3));
                    drop(seeds);
                    drop(owner);
                })
            });
        assert!(refused);
        assert!(first.is_some());
        assert_eq!(used, setup);
        assert_eq!(actual.allocations, 4);
        assert_eq!(actual.reallocations, 0);
        assert_eq!(actual.allocated, 46 + 3 * size_of::<tree::Node<u16, ()>>());
        assert_eq!(actual.freed, actual.allocated);
        assert_eq!(actual.live, 0);
        assert!(!actual.invalid);
    }
}

#[test]
fn complete_old_seed_clone_and_all_avl_removal_shapes_keep_order_and_actual_receivers() {
    for count in [0u16, 1, 2, 17, 128, MAX_LOAN_ENDPOINTS_V1 as u16] {
        let orders: [Vec<u16>; 4] = [
            (0..count).collect(),
            (0..count).rev().collect(),
            (0..count).step_by(2).chain((1..count).step_by(2)).collect(),
            if count >= 3 {
                [2, 0, 1].into_iter().chain(3..count).collect()
            } else {
                (0..count).collect()
            },
        ];
        for order in &orders {
            let (((), old), refused, old_paid) = with_limit_usage(usize::MAX, || {
                measure(|| {
                    let owner = CfgReservation::acquire().unwrap();
                    let mut seeds = BTreeSet::new();
                    for key in order {
                        storage::set_insert(&mut seeds, *key, Some(&owner)).unwrap();
                    }
                    let mut copy = storage::clone_set(&seeds, Some(&owner)).unwrap();
                    assert!(copy.iter().copied().eq(0..count));
                    for key in order {
                        assert!(copy.remove(key));
                        assert!(!copy.remove(key));
                        assert!(seeds.contains(key));
                    }
                    assert!(copy.is_empty());
                    drop(copy);
                    drop(seeds);
                    drop(owner);
                })
            });
            assert!(!refused);
            let (((), actual), refused, paid) = with_limit_usage(usize::MAX, || {
                measure(|| {
                    let owner = CfgReservation::acquire().unwrap();
                    let mut seeds = Set::new(&owner).unwrap();
                    for key in order {
                        set_insert(&mut seeds, *key, Some(&owner)).unwrap();
                    }
                    let mut copy = clone_set(&seeds, Some(&owner)).unwrap();
                    let mut remaining: u16 = count;
                    assert!(copy.iter(&owner).unwrap().copied().eq(0..count));
                    for key in order {
                        assert!(copy.remove(key, &owner).unwrap());
                        assert!(!copy.remove(key, &owner).unwrap());
                        remaining -= 1;
                        assert_eq!(copy.len(), usize::from(remaining));
                        assert!(seeds.contains(key));
                        assert!(!copy.contains(key));
                        let bound = if remaining == 0 {
                            0
                        } else {
                            2 * (u16::BITS - remaining.leading_zeros()) as usize
                        };
                        assert!(tree::height(&copy.0.root) <= bound);
                    }
                    assert!(copy.is_empty());
                    assert!(seeds.iter(&owner).unwrap().copied().eq(0..count));
                    drop(copy);
                    drop(seeds);
                    drop(owner);
                })
            });
            assert!(!refused);
            assert!(actual.allocated <= paid);
            assert!(actual.highwater <= paid);
            for observed in [old, actual] {
                assert_eq!(observed.freed, observed.allocated);
                assert_eq!(observed.live, 0);
                assert!(!observed.invalid);
            }
            eprintln!("seed count={count} old={old:?} old_debit={old_paid} current={actual:?} debit={paid}");
        }
    }
}
