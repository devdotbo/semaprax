//! Immediate d458 suffix-only comparator versus certified head and right split.
use super::*;
fn heads_run(
    function: &ResolvedFunction,
    length: usize,
    depth: usize,
    shared: bool,
    fault: usize,
    old: bool,
    units: usize,
    limit: usize,
    floor: usize,
    hir_shared: bool,
    swaps_only: bool,
    shape: usize,
) -> Run {
    let (mut actual, mut expected) = fixture(length, depth, shared, fault);
    if shape == 1 {
        let values = actual.arena[0].items.clone();
        let split = length / 2;
        let prefix = actual.arena.len();
        actual.arena.push(Segment {
            previous: None,
            items: values[..split].to_vec(),
            depth: 1,
        });
        let suffix = actual.arena.len();
        actual.arena.push(Segment {
            previous: Some(prefix),
            items: values[split..].to_vec(),
            depth: 2,
        });
        actual.paths[1].history.previous = Some(suffix);
        actual.paths[3].history.previous = Some(suffix);
        actual.max_depth = 2;
    } else if shape == 2 {
        let empty = actual.arena.len();
        actual.arena.push(Segment {
            previous: Some(0),
            items: Vec::new(),
            depth: depth + 1,
        });
        actual.paths[1].history.previous = Some(empty);
        actual.paths[3].history.previous = Some(empty);
        actual.max_depth = depth + 1;
    }
    if shape == 3 {
        let empty = actual.arena.len();
        actual.arena.push(Segment {
            previous: None,
            items: Vec::new(),
            depth: 1,
        });
        let values = actual.arena[0].items.clone();
        let next = actual.arena.len();
        actual.arena.push(Segment {
            previous: Some(empty),
            items: values,
            depth: 2,
        });
        actual.paths[1].history.previous = Some(next);
        actual.paths[3].history.previous = Some(next);
        actual.max_depth = 2;
    }
    if shape == 4 {
        let values = actual.arena[0].items.clone();
        actual.arena.clear();
        for (index, value) in values.into_iter().enumerate() {
            actual.arena.push(Segment {
                previous: index.checked_sub(1),
                items: vec![value],
                depth: index + 1,
            });
        }
        if actual.arena.is_empty() {
            actual.arena.push(Segment {
                previous: None,
                items: Vec::new(),
                depth: 1,
            });
        }
        for path in &mut actual.paths {
            path.history.previous = Some(actual.arena.len() - 1);
        }
        actual.max_depth = actual.arena.len();
    }
    if shape == 5 {
        // Same actual observations represented by a right tail, so a fresh
        // left segment reaches the right's exhausted-segment envelope.
        actual.paths.truncate(2);
        let common = actual.arena[0].items.clone();
        let mut tail = common.clone();
        tail.extend(actual.paths[1].history.tail.iter().cloned());
        actual.paths[1].history.previous = None;
        actual.paths[1].history.tail = tail;
        expected.clear();
        for (index, path) in actual.paths.iter().enumerate() {
            let mut observations = if index == 0 {
                common.clone()
            } else {
                Vec::new()
            };
            observations.extend(path.history.tail.iter().cloned());
            expected.push(SkeletonPath {
                observations: observations.into(),
                terminal: path.terminal,
            });
        }
        expected.sort();
    }
    if hir_shared {
        for path in &mut expected {
            path.observations.prefix = path.observations.freeze();
            path.observations.items.clear();
        }
    }
    pair_scope_census::reset();
    if swaps_only {
        frontiers::reset_swaps();
    } else {
        frontiers::reset();
    }
    let observed = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            budget.remaining = units;
            assert!(crate::bounded_output::set_active_floor(floor));
            let result = (|| {
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "899 complete comparison receiver"
                )?;
                if old {
                    before_certified_heads::compare(function, &mut budget, actual, &expected)
                } else {
                    compare(function, &mut budget, actual, &expected)
                }
            })();
            let saved = result.as_ref().map(|_| ()).map_err(failure);
            let retry = if result.is_err()
                && (budget.refusal_spent || crate::bounded_output::active_refused())
            {
                crate::bounded_output::clear_active_floor();
                let before = (budget.remaining, crate::bounded_output::active_remaining());
                let retry = budget
                    .reserve_bytes(function, 0, "899 sticky zero")
                    .unwrap_err();
                assert_eq!(
                    (budget.remaining, crate::bounded_output::active_remaining()),
                    before
                );
                Some(retry)
            } else {
                None
            };
            let outcome = Outcome {
                result: saved,
                retry: retry.as_ref().map(failure),
                remaining: budget.remaining,
                skeleton_spent: budget.skeleton_spent,
            };
            crate::bounded_output::clear_active_floor();
            drop(result);
            drop(retry);
            drop(budget);
            outcome
        })
    });
    let events = frontiers::finish();
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    // The original call frame can refuse before begin; that is genuinely no row.
    let row = if rows.is_empty() {
        assert!(observed.1);
        assert!(observed.0.0.result.is_err());
        pair_scope_census::empty_for_test()
    } else {
        assert_eq!(rows.len(), 1);
        assert!(rows[0].ended && !rows[0].overflow && !rows[0].truncated && !rows[0].unwound);
        rows[0]
    };
    balanced(observed.0.1);
    Run {
        observed,
        events,
        row,
    }
}
// This is the genuine immediate suffix-only generation, not a hypothetical
// first-head-only stage. Every coefficient names an actual old or new packet.
fn invoice(r: &pair_scope_census::Row) -> (usize, usize) {
    let header = 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    let matching = 2 * size_of::<&SkeletonObservation>()
        + 2 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonTerminal>()
        + 2 * size_of::<Ordering>();
    assert_eq!((header, matching), (PAYMENT_HEADER, PAIR_MATCH));
    let base =
        2 * size_of::<usize>() + 3 * size_of::<bool>() + 2 * size_of::<usize>() + size_of::<bool>();
    let pop = 2 * size_of::<&mut Vec<usize>>()
        + size_of::<Option<usize>>()
        + size_of::<usize>()
        + 2 * size_of::<bool>();
    let head = size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 2 * size_of::<&Rc<SkeletonObservation>>()
        + size_of::<&SkeletonObservation>()
        + size_of::<usize>();
    let view = 2 * size_of::<&[Segment]>()
        + 2 * size_of::<usize>()
        + size_of::<&Segment>()
        + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + head;
    let rhs = size_of::<&[Rc<SkeletonObservation>]>()
        + size_of::<usize>()
        + 2 * size_of::<&Rc<SkeletonObservation>>()
        + size_of::<&SkeletonObservation>();
    let first = 2 * size_of::<&SkeletonObservation>() + size_of::<Ordering>();
    let old_step = 2 * size_of::<usize>()
        + 3 * size_of::<bool>()
        + size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 2 * size_of::<&Rc<SkeletonObservation>>()
        + size_of::<&SkeletonObservation>()
        + size_of::<usize>()
        + 2 * size_of::<usize>()
        + size_of::<bool>();
    let old_load = 2 * size_of::<&mut Vec<usize>>()
        + size_of::<Option<usize>>()
        + size_of::<usize>()
        + size_of::<bool>()
        + 2 * size_of::<&[Segment]>()
        + 2 * size_of::<usize>()
        + size_of::<&Segment>()
        + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + size_of::<bool>();
    assert_eq!(
        (base, pop, head, view, rhs, first),
        (
            certified_heads::BASE,
            certified_heads::POP,
            certified_heads::HEAD,
            certified_heads::VIEW_HEAD,
            certified_heads::RHS,
            certified_heads::FIRST
        )
    );
    assert_eq!(
        (old_step, old_load),
        (paired_value::STEP + paired_value::EMPTY, paired_value::LOAD)
    );
    assert_eq!(
        base + head,
        old_step,
        "loaded and exhausted noncertified right envelopes retain the whole old STEP"
    );
    assert_eq!(
        base + pop + view,
        old_step + old_load,
        "foreign right views retain the complete old STEP and LOAD, including empty segments"
    );
    (
        8 * header
            + base * r.certified_bases
            + pop * r.certified_pops
            + head * r.certified_heads
            + view * r.certified_views
            + rhs * r.certified_rhs_assignments
            + first * r.certified_firsts,
        old_step * r.certified_bases + old_load * r.certified_pops + matching * r.certified_firsts,
    )
}
fn observe_heads(
    function: &ResolvedFunction,
    length: usize,
    depth: usize,
    shared: bool,
    shape: usize,
    old: bool,
    units: usize,
    trace: bool,
) -> Run {
    heads_run(
        function,
        length,
        depth,
        shared,
        0,
        old,
        units,
        usize::MAX,
        0,
        false,
        !trace,
        shape,
    )
}
#[test]
fn certified_heads_keep_complete_immediate_invoices_and_short_losses_for_both_hir_representations()
{
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    assert_eq!(
        size_of::<ComparisonOwner<'static>>(),
        size_of::<before_certified_heads::ComparisonOwner<'static>>()
    );
    for length in [0, 1, 2, 8, 128] {
        for (depth, shared, shape) in [
            (1, true, 0),
            (1, false, 0),
            (8, true, 0),
            (1, true, 1),
            (1, true, 2),
            (1, true, 3),
            (1, true, 4),
            (1, true, 5),
        ] {
            for hir_shared in [false, true] {
                let old = heads_run(
                    function,
                    length,
                    depth,
                    shared,
                    0,
                    true,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    hir_shared,
                    true,
                    shape,
                );
                let current = heads_run(
                    function,
                    length,
                    depth,
                    shared,
                    0,
                    false,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    hir_shared,
                    true,
                    shape,
                );
                assert_eq!(old.observed.0.0, current.observed.0.0);
                assert!(current.observed.0.0.result.is_ok());
                assert_eq!(old.observed.0.1, current.observed.0.1);
                assert_eq!(swaps(&old), swaps(&current));
                assert!(old.row.complete && current.row.complete);
                assert_eq!(
                    old.row.identity_observations,
                    current.row.identity_observations
                );
                assert_eq!(old.row.identity_probes, current.row.identity_probes);
                assert_eq!(old.row.identity_bodies, current.row.identity_bodies);
                assert_eq!(old.row.identity_hits, current.row.identity_hits);
                assert_eq!(
                    old.row.value_steps,
                    current.row.value_steps + current.row.certified_bases
                );
                assert_eq!(
                    old.row.loads,
                    current.row.loads + current.row.certified_pops
                );
                assert_eq!(
                    old.row.pair_matches_admitted,
                    current.row.pair_matches_admitted + current.row.certified_firsts
                );
                let (new, removed) = invoice(&current.row);
                assert_eq!(
                    current.observed.2
                        + removed
                        + old.row.identity_detectors * shared_prefix::DETECTOR,
                    old.observed.2 + new + current.row.identity_detectors * shared_prefix::DETECTOR
                );
                if shape == 3 && length > 0 {
                    assert!(
                        current.row.identity_detectors > old.row.identity_detectors,
                        "foreign empty right segments must pay their additional Q"
                    );
                }
                if length == 0 {
                    assert_eq!(current.row.certified_bases, 0);
                    assert_eq!(current.observed.2, old.observed.2 + 8 * PAYMENT_HEADER);
                }
                if shape == 4 && length == 128 {
                    assert!(
                        current.row.certified_firsts > 128,
                        "many real segment first heads, not padded root controls"
                    );
                    assert!(
                        current.observed.2 < old.observed.2,
                        "full genuine d458 invoice including all eight cold headers must win on this dense witness"
                    );
                }
                if !shared {
                    assert_eq!(current.row.certified_firsts, 0);
                }
                eprintln!(
                    "OPT900902 length={length} depth={depth} shared={shared} shape={shape} hir_shared={hir_shared} old_d458={} current={} cold={} base={} pop={} view={} head={} rhs={} first={} old_Q={} current_Q={} G={} H={}; full individual work and Drop; source candidate only, no shipping gain claim",
                    old.observed.2,
                    current.observed.2,
                    8 * PAYMENT_HEADER,
                    current.row.certified_bases,
                    current.row.certified_pops,
                    current.row.certified_views,
                    current.row.certified_heads,
                    current.row.certified_rhs_assignments,
                    current.row.certified_firsts,
                    old.row.identity_detectors,
                    current.row.identity_detectors,
                    current.row.identity_observations,
                    current.row.identity_hits
                );
            }
        }
    }
}
#[test]
fn certified_heads_every_work_cut_retains_all_six_fields_swaps_and_malformed_preparation() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    for (length, depth, shared, shape) in [
        (0, 1, true, 0),
        (1, 1, true, 0),
        (2, 1, false, 0),
        (8, 8, true, 0),
        (8, 1, true, 1),
        (8, 1, true, 2),
        (8, 1, true, 3),
        (8, 1, true, 5),
    ] {
        for fault in 0..9 {
            for hir_shared in [false, true] {
                let complete = heads_run(
                    function,
                    length,
                    depth,
                    shared,
                    fault,
                    true,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    hir_shared,
                    true,
                    shape,
                );
                for units in 0..=complete.observed.0.0.skeleton_spent + 1 {
                    let old = heads_run(
                        function,
                        length,
                        depth,
                        shared,
                        fault,
                        true,
                        units,
                        usize::MAX,
                        0,
                        hir_shared,
                        true,
                        shape,
                    );
                    let current = heads_run(
                        function,
                        length,
                        depth,
                        shared,
                        fault,
                        false,
                        units,
                        usize::MAX,
                        0,
                        hir_shared,
                        true,
                        shape,
                    );
                    assert_eq!(
                        old.observed.0.0, current.observed.0.0,
                        "length={length} depth={depth} shared={shared} shape={shape} fault={fault} HIR={hir_shared} cut={units}"
                    );
                    assert_eq!(swaps(&old), swaps(&current));
                }
            }
        }
    }
}
#[test]
fn certified_heads_every_new_packet_has_exact_short_zero_floor_sticky_and_drop_controls() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    let mut reached = [false; 9];
    let phases = [
        "ordinary plan fresh right segment base",
        "ordinary plan fresh right link pop",
        "ordinary plan certified right slice assignment",
        "ordinary plan certified first observation",
        "ordinary plan foreign right segment view and head",
        "ordinary plan loaded right head",
        "ordinary plan exhausted right step retained envelope",
        "ordinary plan fresh segment identity event",
        "ordinary plan history paired cursor next",
    ];
    for (shared, shape) in [
        (true, 0),
        (false, 0),
        (true, 1),
        (true, 2),
        (true, 3),
        (true, 5),
    ] {
        let complete = observe_heads(
            function,
            8,
            1,
            shared,
            shape,
            false,
            MAX_REPLAY_WORK_UNITS,
            true,
        );
        assert!(complete.observed.0.0.result.is_ok());
        let total = complete.observed.2;
        for physical in [0, total - 1, total] {
            let r = heads_run(
                function,
                8,
                1,
                shared,
                0,
                false,
                MAX_REPLAY_WORK_UNITS,
                physical,
                0,
                false,
                false,
                shape,
            );
            assert_eq!(r.observed.0.0.result.is_ok(), physical == total);
            assert_eq!(r.observed.1, physical != total);
            if physical != total {
                assert!(r.observed.0.0.retry.is_some());
            }
        }
        for event in &complete.events {
            let Some(index) = phases.iter().position(|p| *p == event.phase) else {
                continue;
            };
            reached[index] = true;
            let after = event.after.unwrap();
            for (limit, floor) in [(after - 1, 0), (after, 1)] {
                let r = heads_run(
                    function,
                    8,
                    1,
                    shared,
                    0,
                    false,
                    MAX_REPLAY_WORK_UNITS,
                    limit,
                    floor,
                    false,
                    false,
                    shape,
                );
                assert!(r.observed.1 && r.observed.0.0.result.is_err());
                assert!(r.observed.0.0.retry.is_some());
                assert_eq!(
                    r.observed.0.0.result.as_ref().unwrap_err(),
                    &primitive_failure(function, event, event.before, floor)
                );
            }
        }
    }
    assert_eq!(
        reached, [true; 9],
        "all actual packet frontiers must have a genuine witness"
    );
}
#[test]
fn certified_heads_refused_prefix_distinguishes_admitted_first_banks_from_completed_work() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    let complete = observe_heads(function, 2, 1, true, 0, false, MAX_REPLAY_WORK_UNITS, true);
    let mut first_before_work = false;
    for units in 0..=complete.observed.0.0.skeleton_spent {
        let r = observe_heads(function, 2, 1, true, 0, false, units, true);
        let admitted = |phase| {
            r.events
                .iter()
                .filter(|e| {
                    e.phase == phase
                        || (phase == "ordinary plan loaded right head"
                            && e.phase == "ordinary plan exhausted right step retained envelope")
                })
                .filter_map(|e| e.after.map(|after| after - e.before))
                .sum::<usize>()
        };
        for (phase, count, width) in [
            (
                "ordinary plan fresh right segment base",
                r.row.certified_bases,
                certified_heads::BASE,
            ),
            (
                "ordinary plan fresh right link pop",
                r.row.certified_pops,
                certified_heads::POP,
            ),
            (
                "ordinary plan certified right slice assignment",
                r.row.certified_rhs_assignments,
                certified_heads::RHS,
            ),
            (
                "ordinary plan certified first observation",
                r.row.certified_firsts,
                certified_heads::FIRST,
            ),
            (
                "ordinary plan foreign right segment view and head",
                r.row.certified_views,
                certified_heads::VIEW_HEAD,
            ),
            (
                "ordinary plan loaded right head",
                r.row.certified_heads,
                certified_heads::HEAD,
            ),
        ] {
            assert_eq!(admitted(phase), count * width);
        }
        assert!(r.row.certified_firsts >= r.row.identity_hits);
        if r.row.certified_firsts > r.row.identity_hits {
            assert!(!r.row.complete && r.observed.0.0.result.is_err());
            first_before_work = true;
        }
    }
    assert!(
        first_before_work,
        "a paid first certificate can precede the original work refusal"
    );
}

#[test]
fn certified_heads_keep_whole_d458_owner_macros_and_independent_final_hir_loops() {
    use sha2::{Digest, Sha256};
    let mut restored = before_certified_heads::whole_source().to_owned();
    for literal in [
        "#[path = \"lexical_cursor_before_certified.rs\"]\n",
        "#[path = \"paired_value_before_certified.rs\"]\n",
        "#[path = \"shared_prefix_before_certified.rs\"]\n",
    ] {
        assert_eq!(restored.matches(literal).count(), 1);
        restored = restored.replacen(literal, "", 1);
    }
    assert_eq!(
        restored
            .matches("pub(super) struct ComparisonOwner<")
            .count(),
        1
    );
    restored = restored.replacen(
        "pub(super) struct ComparisonOwner<",
        "struct ComparisonOwner<",
        1,
    );
    assert_eq!(
        restored
            .matches("pair_scope_census::begin(function, budget, true)")
            .count(),
        1
    );
    restored = restored.replacen(
        "pair_scope_census::begin(function, budget, true)",
        "pair_scope_census::begin(function, budget, false)",
        1,
    );
    let helper = "\n#[cfg(test)]\npub(super) fn whole_source() -> &'static str {\n    include_str!(\"before_certified_heads.rs\")\n}\n";
    assert!(restored.ends_with(helper));
    restored.truncate(restored.len() - helper.len());
    assert_eq!(
        restored
            .matches("use super::pair_scope_census;\n\n")
            .count(),
        1
    );
    restored = restored.replacen("use super::pair_scope_census;\n\n", "#[cfg(test)]\nmod before_event_prefix;\n#[cfg(test)]\nmod before_shared_steps;\n#[cfg(test)]\n#[allow(unused_imports, reason = \"complete frozen generation retains its exact original namespace imports\")]\n#[rustfmt::skip]\nmod before_reachable_primitives;\n#[cfg(test)]\nmod before_flat_view;\n#[cfg(test)]\nmod before_fused_flat;\n#[cfg(test)]\nmod before_identity_prefix;\n#[cfg(test)]\nmod before_lexical_cursor;\n#[cfg(test)]\nmod before_pair_scope;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_prepared_pairing;\n#[cfg(test)]\nmod before_retained_tail;\n#[cfg(test)]\npub(super) mod pair_scope_census;\n\n", 1);
    restored.push_str("\n#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\nfn reachable_stage(path: &str, sha: &str, current: &str) -> String {\n    crate::source_reflows_merge::main_e177_final_inputs::value_direct(path, sha, current)\n}\n");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(&Sha256::digest(restored.as_bytes()))
        ),
        "e481d52c0b40ae23fedd04877e6e2756669576688fabd745fbaf35a5928f76d4"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(&Sha256::digest(include_bytes!(
                "lexical_cursor_before_certified.rs"
            )))
        ),
        "c63b8f98af8b1fa55f7e305819efc87107477db4a92f57743365544885b4973c"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(&Sha256::digest(include_bytes!(
                "paired_value_before_certified.rs"
            )))
        ),
        "f68af76308eb729dd805fd1453d23e738fa6cf3aa4ab9bbbe9518d72323c341e"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(&Sha256::digest(include_bytes!(
                "shared_prefix_before_certified.rs"
            )))
        ),
        "375d7cfdbb9ea4c2024d2b56a453e9e9cc7a57648057af869f7050a121e072ee"
    );
    let current = include_str!("../comparison_owner.rs");
    let final_marker = "    let mut pairs = actual.paths.iter().zip(expected);";
    let old_final = restored
        .split_once(final_marker)
        .unwrap()
        .1
        .split_once("    #[cfg(test)]\n    census.success();")
        .unwrap()
        .0;
    let new_final = current
        .split_once(final_marker)
        .unwrap()
        .1
        .split_once("    #[cfg(test)]\n    census.success();")
        .unwrap()
        .0;
    assert_eq!(
        old_final, new_final,
        "full independent ordinary final HIR comparison remains byte exact"
    );
}

#[test]
fn certified_heads_real_oracle_missing_foreign_restored_scope_keeps_ambient_unspent() {
    let program = super::super::super::super::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    let foreign = function.clone();
    let expected = expected(&program, function);
    for state in 0..3 {
        let run = |old: bool| {
            let actual = shaped_883(&expected, 4, true, 0);
            let (status, refused, active_paid, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    let scope =
                        allocation::OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
                    let invocation = match state {
                        0 => Some(allocation::enter(&program, function, scope.lease()).unwrap()),
                        1 => None,
                        _ => Some(allocation::enter(&program, &foreign, scope.lease()).unwrap()),
                    };
                    let mut budget = ReplayBudget::with_skeleton_limit_oracle(
                        MAX_REPLAY_WORK_UNITS,
                        scope.lease(),
                    );
                    let before = crate::bounded_output::active_remaining();
                    // The real caller authenticates typed Oracle authority before
                    // entering either private comparator. Neither comparator mints
                    // or rechecks a program/function lease by itself.
                    let result =
                        match SkeletonWork::with_index(&program, function, &mut budget, None) {
                            Err(error) => {
                                drop(actual);
                                Err(error)
                            }
                            Ok(work) => {
                                drop(work);
                                match budget.reserve_bytes(
                                    function,
                                    COMPARE_CALL_CONTROLS,
                                    "883 typed oracle comparison call",
                                ) {
                                    Err(error) => {
                                        drop(actual);
                                        Err(error)
                                    }
                                    Ok(()) => {
                                        if old {
                                            before_certified_heads::compare(
                                                function,
                                                &mut budget,
                                                actual,
                                                &expected,
                                            )
                                        } else {
                                            compare(function, &mut budget, actual, &expected)
                                        }
                                    }
                                }
                            }
                        };
                    let status = result.as_ref().map(|_| ()).map_err(failure);
                    let work = (
                        budget.remaining,
                        budget.skeleton_remaining,
                        budget.skeleton_spent,
                    );
                    let receipt = scope.receipt();
                    if state == 0 {
                        assert!(status.is_ok());
                    } else {
                        assert_eq!(result, Err(Diagnostic::io("SPX-H006", String::new())));
                        assert_eq!((receipt.charged, receipt.work), (0, 0));
                        assert!(receipt.physical_failure.is_some());
                        let error = budget
                            .reserve_bytes(function, 0, "883 foreign oracle sticky")
                            .unwrap_err();
                        assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
                        assert_eq!((scope.receipt().charged, scope.receipt().work), (0, 0));
                        drop(invocation);
                        let restored =
                            allocation::enter(&program, function, scope.lease()).unwrap();
                        let retry = budget
                            .reserve_bytes(function, 0, "883 restored oracle sticky")
                            .unwrap_err();
                        assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                        assert_eq!((scope.receipt().charged, scope.receipt().work), (0, 0));
                        drop(restored);
                    }
                    assert_eq!(crate::bounded_output::active_remaining(), before);
                    drop(result);
                    drop(budget);
                    (status, work, receipt.work)
                });
            assert!(!refused && first.is_none());
            assert_eq!(
                active_paid, 0,
                "explicit Oracle never falls back to ambient payer"
            );
            status
        };
        assert_eq!(
            run(false),
            run(true),
            "typed same fixture, all six fields and exact real work"
        );
    }
}

#[test]
fn certified_heads_both_complete_generations_unwind_with_full_drop_and_reached_prefix() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    for old in [true, false] {
        let (actual, expected) = fixture(16, 1, true, 0);
        pair_scope_census::reset();
        frontiers::reset();
        pair_scope_census::unwind_next_identity_observation();
        let ((unwound, metrics), refused, _, receipt) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        paid!(
                            &mut budget,
                            function,
                            COMPARE_CALL_CONTROLS,
                            "899 unwind receiver"
                        )
                        .unwrap();
                        if old {
                            before_certified_heads::compare(
                                function,
                                &mut budget,
                                actual,
                                &expected,
                            )
                        } else {
                            compare(function, &mut budget, actual, &expected)
                        }
                        .unwrap();
                    }));
                    let unwound = outcome.is_err();
                    drop(outcome);
                    unwound
                })
            });
        assert!(unwound && !refused && receipt.is_none());
        balanced(metrics);
        frontiers::finish();
        let (rows, lost) = pair_scope_census::finish();
        assert_eq!(lost, 0);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].ended && rows[0].unwound && !rows[0].complete);
        assert_eq!(rows[0].identity_hits, 1);
        assert_eq!(rows[0].identity_observations, 1);
        assert_eq!(rows[0].identity_work, 4);
    }
}

#[test]
fn certified_heads_versioned_mixed_limits_expose_work_before_removed_physical_match() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    let success = observe_heads(function, 2, 1, true, 0, true, MAX_REPLAY_WORK_UNITS, true);
    let matching = success
        .events
        .iter()
        .find(|e| {
            e.phase == "ordinary plan history paired cursor next"
                && e.after == Some(e.before + PAIR_MATCH)
        })
        .unwrap();
    let mut changed_priority = false;
    for units in 0..=success.observed.0.0.skeleton_spent {
        let old = heads_run(
            function,
            2,
            1,
            true,
            0,
            true,
            units,
            matching.before + PAIR_MATCH - 1,
            0,
            false,
            false,
            0,
        );
        let current = heads_run(
            function,
            2,
            1,
            true,
            0,
            false,
            units,
            matching.before + 8 * PAYMENT_HEADER + PAIR_MATCH - 1,
            0,
            false,
            false,
            0,
        );
        if old.observed.1 && !current.observed.1 && current.row.pairs == 1 {
            assert_eq!(
                old.observed.0.0.result.as_ref().unwrap_err(),
                &primitive_failure(function, matching, matching.before, 0)
            );
            assert!(current.observed.0.0.result.is_err());
            assert_eq!(current.observed.0.0.remaining, 0);
            assert!(!current.row.complete);
            changed_priority = true;
            break;
        }
    }
    assert!(
        changed_priority,
        "new v2 order must not restore the absent old MATCH fee to hide its mixed-limit consequence"
    );
    let current_success =
        observe_heads(function, 2, 1, true, 0, false, MAX_REPLAY_WORK_UNITS, true);
    let base = current_success
        .events
        .iter()
        .find(|e| e.phase == "ordinary plan fresh right segment base")
        .unwrap();
    let right_step = success
        .events
        .iter()
        .filter(|e| {
            e.phase == "ordinary plan history paired cursor next"
                && e.after == Some(e.before + paired_value::STEP + paired_value::EMPTY)
        })
        .nth(1)
        .unwrap();
    let mut split_priority = false;
    for units in 0..=success.observed.0.0.skeleton_spent {
        let old = heads_run(
            function,
            2,
            1,
            true,
            0,
            true,
            units,
            right_step.before + certified_heads::BASE,
            0,
            false,
            false,
            0,
        );
        let current = heads_run(
            function,
            2,
            1,
            true,
            0,
            false,
            units,
            base.before + certified_heads::BASE,
            0,
            false,
            false,
            0,
        );
        if old.observed.1 && !current.observed.1 && current.row.certified_bases == 1 {
            assert_eq!(
                old.observed.0.0.result.as_ref().unwrap_err(),
                &primitive_failure(function, right_step, right_step.before, 0)
            );
            assert!(current.observed.0.0.result.is_err());
            assert_eq!(current.observed.0.0.remaining, 0);
            assert_eq!(current.row.certified_pops, 0);
            split_priority = true;
            break;
        }
    }
    assert!(
        split_priority,
        "BASE precedes the old right segment work; POP is admitted only after it"
    );
}

#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn certified_heads_opt_in_counts_every_real_new_packet_after_complete_teardown() {
    use crate::workflow_profile::plan_comparison::{Binding, Capture, Site};
    use sha2::{Digest, Sha256};
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    let capture = Capture::begin(Binding::caller_labelled(
        "900902 actual private comparator",
        Sha256::digest(include_bytes!("certified_heads_tests.rs")).into(),
    ))
    .unwrap();
    let r = observe_heads(function, 8, 1, true, 0, false, MAX_REPLAY_WORK_UNITS, false);
    let report = capture.finish().unwrap();
    assert!(r.row.complete && r.observed.0.0.result.is_ok());
    assert!(
        report.counts_complete
            && report.detail_complete
            && !report.overflow
            && report.row_count == 1
    );
    for (site, count, amount) in [
        (
            Site::CertifiedBaseAdmitted,
            r.row.certified_bases,
            certified_heads::BASE,
        ),
        (
            Site::CertifiedPopAdmitted,
            r.row.certified_pops,
            certified_heads::POP,
        ),
        (
            Site::CertifiedRhsAdmitted,
            r.row.certified_rhs_assignments,
            certified_heads::RHS,
        ),
        (
            Site::CertifiedViewAdmitted,
            r.row.certified_views,
            certified_heads::VIEW_HEAD,
        ),
        (
            Site::CertifiedHeadAdmitted,
            r.row.certified_heads,
            certified_heads::HEAD,
        ),
        (
            Site::CertifiedFirstAdmitted,
            r.row.certified_firsts,
            certified_heads::FIRST,
        ),
    ] {
        assert_eq!(report.counts.at(site), count as u64);
        assert_eq!(
            report.counts.admitted_bytes[site as usize],
            (count * amount) as u64
        );
    }
}
