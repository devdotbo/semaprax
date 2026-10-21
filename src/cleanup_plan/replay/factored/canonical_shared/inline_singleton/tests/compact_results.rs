//! Additive complete-owner controls; every original predecessor test stays unchanged.
use super::super::lexical_successors::frame_loop::compact::result_banks;
use super::*;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
#[allow(
    clippy::too_many_arguments,
    reason = "Matched whole-owner predecessor and compact transport controls use identical actual calls."
)]
fn run(
    compact: bool,
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    root: usize,
    weight: u64,
    limit: usize,
    work: usize,
    floor: usize,
) -> Run {
    let ((result, remaining, skeleton, spent, heap, shapes, successors), refused, paid, _) =
        with_limit_usage_refusal(limit, || {
            let ((result, remaining, skeleton, spent, shapes, successors), heap) = measure(|| {
                let mut b = ReplayBudget::with_skeleton_limit(work);
                b.remaining = work;
                let observer = old_census::observe(f, &b);
                let shapes = shape_census::observe(f, &b);
                assert!(crate::bounded_output::set_active_floor(floor));
                let result = (|| {
                    b.reserve_bytes(
                        f,
                        Canonicalizer::CONSTRUCTOR_CONTROLS + NEW_CALL,
                        "canonical ordered dictionary owner receiving",
                    )?;
                    let mut c = Canonicalizer::new(a, sink);
                    b.reserve_array::<(usize, u64)>(f, 1, "889 matched root")?;
                    if compact {
                        c.id_compact(f, vec![(root, weight)], &mut b)
                    } else {
                        c.id(f, vec![(root, weight)], &mut b)
                    }
                })()
                .map_err(failure);
                let row = shapes.finish(matches!(result, Ok(Some(_))));
                let successor = old_census::take();
                drop(shapes);
                drop(observer);
                (
                    result,
                    b.remaining,
                    b.skeleton_remaining,
                    b.skeleton_spent,
                    row,
                    successor,
                )
            });
            (result, remaining, skeleton, spent, heap, shapes, successors)
        });
    Run {
        result,
        remaining,
        skeleton,
        spent,
        paid,
        refused,
        heap,
        shapes,
        successors,
    }
}
#[test]
fn real_byte_result_layout_and_mixed_primitive_bank_delta_are_exact() {
    let [old_unit, new_unit, old_cold, new_cold, old_warm, new_warm, work, node] = result_banks();
    let args = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>();
    assert_eq!(old_unit, args + 2 * size_of::<Result<(), Diagnostic>>());
    assert_eq!(new_unit, args + 2 * size_of::<Result<(), ByteFailure>>());
    assert!(new_unit < old_unit);
    assert_eq!(work, old_unit);
    assert_eq!(
        node,
        2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, &str)>()
            + 2 * size_of::<Result<(), Diagnostic>>()
    );
    assert_eq!(old_cold - new_cold, 23 * (old_unit - new_unit));
    assert_eq!(old_warm, new_warm);
}
#[test]
fn zero_single_many_and_overflow_keep_complete_results_work_and_backing() {
    let f = function();
    for length in [0, 1, 4, 17] {
        for width in [0, 1, 2, 4] {
            for weight in [1, 2, u64::MAX] {
                let (a, sink, root) = graph(&f, length, width, false);
                let old = run(
                    false,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    64 * 1024 * 1024,
                    MAX_REPLAY_WORK_UNITS,
                    0,
                );
                let current = run(
                    true,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    64 * 1024 * 1024,
                    MAX_REPLAY_WORK_UNITS,
                    0,
                );
                same(&old, &current);
                assert_eq!(old.refused, current.refused);
                assert_eq!(old.heap, current.heap);
                if matches!(current.result, Ok(Some(_))) {
                    let m = |r: &Run| {
                        r.successors
                            .cells
                            .iter()
                            .flat_map(|m| m.iter())
                            .flat_map(|p| p.iter())
                            .map(|r| r.primitive_banks)
                            .sum::<usize>()
                    };
                    assert!(old.successors.complete && current.successors.complete);
                    assert_eq!(m(&old), m(&current));
                    let [wide, compact, ..] = result_banks();
                    assert_eq!(old.paid - current.paid, m(&current) * 23 * (wide - compact));
                }
            }
        }
    }
}
#[test]
fn every_small_work_cut_cycle_exact_short_zero_and_floor_refusal_release_owners() {
    let f = function();
    let (a, sink, root) = graph(&f, 2, 1, false);
    let full = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(matches!(full.result, Ok(Some(_))) && !full.refused);
    for work in 0..=MAX_REPLAY_WORK_UNITS - full.remaining {
        same(
            &run(false, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0),
            &run(true, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0),
        );
    }
    let exact = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        full.paid,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert_eq!(exact.result, full.result);
    assert!(!exact.refused);
    balanced(&exact);
    for (limit, floor) in [
        (0, 0),
        (full.paid - 1, 0),
        (64 * 1024 * 1024, 64 * 1024 * 1024),
    ] {
        let short = run(
            true,
            &f,
            &a,
            sink,
            root,
            1,
            limit,
            MAX_REPLAY_WORK_UNITS,
            floor,
        );
        assert!(short.result.is_err() && short.refused);
        balanced(&short);
    }
    let (a, sink, root) = graph(&f, 1, 1, true);
    let old = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    let current = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    same(&old, &current);
    assert!(current.result.is_err());
}
