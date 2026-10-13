use super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
use std::collections::BTreeMap;
#[derive(Clone, Copy)]
enum Op {
    Insert(&'static str, bool),
    Get(&'static str),
    Remove(&'static str),
}
const OPS: &[Op] = &[
    Op::Insert("m", true),
    Op::Insert("g", false),
    Op::Insert("t", true),
    Op::Insert("d", true),
    Op::Insert("j", false),
    Op::Insert("p", true),
    Op::Insert("w", false),
    Op::Insert("a", true),
    Op::Insert("f", false),
    Op::Insert("h", true),
    Op::Insert("l", false),
    Op::Insert("n", true),
    Op::Insert("r", false),
    Op::Insert("u", true),
    Op::Insert("z", false),
    Op::Insert("é\0suffix", true),
    Op::Insert("", false),
    Op::Get("m"),
    Op::Get("missing"),
    Op::Insert("g", true),
    Op::Remove("m"),
    Op::Remove("t"),
    Op::Remove("g"),
    Op::Remove("a"),
    Op::Remove("missing"),
    Op::Insert("b", false),
    Op::Insert("q", true),
    Op::Insert("s", false),
    Op::Insert("é\0suffix", false),
    Op::Get("é\0suffix"),
    Op::Remove("é\0suffix"),
    Op::Get("é\0suffix"),
];
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(
            r#"module ordered; @id("main") fn main()->i64 {7}"#,
            "ordered.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn original() -> Vec<Option<bool>> {
    // Complete ordinary owned String map is independent of the new carrier.
    let mut map = BTreeMap::<String, bool>::new();
    OPS.iter()
        .map(|op| match op {
            Op::Insert(key, value) => map.insert((*key).to_owned(), *value),
            Op::Get(key) => map.get(*key).copied(),
            Op::Remove(key) => map.remove(*key),
        })
        .collect()
}
// Test assertion only: no allocation, budget, or authority. It does not count
// as production work evidence. The fixed bound belongs only to this small case.
fn verify(map: &Map<bool>) {
    assert!(map.nodes.len() <= 128);
    let mut seen = [false; 128];
    fn visit<'a>(
        map: &'a Map<bool>,
        index: Option<usize>,
        seen: &mut [bool; 128],
        lower: Option<&'a str>,
        upper: Option<&'a str>,
    ) -> u32 {
        let Some(index) = index else {
            return 0;
        };
        assert!(index < map.nodes.len() && !seen[index]);
        seen[index] = true;
        let node = &map.nodes[index];
        let (key, _) = node.entry.as_ref().unwrap();
        assert!(lower.is_none_or(|lower| lower < key.as_str()));
        assert!(upper.is_none_or(|upper| key.as_str() < upper));
        let left = visit(map, node.left, seen, lower, Some(key));
        let right = visit(map, node.right, seen, Some(key), upper);
        assert!(left.abs_diff(right) < 2);
        assert_eq!(node.height, left.max(right) + 1);
        node.height
    }
    visit(map, map.root, &mut seen, None, None);
    let mut free = map.free;
    while let Some(index) = free {
        assert!(index < map.nodes.len() && !seen[index]);
        seen[index] = true;
        let node = &map.nodes[index];
        assert!(node.entry.is_none() && node.height == 0 && node.right.is_none());
        free = node.left;
    }
    assert!(seen[..map.nodes.len()].iter().all(|seen| *seen));
}
fn execute(map: &mut Map<bool>, op: Op, payer: &mut Payer<'_>) -> Result<Option<bool>, Diagnostic> {
    match op {
        Op::Insert(key, value) => {
            // The caller separately owns key creation/copy/final release; the
            // map never clones it or claims authority over an unpaid payload.
            inventory_metadata::step(payer, 1)?;
            let key = inventory_text::raw(payer, format_args!("{key}"))?;
            map.insert(key, value, payer)
        }
        Op::Get(key) => map.get(key, payer).map(|found| found.copied()),
        Op::Remove(key) => map.remove(key, payer),
    }
}
fn run(
    bytes: usize,
    work: usize,
    function: &ResolvedFunction,
    expected: &[Option<bool>],
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut matched = true;
                let result = budget
                    .charge(function, 0, "ordered receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        let mut payer = super::super::tests::payer(function, &mut budget);
                        let mut map = Map::acquire(&mut payer)?;
                        let result = (|| {
                            for (i, op) in OPS.iter().copied().enumerate() {
                                let value = execute(&mut map, op, &mut payer)?;
                                matched &= value == expected[i];
                                verify(&map);
                            }
                            Ok::<_, Diagnostic>(())
                        })();
                        if result.is_err() {
                            let before = current_metrics();
                            let retry = map.get("m", &mut payer);
                            assert!(retry.is_err());
                            assert_eq!(current_metrics().allocated, before.allocated);
                            drop(retry);
                        }
                        // Includes every retained key/slot/frontier and failed input.
                        drop(map);
                        result
                    });
                let failed = result.is_err();
                let used = work.saturating_sub(budget.remaining);
                drop(result);
                drop(budget);
                (matched, failed, used)
            })
        });
    (outcome, observed, refused, paid)
}
#[test]
fn online_avl_keeps_complete_owned_map_receiver_rotations_duplicates_removal_and_drop() {
    let program = fixture();
    let expected = original();
    let (ample, observed, refused, paid) = run(
        usize::MAX,
        MAX_REPLAY_WORK_UNITS,
        &program.functions[0],
        &expected,
    );
    assert!(ample.0 && !ample.1 && !refused && !observed.invalid);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    let (exact, observed, refused, used) = run(paid, ample.2, &program.functions[0], &expected);
    assert!(exact.0 && !exact.1 && !refused && !observed.invalid);
    assert_eq!(used, paid);
    assert_eq!(observed.live, 0);
}
#[test]
fn actual_avl_backing_and_work_zero_exact_short_keep_first_refusal_and_receiving_drop() {
    let program = fixture();
    let expected = original();
    let (ample, _, _, paid) = run(
        usize::MAX,
        MAX_REPLAY_WORK_UNITS,
        &program.functions[0],
        &expected,
    );
    assert!(ample.0 && !ample.1);
    for (bytes, work) in [
        (0, MAX_REPLAY_WORK_UNITS),
        (paid - 1, MAX_REPLAY_WORK_UNITS),
        (paid, 0),
        (paid, ample.2 - 1),
    ] {
        let (short, observed, _, used) = run(bytes, work, &program.functions[0], &expected);
        assert!(short.1 && !observed.invalid && observed.allocated <= used);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn paid_avl_reuses_deleted_slots_and_retains_actual_frontier_capacity() {
    let program = fixture();
    let (((), observed), refused, paid) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::new();
                budget.charge(function, 0, "ordered reuse setup").unwrap();
                let mut payer = super::super::tests::payer(function, &mut budget);
                let mut map = Map::acquire(&mut payer).unwrap();
                for key in ["m", "g", "t", "d", "j", "p", "w"] {
                    execute(&mut map, Op::Insert(key, true), &mut payer).unwrap();
                }
                let len = map.nodes.len();
                let capacity = map.nodes.capacity();
                let backing = map.nodes.as_ptr();
                for key in ["m", "d", "g"] {
                    assert_eq!(map.remove(key, &mut payer).unwrap(), Some(true));
                }
                for key in ["a", "b", "c"] {
                    assert_eq!(
                        execute(&mut map, Op::Insert(key, false), &mut payer).unwrap(),
                        None
                    );
                }
                assert_eq!(map.nodes.len(), len);
                assert_eq!(map.nodes.capacity(), capacity);
                assert_eq!(map.nodes.as_ptr(), backing);
                verify(&map);
                drop(map);
                drop(budget);
            })
        });
    assert!(!refused && !observed.invalid && observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
