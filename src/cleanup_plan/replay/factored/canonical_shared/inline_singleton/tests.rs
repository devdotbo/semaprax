//! Complete immediate-owner controls. All measurements remain authored/unrun.
use super::super::lexical_successors::census as old_census;
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::MAX_REPLAY_WORK_UNITS;
use crate::test_allocation_observer::{measure, Metrics};
use inline_exit::census as shape_census;
use std::mem::{align_of, size_of};
type OldCall<'a> = (
    &'a mut super::super::Canonicalizer<'a>,
    &'a ResolvedFunction,
    Weighted,
    &'a mut ReplayBudget,
);
type NewCall<'a> = (
    &'a mut Canonicalizer<'a>,
    &'a ResolvedFunction,
    Weighted,
    &'a mut ReplayBudget,
);
const OLD_CALL: usize = 2 * size_of::<OldCall<'static>>()
    + 2 * size_of::<Result<Option<usize>, Diagnostic>>()
    + 2 * size_of::<Weighted>();
const NEW_CALL: usize = 2 * size_of::<NewCall<'static>>()
    + 2 * size_of::<Result<Option<usize>, Diagnostic>>()
    + 2 * size_of::<Weighted>();
fn function() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
fn graph(
    f: &ResolvedFunction,
    length: usize,
    width: usize,
    cycle: bool,
) -> (Automaton, usize, usize) {
    let mut a = Automaton::default();
    let mut b = ReplayBudget::unobserved();
    let sink = a.node(f, &mut b).unwrap();
    let mut next = sink;
    for index in 0..length {
        let node = a.node(f, &mut b).unwrap();
        for branch in 0..width {
            let terminal = if branch % 2 == 0 {
                super::super::super::SkeletonTerminal::Success
            } else {
                super::super::super::SkeletonTerminal::Failure
            };
            a.movement(f, &mut b, node, Label::End(terminal), next)
                .unwrap();
        }
        if cycle && index == 0 {
            a.movement(
                f,
                &mut b,
                node,
                Label::End(super::super::super::SkeletonTerminal::Success),
                node,
            )
            .unwrap();
        }
        next = node;
    }
    (a, sink, next)
}
#[derive(Debug, PartialEq, Eq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 2048], usize),
    path: Option<([u8; 2048], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 2048], usize)>,
}
fn text(s: &str) -> ([u8; 2048], usize) {
    assert!(s.len() <= 2048);
    let mut bytes = [0; 2048];
    bytes[..s.len()].copy_from_slice(s.as_bytes());
    (bytes, s.len())
}
fn failure(d: Diagnostic) -> Failure {
    Failure {
        code: d.code,
        severity: d.severity,
        message: text(&d.message),
        path: d.path.as_deref().map(text),
        span: d.span,
        help: d.help.as_deref().map(text),
    }
}
#[derive(Debug)]
struct Run {
    result: Result<Option<usize>, Failure>,
    remaining: usize,
    skeleton: usize,
    spent: usize,
    paid: usize,
    refused: bool,
    heap: Metrics,
    shapes: shape_census::Row,
    successors: old_census::Report,
}
#[allow(
    clippy::too_many_arguments,
    reason = "Matched whole-owner controls retain the actual independent old/current calls."
)]
fn run(
    old: bool,
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
                    if old {
                        b.reserve_bytes(
                            f,
                            super::super::Canonicalizer::CONSTRUCTOR_CONTROLS + OLD_CALL,
                            "canonical ordered dictionary owner receiving",
                        )?;
                        let mut c = super::super::Canonicalizer::new(a, sink);
                        b.reserve_array::<(usize, u64)>(f, 1, "889 matched root")?;
                        c.id(f, vec![(root, weight)], &mut b)
                    } else {
                        b.reserve_bytes(
                            f,
                            Canonicalizer::CONSTRUCTOR_CONTROLS + NEW_CALL,
                            "canonical ordered dictionary owner receiving",
                        )?;
                        let mut c = Canonicalizer::new(a, sink);
                        b.reserve_array::<(usize, u64)>(f, 1, "889 matched root")?;
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
fn balanced(r: &Run) {
    assert!(!r.heap.invalid);
    assert_eq!(r.heap.live, 0);
    assert_eq!(r.heap.allocated, r.heap.freed);
}
fn same(old: &Run, current: &Run) {
    assert_eq!(current.result, old.result);
    assert_eq!(
        (current.remaining, current.skeleton, current.spent),
        (old.remaining, old.skeleton, old.spent)
    );
    balanced(old);
    balanced(current);
}
#[test]
fn actual_old_and_inline_frame_and_output_layouts_keep_the_original_capacity_invoice() {
    type OldOutput = (u64, Vec<(Label, SharedWeighted)>);
    type NewOutput = (u64, Label, SharedWeighted);
    assert_eq!(
        size_of::<CanonicalFrame>(),
        size_of::<super::super::CanonicalFrame>()
    );
    assert_eq!(
        align_of::<CanonicalFrame>(),
        align_of::<super::super::CanonicalFrame>()
    );
    assert!(size_of::<Option<NewOutput>>() <= size_of::<Option<OldOutput>>());
    assert_eq!(
        Canonicalizer::CONSTRUCTOR_CONTROLS,
        super::super::Canonicalizer::CONSTRUCTOR_CONTROLS
    );
    assert_eq!(OLD_CALL, NEW_CALL);
    eprintln!("OPT889 actual layouts frame old={} current={} align old={} current={} output old={} current={} fields={} query={} selectors={} root_primitives={}",size_of::<super::super::CanonicalFrame>(),size_of::<CanonicalFrame>(),align_of::<super::super::CanonicalFrame>(),align_of::<CanonicalFrame>(),size_of::<Option<OldOutput>>(),size_of::<Option<NewOutput>>(),inline_exit::FIELDS,inline_exit::RECEIVING,inline_exit::SELECT,inline_exit::PRIMITIVES);
}
#[test]
fn zero_one_many_and_repeated_labels_preserve_complete_immediate_ids_work_and_drop() {
    let f = function();
    for length in [0, 1, 4, 17] {
        for width in [0, 1, 2, 4] {
            for weight in [1, 2, u64::MAX] {
                let (a, sink, root) = graph(&f, length, width, false);
                let old = run(
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
                let current = run(
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
                same(&old, &current);
                assert_eq!(old.refused, current.refused);
                eprintln!("OPT889 shape length={length} width={width} weight={weight} old={} current={} new={:?}",old.paid,current.paid,current.shapes);
            }
        }
    }
}
#[test]
fn genuine_long_singleton_chain_has_positive_complete_typed_net_without_padding_old() {
    let f = function();
    let (a, sink, root) = graph(&f, 64, 1, false);
    let old = run(
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
    let current = run(
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
    same(&old, &current);
    assert!(current.result.is_ok() && !old.refused && !current.refused);
    assert!(current.shapes.complete && !current.shapes.overflowed);
    assert!(current.successors.complete && !current.successors.foreign_observation);
    let n = current.successors.cells[1]
        .iter()
        .flat_map(|phase| phase.iter())
        .map(|c| c.acquired)
        .sum::<usize>();
    let m = current.successors.cells[1]
        .iter()
        .flat_map(|phase| phase.iter())
        .map(|c| c.primitive_banks)
        .sum::<usize>();
    let one = current.shapes.arity[1];
    assert!(one >= 64);
    let old_per = super::super::Canonicalizer::BORROWED_EXIT_WARM;
    let old_first = super::super::Canonicalizer::BORROWED_EXIT_PRIMITIVE;
    let extra_cold = lexical_successors::ONE_PASS_UNIT_ABI;
    let drop_bytes =
        current.shapes.unit_drops * 2 * size_of::<Rc<ordered_weights::OrderedWeights>>()
            + current.shapes.general_drops * 2 * size_of::<ordered_weights::OrderedWeights>();
    // The two genuine complete Frame types have equal width, and the identical
    // push/pop order keeps every cumulative replacement capacity invoice equal.
    let expected = one as i128
        * (old_per as i128 - inline_exit::RECEIVING as i128 - inline_exit::FIELDS as i128
            + size_of::<(Label, SharedWeighted)>() as i128)
        + old_first as i128
        - inline_exit::PRIMITIVES as i128
        - m as i128 * extra_cold as i128
        - n as i128 * inline_exit::SELECT as i128
        - drop_bytes as i128;
    assert_eq!(old.paid as i128 - current.paid as i128, expected);
    assert!(
        expected > 0 && current.paid < old.paid,
        "no adoption from gross Vec savings: {current:?}"
    );
    assert!(
        current.heap.allocations < old.heap.allocations,
        "actual removed singleton backing, not a fee-only change"
    );
    eprintln!("OPT889 complete old={old:?} current={current:?} N={n} M={m} actual_net={expected}; no application result inferred");
}
#[test]
fn every_small_work_cut_and_cycle_keep_six_fields_before_later_receiving() {
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
    let used = MAX_REPLAY_WORK_UNITS - full.remaining;
    for work in 0..=used {
        let old = run(true, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0);
        let current = run(false, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0);
        same(&old, &current);
    }
    let (a, sink, root) = graph(&f, 1, 1, true);
    let old = run(
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
    let current = run(
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
    same(&old, &current);
    assert!(current.result.is_err());
}
#[test]
fn actual_inline_owner_exact_short_zero_and_floor_refusals_release_every_owner() {
    let f = function();
    let (a, sink, root) = graph(&f, 4, 1, false);
    let full = run(
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
    assert!(matches!(full.result, Ok(Some(_))));
    let exact = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        full.paid,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(exact.result.is_ok() && !exact.refused);
    balanced(&exact);
    for limit in [0, full.paid - 1] {
        let short = run(
            false,
            &f,
            &a,
            sink,
            root,
            1,
            limit,
            MAX_REPLAY_WORK_UNITS,
            0,
        );
        assert!(short.result.is_err() && short.refused);
        balanced(&short);
        assert!(!short.shapes.complete);
    }
    let floor = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        64 * 1024 * 1024,
    );
    assert!(floor.result.is_err() && floor.refused);
    balanced(&floor);
}
