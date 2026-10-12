use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn ordinary_numeric_memo_keeps_exact_pointer_continuation_and_opposite_rank_storage() {
    let source = crate::check("module memo.rank; @id(\"main\") fn main()->i64 {0}", "memo-rank.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let function = &program.functions[0];
    // Real simultaneous Rc addresses, then exact opposite allocator ranks.
    // These live outside the narrow memo-storage observer.
    let nodes = (0..128).map(|_| Rc::new(ObservationDag::legacy(
        ObservationDagKind::Seq(None, Vec::new()),
    ))).collect::<Vec<_>>();
    let mut pointers = nodes.iter().map(|node| Rc::as_ptr(node) as usize).collect::<Vec<_>>();
    pointers.sort_unstable();
    let run = |reverse: bool| {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        measure(|| {
            let mut memo = Memo::Ordinary(BTreeMap::new());
            let mut old = BTreeMap::new();
            // Saved old key/value meaning, built outside no claimed paid boundary.
            for ordinal in 0..128 {
                let pointer = pointers[if reverse {127 - ordinal} else {ordinal}];
                old.insert((pointer, 3usize), ordinal + 7);
                insert(function, &mut memo, test_key(pointer, ordinal, 3), ordinal + 7, &mut budget).unwrap();
            }
            for ordinal in 0..128 {
                let pointer = pointers[if reverse {127 - ordinal} else {ordinal}];
                assert_eq!(get(function, &memo, &test_key(pointer, ordinal, 3), &mut budget).unwrap(), old.get(&(pointer, 3)).copied());
                assert_eq!(get(function, &memo, &test_key(pointer, ordinal, 4), &mut budget).unwrap(), None);
            }
            // The independent saved map has rank-dependent storage; exclude it
            // from the allocation comparison by comparing new map separately.
            drop(old);
            drop(memo);
        })
    };
    let (_, left) = run(false);
    let (_, right) = run(true);
    assert!(!left.invalid && !right.invalid);
    assert_eq!(left.live, 0);
    assert_eq!(right.live, 0);
    // Full result parity above includes the saved map. Observe only memo below.
    let storage = |reverse: bool| {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        measure(|| {
            let mut memo = Memo::Ordinary(BTreeMap::new());
            for ordinal in 0..128 {
                let pointer = pointers[if reverse {127 - ordinal} else {ordinal}];
                insert(function, &mut memo, test_key(pointer, ordinal, 3), ordinal + 7, &mut budget).unwrap();
            }
            drop(memo);
        }).1
    };
    let left = storage(false);
    let right = storage(true);
    assert_eq!(left, right);
    assert!(!left.invalid);
    assert_eq!(left.live, 0);
    assert_eq!(left.allocated, left.freed);
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let mut memo = Memo::Ordinary(BTreeMap::new());
    insert(function, &mut memo, test_key(pointers[0], 0, 3), 7, &mut budget).unwrap();
    assert!(get(function, &memo, &test_key(pointers[1], 0, 3), &mut budget).is_err());
    assert!(insert(function, &mut memo, test_key(pointers[1], 0, 3), 8, &mut budget).is_err());
    assert_eq!(get(function, &memo, &test_key(pointers[0], 0, 3), &mut budget).unwrap(), Some(7));
    let detached = Key { pointer: pointers[0], continuation: 3, ordinal: None };
    assert!(get(function, &memo, &detached, &mut budget).is_err());
}
