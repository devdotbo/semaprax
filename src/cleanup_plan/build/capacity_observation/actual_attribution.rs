//! Raw requested-Layout attribution, independent of the historical capacity model.
//! The observer runs continuously; these allocation-free snapshots do not define
//! its peak, omit any allocation window, rewrite metrics or spend a budget.
use crate::test_allocation_observer::{current_metrics, Metrics};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Point {
    IndexStart,
    IndexEnd,
    BeforePush,
    AfterPush,
    Return,
    Publication,
}
const POINTS: usize = 6;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Trace {
    pub start: Metrics,
    pub counts: [usize; POINTS],
    pub first: [Option<Metrics>; POINTS],
    pub last: [Option<Metrics>; POINTS],
    pub largest_sampled_live: [Option<Metrics>; POINTS],
    pub valid: bool,
}
thread_local! { static TRACE: Cell<Option<Trace>> = const { Cell::new(None) }; }
pub(super) struct Scope {
    previous: Option<Trace>,
}
impl Scope {
    /// This private test receiver must start inside an unchanged allocation observer.
    pub fn begin() -> Self {
        let start = current_metrics();
        Self {
            previous: TRACE.with(|cell| {
                cell.replace(Some(Trace {
                    start,
                    counts: [0; POINTS],
                    first: [None; POINTS],
                    last: [None; POINTS],
                    largest_sampled_live: [None; POINTS],
                    valid: !start.invalid,
                }))
            }),
        }
    }
    pub fn snapshot(&self) -> Trace {
        TRACE.with(|cell| cell.get().expect("active attribution scope"))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        TRACE.with(|cell| cell.set(self.previous));
    }
}
fn record(point: Point) {
    TRACE.with(|cell| {
        let Some(mut trace) = cell.get() else {
            return;
        };
        let metrics = current_metrics();
        let index = point as usize;
        if let Some(count) = trace.counts[index].checked_add(1) {
            trace.counts[index] = count;
        } else {
            trace.valid = false;
        }
        trace.valid &= !metrics.invalid;
        if trace.first[index].is_none() {
            trace.first[index] = Some(metrics);
        }
        trace.last[index] = Some(metrics);
        if trace.largest_sampled_live[index].is_none_or(|old| old.live < metrics.live) {
            trace.largest_sampled_live[index] = Some(metrics);
        }
        cell.set(Some(trace));
    });
}
pub(in crate::cleanup_plan::build) fn index_start() {
    record(Point::IndexStart);
}
pub(in crate::cleanup_plan::build) fn index_end() {
    record(Point::IndexEnd);
}
pub(in crate::cleanup_plan::build) fn staging_before_push() {
    record(Point::BeforePush);
}
pub(in crate::cleanup_plan::build) fn staging_after_push() {
    record(Point::AfterPush);
}
pub(in crate::cleanup_plan::build) fn returning() {
    record(Point::Return);
}
pub(in crate::cleanup_plan::build) fn published() {
    record(Point::Publication);
}

/// Exact whole-current source projection; only the enumerated hooks may differ.
pub(super) fn project_original_helper(current: &str) -> String {
    let mut projected = current.to_owned();
    assert_eq!(projected.matches("    #[cfg(test)]\n    capacity_observation::actual_attribution::index_start();\n    let index = ModuleIndex::new(program)?;").count(), 1);
    projected = projected.replace("    #[cfg(test)]\n    capacity_observation::actual_attribution::index_start();\n    let index = ModuleIndex::new(program)?;", "    let index = ModuleIndex::new(program)?;");
    assert_eq!(projected.matches("    #[cfg(test)]\n    capacity_observation::actual_attribution::index_end();\n    let count = program\n").count(), 1);
    projected = projected.replace("    #[cfg(test)]\n    capacity_observation::actual_attribution::index_end();\n    let count = program\n", "    let count = program\n");
    assert_eq!(projected.matches("        plans.push(observe_finished_plan(lexical_control::build_with_index(\n            program, function, &execution, &index,\n        )?));").count(), 1);
    projected = projected.replace("        plans.push(observe_finished_plan(lexical_control::build_with_index(\n            program, function, &execution, &index,\n        )?));", "        plans.push(lexical_control::build_with_index(\n            program, function, &execution, &index,\n        )?);");
    assert_eq!(projected.matches("        plans.push(observe_finished_plan(lexical_control::build_with_index(\n            program,\n            &instance.function,\n            &execution,\n            &index,\n        )?));").count(), 1);
    projected = projected.replace("        plans.push(observe_finished_plan(lexical_control::build_with_index(\n            program,\n            &instance.function,\n            &execution,\n            &index,\n        )?));", "        plans.push(lexical_control::build_with_index(\n            program,\n            &instance.function,\n            &execution,\n            &index,\n        )?);");
    assert_eq!(projected.matches("        capacity_observation::staged(&plans);\n        #[cfg(test)]\n        capacity_observation::actual_attribution::staging_after_push();").count(), 2);
    projected = projected.replace("        capacity_observation::staged(&plans);\n        #[cfg(test)]\n        capacity_observation::actual_attribution::staging_after_push();", "        capacity_observation::staged(&plans);");
    assert_eq!(projected.matches("    #[cfg(test)]\n    capacity_observation::actual_attribution::returning();\n    Ok(plans)\n}").count(), 1);
    projected = projected.replace("    #[cfg(test)]\n    capacity_observation::actual_attribution::returning();\n    Ok(plans)\n}", "    Ok(plans)\n}");
    assert_eq!(projected.matches("    debug_assert!(plans.next().is_none());\n    #[cfg(test)]\n    capacity_observation::actual_attribution::published();\n    Ok(())").count(), 1);
    projected = projected.replace("    debug_assert!(plans.next().is_none());\n    #[cfg(test)]\n    capacity_observation::actual_attribution::published();\n    Ok(())", "    debug_assert!(plans.next().is_none());\n    Ok(())");
    assert_eq!(projected.matches("\n// Production is the standard identity move; test instrumentation copies raw\n// observer counters only, after the complete plan and before the original push.\n#[cfg(not(test))]\nuse std::convert::identity as observe_finished_plan;\n#[cfg(test)]\nfn observe_finished_plan(plan: CleanupPlan) -> CleanupPlan {\n    capacity_observation::actual_attribution::staging_before_push();\n    plan\n}\n#[cfg(test)]\npub(super) mod original_plans;\n").count(), 1);
    projected = projected.replace("\n// Production is the standard identity move; test instrumentation copies raw\n// observer counters only, after the complete plan and before the original push.\n#[cfg(not(test))]\nuse std::convert::identity as observe_finished_plan;\n#[cfg(test)]\nfn observe_finished_plan(plan: CleanupPlan) -> CleanupPlan {\n    capacity_observation::actual_attribution::staging_before_push();\n    plan\n}\n#[cfg(test)]\npub(super) mod original_plans;\n", "");
    projected
}

mod tests;
