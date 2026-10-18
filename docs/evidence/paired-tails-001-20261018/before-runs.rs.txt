//! Exact immutable-run successor; complete older guards remain independently executable.
fn restore(current: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(current.len(), length, "unknown shared-step subject extent");
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, before) in edits {
        let end = at
            .checked_add(after.len())
            .expect("shared step inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(&result[at..end], after, "unknown shared step source span");
        result.replace_range(at..end, before);
        last = at;
    }
    result
}
pub(super) fn previous_macro(current: &str) -> String {
    let current = super::super::super::paired_links::previous_macro(current);
    let current = current.as_str();
    restore(current, 17993, MACRO)
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (15443, "\n// Both retained views name the same immutable segment at the same offset.\n// Replay every original work request, but form only numeric loop carriers.\n// Private borrowed views/item positions publish after the whole run. An error\n// returns immediately: these non-owning fields cannot affect a Diagnostic,\n// owned Drop, Rc count or any externally visible state before that return.\nmacro_rules! shared_plan_segment_run {\n    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        paid!(\n            $budget,\n            $function,\n            2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>(),\n            $phase\n        )?;\n        #[cfg(test)]\n        $census.shared_runs();\n        let shared_length = $owner.left_items.len();\n        let mut shared_remaining = shared_length;\n        loop {\n            // Full condition/decrement envelope, including the zero sentinel.\n            paid!(\n                $budget,\n                $function,\n                4 * size_of::<usize>() + size_of::<bool>(),\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.shared_turns();\n            if shared_remaining == 0 {\n                break;\n            }\n            #[cfg(test)]\n            $census.shared_steps();\n            $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n            #[cfg(test)]\n            $census.step();\n            $budget.charge(\n                $function,\n                1,\n                \"ordinary plan history forward segment traversal\",\n            )?;\n            $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n            #[cfg(test)]\n            $census.step();\n            $budget.charge(\n                $function,\n                1,\n                \"ordinary plan history forward segment traversal\",\n            )?;\n            $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n            #[cfg(test)]\n            $census.round();\n            shared_remaining -= 1;\n        }\n        paid!(\n            $budget,\n            $function,\n            4 * size_of::<&[Rc<SkeletonObservation>]>()\n                + 4 * size_of::<usize>()\n                + size_of::<Ordering>(),\n            $phase\n        )?;\n        $owner.left_items = &[];\n        $owner.right_items = &[];\n        $owner.left_cursor.item += shared_length;\n        $owner.right_cursor.item += shared_length;\n        $owner.ordering = Ordering::Equal;\n        continue;\n    };\n}\n", ""),
    (8834, "                    shared_plan_segment_run!($owner, $census, $function, $budget, $phase);\n", "                    paid!(\n                        $budget,\n                        $function,\n                        3 * size_of::<&[Rc<SkeletonObservation>]>()\n                            + 2 * size_of::<usize>()\n                            + size_of::<Ordering>()\n                            + size_of::<bool>(),\n                        $phase\n                    )?;\n                    #[cfg(test)]\n                    $census.shared_steps();\n                    let [_, right_rest @ ..] = $owner.right_items else {\n                        unreachable!(\"identical immutable ordinary plan segment and offset\");\n                    };\n                    $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                    #[cfg(test)]\n                    $census.step();\n                    $budget.charge(\n                        $function,\n                        1,\n                        \"ordinary plan history forward segment traversal\",\n                    )?;\n                    $owner.left_items = left_rest;\n                    $owner.left_cursor.item += 1;\n                    $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                    #[cfg(test)]\n                    $census.step();\n                    $budget.charge(\n                        $function,\n                        1,\n                        \"ordinary plan history forward segment traversal\",\n                    )?;\n                    $owner.right_items = right_rest;\n                    $owner.right_cursor.item += 1;\n                    $owner.ordering = Ordering::Equal;\n                    $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n                    #[cfg(test)]\n                    $census.round();\n                    continue;\n"),
    (8365, "            if let [_, ..] = $owner.left_items {\n", "            if let [_, left_rest @ ..] = $owner.left_items {\n"),
    (8234, "            paid!($budget, $function, size_of::<bool>(), $phase)?;\n", "            paid!(\n                $budget,\n                $function,\n                size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>(),\n                $phase\n            )?;\n"),
    (7623, "// Six payment and five work sites have distinct complete primitive frames in\n", "// Four payment and five work sites have distinct complete primitive frames in\n"),
    (488, "pub(super) const PRIMITIVE_FRAMES: usize = 42 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 40 * PAYMENT_HEADER;\n"),
    (339, "// The shared immutable-step branch adds six payment and five work sites.\n", "// The shared immutable-step branch adds four payment and five work sites.\n"),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    let current = super::super::super::paired_links::previous_pair_tests(current);
    let current = current.as_str();
    restore(current, 49614, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (47511, "\n#[test]\nfn shared_run_replays_every_real_883_work_cut_before_private_cursor_publication() {\n    let program = super::super::super::tests::program();\n    let function = program\n        .functions\n        .iter()\n        .find(|f| f.id.as_str() == \"f\")\n        .unwrap();\n    let item = Rc::new(SkeletonObservation::Boolean {\n        expression: function.body.id.clone(),\n        value: true,\n    });\n    let expected = vec![\n        SkeletonPath {\n            observations: vec![item.clone(); 4].into(),\n            terminal: SkeletonTerminal::Success\n        };\n        2\n    ];\n    let actual = || HistorySet {\n        arena: vec![Segment {\n            previous: None,\n            depth: 1,\n            items: vec![item.clone(); 4],\n        }],\n        paths: (0..2)\n            .map(|_| PlanPath {\n                history: History {\n                    previous: Some(0),\n                    tail: Vec::new(),\n                },\n                terminal: SkeletonTerminal::Success,\n            })\n            .collect(),\n        max_depth: 1,\n    };\n    pair_scope_census::reset();\n    let (old, old_heap, _) =\n        observe_shaped_pair(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);\n    pair_scope_census::reset();\n    let (new, new_heap, _) =\n        observe_shaped_pair(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);\n    let row = complete_row(false);\n    assert!(row.shared_runs > 0 && row.shared_steps > row.shared_runs);\n    assert_eq!(row.shared_turns, row.shared_steps + row.shared_runs);\n    assert_eq!(new, old);\n    assert!(new.result.is_ok());\n    assert_eq!(new_heap, old_heap);\n    let spent = MAX_REPLAY_WORK_UNITS - old.remaining;\n    for cut in 0..=spent {\n        pair_scope_census::reset();\n        let (old, _, _) = observe_shaped_pair(function, &expected, actual(), true, cut);\n        pair_scope_census::reset();\n        let (new, _, _) = observe_shaped_pair(function, &expected, actual(), false, cut);\n        assert_eq!(\n            new, old,\n            \"complete six-field diagnostics and exact remaining work at cut {cut}\"\n        );\n    }\n}\n", ""),
    (6600, "        + (2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>()) * row.shared_runs\n        + (4 * size_of::<usize>() + size_of::<bool>()) * row.shared_turns\n        + (4 * size_of::<&[Rc<SkeletonObservation>]>()\n            + 4 * size_of::<usize>()\n            + size_of::<Ordering>())\n            * row.shared_runs;\n", "        + (3 * size_of::<&[Rc<SkeletonObservation>]>()\n            + 2 * size_of::<usize>()\n            + size_of::<Ordering>()\n            + size_of::<bool>())\n            * row.shared_steps;\n"),
    (6469, "        + size_of::<bool>() * row.shared_patterns\n", "        + (size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>()) * row.shared_patterns\n"),
    (6325, "        + if paths >= 2 { 11 * header } else { 0 }\n", "        + if paths >= 2 { 9 * header } else { 0 }\n"),
];
pub(super) fn previous_census(current: &str) -> String {
    restore(current, 16853, CENSUS)
}
#[rustfmt::skip]
const CENSUS: &[(usize, &str, &str)] = &[
    (15803, "    #[test]\n    fn shared_run_counter_overflow_is_incomplete_without_work_or_identity_aliasing() {\n        for turn in [false, true] {\n            let function = fixture();\n            let budget = ReplayBudget::unobserved();\n            let remaining = budget.remaining;\n            reset();\n            let guard = begin(&function, &budget, false);\n            TRACE.with(|trace| {\n                let mut trace = trace.borrow_mut();\n                if turn {\n                    trace.rows[0].shared_turns = usize::MAX;\n                } else {\n                    trace.rows[0].shared_runs = usize::MAX;\n                }\n            });\n            if turn {\n                guard.shared_turns();\n            } else {\n                guard.shared_runs();\n            }\n            guard.success();\n            drop(guard);\n            let (rows, lost) = finish();\n            assert_eq!(lost, 0);\n            assert_eq!(budget.remaining, remaining);\n            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);\n        }\n    }\n", ""),
    (7099, "            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_runs(&self) {\n        self.update(|r| match r.shared_runs.checked_add(1) {\n            Some(n) => r.shared_runs = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_turns(&self) {\n        self.update(|r| match r.shared_turns.checked_add(1) {\n            Some(n) => r.shared_turns = n,\n", ""),
    (1351, "    shared_runs: 0,\n    shared_turns: 0,\n", ""),
    (732, "    pub shared_runs: usize,\n    pub shared_turns: usize,\n", ""),
];
pub(super) fn previous_boundary_controls(current: &str) -> String {
    let current = super::super::super::paired_links::previous_boundary_controls(current);
    let current = current.as_str();
    restore(current, 22995, BOUNDARY_CONTROLS)
}
#[rustfmt::skip]
const BOUNDARY_CONTROLS: &[(usize, &str, &str)] = &[
    (22585, "            &shared_runs::previous_route(include_str!(\n                \"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"\n            )),\n            previous_route_stage,\n", "            include_str!(\"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"),\n            previous_route,\n"),
    (21905, "            &shared_runs::previous_census(include_str!(\"../../../../../pair_scope_census.rs\")),\n            previous_census_stage,\n", "            include_str!(\"../../../../../pair_scope_census.rs\"),\n            previous_census,\n"),
    (21532, "            &shared_runs::previous_pair_tests(include_str!(\"../../../../../pair_scope_tests.rs\")),\n            previous_pair_tests_stage,\n", "            include_str!(\"../../../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n"),
    (21181, "            &shared_runs::previous_macro(include_str!(\"../../../../../lexical_cursor.rs\")),\n            previous_macro_stage,\n", "            include_str!(\"../../../../../lexical_cursor.rs\"),\n            previous_macro,\n"),
    (17387, "    previous_route_stage(&shared_runs::previous_route(current))\n}\nfn previous_route_stage(current: &str) -> String {\n", ""),
    (13153, "    previous_census_stage(&shared_runs::previous_census(current))\n}\nfn previous_census_stage(current: &str) -> String {\n", ""),
    (7881, "    previous_pair_tests_stage(&shared_runs::previous_pair_tests(current))\n}\nfn previous_pair_tests_stage(current: &str) -> String {\n", ""),
    (794, "    previous_macro_stage(&shared_runs::previous_macro(current))\n}\nfn previous_macro_stage(current: &str) -> String {\n", ""),
    (721, "mod shared_runs;\n", ""),
];
pub(super) fn previous_route(current: &str) -> String {
    let current = super::super::super::paired_links::previous_route(current);
    let current = current.as_str();
    restore(current, 35279, ROUTE)
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
    (34428, "            .count(),\n        1\n    );\n    assert_eq!(\n        compact\n            .matches(\"shared_plan_segment_run!($owner,$census,$function,$budget,$phase);\")\n", ""),
    (34171, "    assert_eq!(lexical.matches(\"shared_plan_segment_run!(\").count(), 1);\n", ""),
    (34032, "    assert_eq!(sites.len(), 13, \"audit every generic payment site\");\n", "    assert_eq!(sites.len(), 11, \"audit every generic payment site\");\n"),
];
#[cfg(test)]
mod tests {
    use super::*;
    fn hostile(
        current: &str,
        inverse: fn(&str) -> String,
        before: &str,
        edits: &[(usize, &str, &str)],
    ) {
        let &(at, after, _) = edits
            .iter()
            .find(|(_, after, _)| !after.is_empty())
            .unwrap();
        let mut changed = current.to_owned();
        let end = at + after.chars().next().unwrap().len_utf8();
        let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
        changed.replace_range(at..end, replacement);
        assert!(std::panic::catch_unwind(|| inverse(&changed)).is_err());
        assert!(std::panic::catch_unwind(|| inverse(&format!("{current} "))).is_err());
        let at = current
            .char_indices()
            .find(|(i, c)| {
                c.is_ascii_alphanumeric()
                    && !edits
                        .iter()
                        .any(|(at, after, _)| *i >= *at && *i < at + after.len())
            })
            .unwrap()
            .0;
        let mut outside = current.to_owned();
        let replacement = if &outside[at..at + 1] == "x" {
            "y"
        } else {
            "x"
        };
        outside.replace_range(at..at + 1, replacement);
        let restored = std::panic::catch_unwind(|| inverse(&outside));
        assert!(restored.is_err() || restored.unwrap() != before);
    }
    #[test]
    fn whole_run_sources_restore_every_complete_predecessor() {
        assert_eq!(
            previous_macro(include_str!("../../../../../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_pair_tests(include_str!("../../../../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-pair-tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_census(include_str!("../../../../../../pair_scope_census.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-census.rs.txt"
            ))
        );
        assert_eq!(
            previous_boundary_controls(include_str!("../shared_boundaries.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-boundary-controls.rs.txt"
            ))
        );
        assert_eq!(
            previous_route(include_str!(
                "../../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-route.rs.txt"
            ))
        );
    }
    #[test]
    fn run_sources_refuse_span_extent_and_outside_drift_without_masking() {
        hostile(
            &super::super::super::super::paired_links::previous_macro(include_str!(
                "../../../../../../lexical_cursor.rs"
            )),
            previous_macro,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-macro.rs.txt"
            )),
            MACRO,
        );
        hostile(
            &super::super::super::super::paired_links::previous_pair_tests(include_str!(
                "../../../../../../pair_scope_tests.rs"
            )),
            previous_pair_tests,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-pair-tests.rs.txt"
            )),
            PAIR_TESTS,
        );
        hostile(
            include_str!("../../../../../../pair_scope_census.rs"),
            previous_census,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-census.rs.txt"
            )),
            CENSUS,
        );
        hostile(
            &super::super::super::super::paired_links::previous_boundary_controls(include_str!(
                "../shared_boundaries.rs"
            )),
            previous_boundary_controls,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-boundary-controls.rs.txt"
            )),
            BOUNDARY_CONTROLS,
        );
        hostile(
            &super::super::super::super::paired_links::previous_route(include_str!(
                "../../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            previous_route,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-route.rs.txt"
            )),
            ROUTE,
        );
    }
}
