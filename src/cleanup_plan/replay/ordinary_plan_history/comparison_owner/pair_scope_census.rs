//! Diagnostic-only fixed storage. No compiler payment, work or verdict authority.
use super::*;
use std::cell::{Cell, RefCell};
#[derive(Clone, Copy, Debug)]
pub(super) struct Row {
    pub invocation: usize,
    pub generation: usize,
    pub serial: usize,
    pub old: bool,
    pub function: [u8; 256],
    pub function_len: usize,
    pub payer: &'static str,
    pub selected: bool,
    pub owner_admitted: bool,
    pub pair_matches_admitted: usize,
    pub pairs: usize,
    pub rounds: usize,
    pub value_steps: usize,
    pub value_tails: usize,
    pub tails: usize,
    pub steps: usize,
    pub loads: usize,
    pub identity_detectors: usize,
    pub identity_hits: usize,
    pub identity_observations: usize,
    pub identity_work: usize,
    pub identity_captures: usize,
    pub identity_extent_captures: usize,
    pub complete: bool,
    pub ended: bool,
    pub unwound: bool,
    pub overflow: bool,
    pub truncated: bool,
}
const EMPTY: Row = Row {
    invocation: 0,
    generation: 0,
    serial: 0,
    old: false,
    function: [0; 256],
    function_len: 0,
    payer: "",
    selected: false,
    owner_admitted: false,
    pair_matches_admitted: 0,
    pairs: 0,
    rounds: 0,
    value_steps: 0,
    value_tails: 0,
    tails: 0,
    steps: 0,
    loads: 0,
    identity_detectors: 0,
    identity_hits: 0,
    identity_observations: 0,
    identity_work: 0,
    identity_captures: 0,
    identity_extent_captures: 0,
    complete: false,
    ended: false,
    unwound: false,
    overflow: false,
    truncated: false,
};
pub(super) fn empty_for_test() -> Row {
    EMPTY
}
struct Trace {
    enabled: bool,
    count: usize,
    generation: usize,
    serial: usize,
    rows: [Row; 128],
}
thread_local! {
    static TRACE: RefCell<Trace> = const { RefCell::new(Trace { enabled: false, count: 0, generation: 0, serial: 0, rows: [EMPTY; 128] }) };
    static LOST: Cell<usize> = const { Cell::new(0) };
    static UNWIND_IDENTITY: Cell<bool> = const { Cell::new(false) };
}
fn lost() {
    LOST.with(|n| n.set(n.get().saturating_add(1)));
}
pub(super) fn unwind_next_identity_observation() {
    UNWIND_IDENTITY.with(|flag| flag.set(true));
}
pub(super) fn reset() {
    UNWIND_IDENTITY.with(|flag| flag.set(false));
    LOST.with(|n| n.set(0));
    TRACE.with(|t| {
        let Ok(mut t) = t.try_borrow_mut() else {
            lost();
            return;
        };
        if t.enabled {
            for row in &t.rows[..t.count] {
                if !row.ended {
                    lost();
                }
            }
        }
        t.count = 0;
        if let Some(generation) = t.generation.checked_add(1) {
            t.generation = generation;
            t.enabled = true;
        } else {
            t.enabled = false;
            lost();
        }
    });
}
#[derive(Clone, Copy)]
struct Identity {
    index: usize,
    generation: usize,
    serial: usize,
}
pub(in crate::cleanup_plan::replay::ordinary_plan_history) struct Guard {
    identity: Option<Identity>,
}
pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn begin(
    function: &ResolvedFunction,
    budget: &ReplayBudget,
    old: bool,
) -> Guard {
    let identity = TRACE.with(|t| {
        let Ok(mut t) = t.try_borrow_mut() else {
            lost();
            return None;
        };
        if !t.enabled {
            return None;
        }
        if t.count == t.rows.len() {
            lost();
            return None;
        }
        let Some(serial) = t.serial.checked_add(1) else {
            lost();
            return None;
        };
        t.serial = serial;
        let index = t.count;
        t.count += 1;
        let mut row = EMPTY;
        row.invocation = index;
        row.generation = t.generation;
        row.serial = serial;
        row.old = old;
        let id = function.id.as_str().as_bytes();
        row.function_len = id.len().min(row.function.len());
        row.truncated = id.len() > row.function.len();
        row.function[..row.function_len].copy_from_slice(&id[..row.function_len]);
        row.payer = match budget.payer {
            AllocationPayer::LegacyUnobserved => "unobserved",
            AllocationPayer::ActiveBuilder => "active-builder",
            AllocationPayer::SelectedReservation => "selected-reservation",
            AllocationPayer::Oracle(_) => "oracle",
        };
        row.selected = budget.admission == allocation::AdmissionMode::SelectedRetained;
        t.rows[index] = row;
        Some(Identity {
            index,
            generation: t.generation,
            serial,
        })
    });
    Guard { identity }
}
impl Guard {
    fn update(&self, action: impl FnOnce(&mut Row)) {
        let Some(identity) = self.identity else {
            return;
        };
        TRACE.with(|t| {
            if let Ok(mut t) = t.try_borrow_mut() {
                if !t.enabled || t.generation != identity.generation || identity.index >= t.count {
                    lost();
                    return;
                }
                let row = &mut t.rows[identity.index];
                if row.generation != identity.generation || row.serial != identity.serial {
                    lost();
                    return;
                }
                action(row);
            } else {
                lost();
            }
        });
    }
    pub(super) fn owner_admitted(&self) {
        self.update(|r| r.owner_admitted = true);
    }
    pub(super) fn pair_match_admitted(&self) {
        self.update(|r| match r.pair_matches_admitted.checked_add(1) {
            Some(n) => r.pair_matches_admitted = n,
            None => r.overflow = true,
        });
    }
    pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn pair(&self) {
        self.update(|r| match r.pairs.checked_add(1) {
            Some(n) => r.pairs = n,
            None => r.overflow = true,
        });
    }
    pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn round(&self) {
        self.update(|r| match r.rounds.checked_add(1) {
            Some(n) => r.rounds = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn value_step(&self) {
        self.update(|r| match r.value_steps.checked_add(1) {
            Some(n) => r.value_steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn value_tail(&self) {
        self.update(|r| match r.value_tails.checked_add(1) {
            Some(n) => r.value_tails = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn step(&self) {
        self.update(|r| match r.steps.checked_add(1) {
            Some(n) => r.steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn load(&self) {
        self.update(|r| match r.loads.checked_add(1) {
            Some(n) => r.loads = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_detector(&self) {
        self.update(|r| match r.identity_detectors.checked_add(1) {
            Some(n) => r.identity_detectors = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_hit(&self) {
        self.update(|r| match r.identity_hits.checked_add(1) {
            Some(n) => r.identity_hits = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_observation(&self) {
        self.update(|r| match r.identity_observations.checked_add(1) {
            Some(n) => r.identity_observations = n,
            None => r.overflow = true,
        });
        if UNWIND_IDENTITY.with(|flag| flag.replace(false)) {
            panic!("test-only unwind inside live certified PLAN remainder");
        }
    }
    pub(super) fn identity_work(&self) {
        self.update(|r| match r.identity_work.checked_add(1) {
            Some(n) => r.identity_work = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_capture(&self) {
        self.update(|r| match r.identity_captures.checked_add(1) {
            Some(n) => r.identity_captures = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_extent_capture(&self) {
        self.update(|r| match r.identity_extent_captures.checked_add(1) {
            Some(n) => r.identity_extent_captures = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn tail(&self) {
        self.update(|r| match r.tails.checked_add(1) {
            Some(n) => r.tails = n,
            None => r.overflow = true,
        });
    }
    pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn success(&self) {
        self.update(|r| r.complete = !r.truncated && !r.overflow);
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.update(|r| {
            r.ended = true;
            r.unwound = std::thread::panicking();
        });
    }
}
// Called only after the original result/fixture/owner Drop and observed budget.
// Allocation here cannot affect a compiler outcome or frontier.
pub(super) fn finish() -> (Vec<Row>, usize) {
    let mut rows = TRACE.with(|t| {
        let Ok(mut t) = t.try_borrow_mut() else {
            lost();
            return Vec::new();
        };
        t.enabled = false;
        let count = t.count;
        for row in &mut t.rows[..count] {
            if !row.ended || row.unwound || row.truncated || row.overflow {
                row.complete = false;
            }
        }
        t.rows[..t.count].to_vec()
    });
    let lost = LOST.with(Cell::get);
    if lost != 0 {
        for row in &mut rows {
            row.complete = false;
        }
    }
    (rows, lost)
}

#[cfg(test)]
mod controls {
    use super::*;
    fn fixture() -> ResolvedFunction {
        let mut program = super::super::super::tests::program();
        program.functions.remove(0)
    }
    #[test]
    fn nested_reset_rejects_foreign_and_restored_generation_without_touching_new_row() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let outer = begin(&function, &budget, true);
        outer.pair();
        reset();
        let current = begin(&function, &budget, false);
        current.pair();
        outer.round();
        outer.success();
        drop(outer);
        current.round();
        current.success();
        drop(current);
        let (rows, lost_count) = finish();
        assert!(lost_count > 0);
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].old, rows[0].pairs, rows[0].rounds), (false, 1, 1));
        assert!(rows[0].ended && !rows[0].complete);
        reset();
        let restored = begin(&function, &budget, true);
        restored.success();
        drop(restored);
        let (rows, lost_count) = finish();
        assert_eq!(lost_count, 0);
        assert!(rows[0].complete);
    }
    #[test]
    fn finish_while_guard_is_live_cannot_publish_complete_or_resume_same_row() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let live = begin(&function, &budget, false);
        live.success();
        let (rows, lost_count) = finish();
        assert_eq!(lost_count, 0);
        assert!(!rows[0].ended && !rows[0].complete);
        live.pair();
        drop(live);
        let (rows, lost_count) = finish();
        assert!(lost_count > 0);
        assert_eq!(rows[0].pairs, 0);
        assert!(!rows[0].complete);
    }
    #[test]
    fn serial_overflow_is_explicit_loss_and_does_not_alias_a_live_invocation() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let live = begin(&function, &budget, false);
        live.pair();
        TRACE.with(|t| t.borrow_mut().serial = usize::MAX);
        let refused = begin(&function, &budget, true);
        refused.success();
        drop(refused);
        live.success();
        drop(live);
        let (rows, lost_count) = finish();
        assert_eq!(lost_count, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pairs, 1);
        assert!(!rows[0].complete);
        // Restore test-only counter state after proving the terminal overflow.
        TRACE.with(|t| t.borrow_mut().serial = 0);
    }
    #[test]
    fn generation_overflow_disables_collection_and_never_wraps_to_old_identity() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let old = begin(&function, &budget, true);
        TRACE.with(|t| t.borrow_mut().generation = usize::MAX);
        reset();
        let refused = begin(&function, &budget, false);
        refused.success();
        drop(refused);
        old.success();
        drop(old);
        let (rows, lost_count) = finish();
        assert!(lost_count > 0);
        assert!(rows.is_empty());
        TRACE.with(|t| t.borrow_mut().generation = 0);
    }
    #[test]
    fn prepared_link_counter_overflow_is_incomplete_and_does_not_spend_work() {
        for load in [false, true] {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut trace = trace.borrow_mut();
                if load {
                    trace.rows[0].loads = usize::MAX;
                } else {
                    trace.rows[0].steps = usize::MAX;
                }
            });
            if load {
                guard.load();
            } else {
                guard.step();
            }
            guard.success();
            drop(guard);
            let (rows, lost_count) = finish();
            assert_eq!(lost_count, 0);
            assert_eq!(rows.len(), 1);
            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);
            assert_eq!(budget.remaining, remaining);
        }
    }
}

// Sole owning-test observation scope, outside compiler ledgers and allocation
// windows. It prints after every measured value/guard has dropped, including
// unwind, without converting an incomplete refused prefix into a full result.
pub(in crate::cleanup_plan::replay) struct Scope {
    bytes: usize,
    outcome: Cell<Option<(bool, bool)>>,
}
impl Scope {
    pub(in crate::cleanup_plan::replay) fn whole_caller(bytes: usize) -> Self {
        reset();
        Self {
            bytes,
            outcome: Cell::new(None),
        }
    }
    pub(in crate::cleanup_plan::replay) fn observed(&self, success: bool, refused: bool) {
        self.outcome.set(Some((success, refused)));
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let (rows, lost) = finish();
        let unwind = std::thread::panicking();
        let sum = |field: fn(&Row) -> usize| {
            rows.iter()
                .try_fold(0usize, |n, row| n.checked_add(field(row)))
        };
        let totals = [
            sum(|r| r.pairs),
            sum(|r| r.rounds),
            sum(|r| r.pair_matches_admitted),
            sum(|r| r.value_steps),
            sum(|r| r.value_tails),
            sum(|r| r.steps),
            sum(|r| r.loads),
        ];
        let total_overflow = totals.iter().any(Option::is_none);
        let complete = self
            .outcome
            .get()
            .is_some_and(|(ok, refused)| ok && !refused)
            && !unwind
            && lost == 0
            && !total_overflow
            && rows
                .iter()
                .all(|row| row.complete && !row.old && row.owner_admitted);
        eprintln!(
            "OPT888 whole-caller current_candidate=true unqualified=true bytes={} outcome={:?} unwind={} complete={} partial_prefix={} invocations_reached={} owner_banks_admitted={} pairs_prepared={:?} rounds_reached={:?} match_banks_admitted={:?} step_banks_admitted={:?} tail_banks_admitted={:?} prepared_steps={:?} prepared_loads={:?} total_overflow={} lost={} row_capacity=128; complete-only fused component equation current+tuple*N+2Option*(STEP+TAIL)=old886+5HEADER*M+2reference*N; flat/retained-tail/prepared-link components billed separately; no whole-shipping net claim",
            self.bytes,
            self.outcome.get(),
            unwind,
            complete,
            !complete,
            rows.len(),
            rows.iter().filter(|r| r.owner_admitted).count(),
            totals[0],
            totals[1],
            totals[2],
            totals[3],
            totals[4],
            totals[5],
            totals[6],
            total_overflow,
            lost
        );
        eprintln!(
            "OPT888 typed-invoice tuple={} option_pair={} added_outer_references={} added_cold_headers={}",
            size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>(),
            2 * size_of::<Option<&SkeletonObservation>>(),
            NEW_PAIR_REFERENCES,
            paired_value::ADDITIONAL_PRIMITIVES
        );
        for row in rows {
            let function = std::str::from_utf8(&row.function[..row.function_len])
                .unwrap_or("<truncated UTF-8>");
            eprintln!(
                "OPT888 row={} generation={} serial={} function={:?} old={} payer={} selected={} owner_admitted={} pairs={} rounds={} match_admitted={} STEP={} TAIL={} prepared_steps={} prepared_loads={} complete={} ended={} unwound={} overflow={} truncated={}",
                row.invocation,
                row.generation,
                row.serial,
                function,
                row.old,
                row.payer,
                row.selected,
                row.owner_admitted,
                row.pairs,
                row.rounds,
                row.pair_matches_admitted,
                row.value_steps,
                row.value_tails,
                row.steps,
                row.loads,
                row.complete,
                row.ended,
                row.unwound,
                row.overflow,
                row.truncated
            );
        }
    }
}
