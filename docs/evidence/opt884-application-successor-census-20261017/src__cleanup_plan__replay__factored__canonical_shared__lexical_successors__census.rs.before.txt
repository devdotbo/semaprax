//! Fixed diagnostic aggregate only: no allocation, authority, payer or gate input.
use super::*;
use std::cell::RefCell;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Cell {
    pub attempts: usize,
    pub requested: usize,
    pub acquired: usize,
    pub primitive_banks: usize,
    pub refused: usize,
    pub selection_requested: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Report {
    // [old/current][other/HIR/PLAN][general/unit]
    pub cells: [[[Cell; 2]; 3]; 2],
    pub complete: bool,
    pub function: usize,
    pub budget: usize,
    pub invocation: usize,
    pub foreign_observation: bool,
}
#[derive(Clone, Copy)]
struct Pending {
    old: usize,
    phase: usize,
    unit: usize,
    bytes: usize,
    banks: usize,
}
struct State {
    enabled: bool,
    phase: usize,
    serial: usize,
    report: Report,
    pending: Option<Pending>,
}
impl State {
    const fn new() -> Self {
        Self {
            enabled: false,
            phase: 0,
            serial: 0,
            report: Report {
                cells: [[[Cell {
                    attempts: 0,
                    requested: 0,
                    acquired: 0,
                    primitive_banks: 0,
                    refused: 0,
                    selection_requested: 0,
                }; 2]; 3]; 2],
                complete: true,
                function: 0,
                budget: 0,
                invocation: 0,
                foreign_observation: false,
            },
            pending: None,
        }
    }
}
thread_local! { static STATE: RefCell<State> = const { RefCell::new(State::new()) }; }
fn add(slot: &mut usize, value: usize, complete: &mut bool) {
    match slot.checked_add(value) {
        Some(n) => *slot = n,
        None => {
            *slot = usize::MAX;
            *complete = false;
        }
    }
}
pub(super) struct Observation {
    previous: bool,
}
pub(super) fn observe(function: &ResolvedFunction, budget: &ReplayBudget) -> Observation {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let previous = s.enabled;
        assert!(!previous, "diagnostic observations are not nested");
        let mut complete = true;
        let mut serial = s.serial;
        add(&mut serial, 1, &mut complete);
        s.serial = serial;
        s.report = State::new().report;
        s.report.complete = complete;
        s.report.function = std::ptr::from_ref(function) as usize;
        s.report.budget = std::ptr::from_ref(budget) as usize;
        s.report.invocation = serial;
        s.enabled = true;
        s.pending = None;
        Observation { previous }
    })
}
pub(super) fn take() -> Report {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(p) = s.pending.take() {
            let mut complete = s.report.complete;
            add(
                &mut s.report.cells[p.old][p.phase][p.unit].refused,
                1,
                &mut complete,
            );
            s.report.complete = complete;
        }
        s.report
    })
}
impl Drop for Observation {
    fn drop(&mut self) {
        STATE.with(|s| s.borrow_mut().enabled = self.previous);
    }
}
pub(in crate::cleanup_plan::replay::factored) struct Phase {
    previous: usize,
}
pub(in crate::cleanup_plan::replay::factored) fn phase(hir: bool) -> Phase {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let previous = s.phase;
        s.phase = if hir { 1 } else { 2 };
        Phase { previous }
    })
}
impl Drop for Phase {
    fn drop(&mut self) {
        STATE.with(|s| s.borrow_mut().phase = self.previous);
    }
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn request(
    unit: bool,
    old: bool,
    warm: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.enabled {
            return;
        }
        if s.report.function != std::ptr::from_ref(function) as usize
            || s.report.budget != std::ptr::from_ref(budget) as usize
        {
            s.report.foreign_observation = true;
            s.report.complete = false;
            return;
        }
        if s.pending.is_some() {
            s.report.complete = false;
            return;
        }
        let old = usize::from(!old);
        let phase = s.phase;
        let unit = usize::from(unit);
        let bytes = if old == 0 {
            ORIGINAL_RECEIVING
        } else if warm {
            WARM_RECEIVING
        } else {
            COLD_RECEIVING
        };
        let banks = usize::from(old == 0 || !warm);
        let mut complete = s.report.complete;
        let c = &mut s.report.cells[old][phase][unit];
        add(&mut c.attempts, 1, &mut complete);
        add(&mut c.requested, bytes, &mut complete);
        if old == 1 {
            add(&mut c.selection_requested, PER_USE_SELECTION, &mut complete);
        }
        s.report.complete = complete;
        s.pending = Some(Pending {
            old,
            phase,
            unit,
            bytes,
            banks,
        });
    });
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn acquired() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.enabled {
            return;
        }
        let Some(p) = s.pending.take() else {
            s.report.complete = false;
            return;
        };
        let mut complete = s.report.complete;
        let c = &mut s.report.cells[p.old][p.phase][p.unit];
        add(&mut c.acquired, p.bytes, &mut complete);
        add(&mut c.primitive_banks, p.banks, &mut complete);
        s.report.complete = complete;
    });
}
#[test]
fn fixed_successor_census_8769_rows_and_checked_overflow_are_diagnostic_only() {
    let mut count = 0;
    let mut complete = true;
    for _ in 0..8769 {
        add(&mut count, 1, &mut complete);
    }
    assert_eq!(count, 8769);
    assert!(complete);
    count = usize::MAX;
    add(&mut count, 1, &mut complete);
    assert_eq!(count, usize::MAX);
    assert!(!complete);
}

#[test]
fn all_twelve_census_cells_route_without_event_buffer_or_authority() {
    let program = crate::check("module c884; @id(\"main\") fn main()->i64{1}", "c884.spx").unwrap();
    let f = crate::hir::resolve(&program).unwrap().functions.remove(0);
    let b = ReplayBudget::unobserved();
    let guard = observe(&f, &b);
    // Diagnostic routing only: these calls do not reserve any real bytes and
    // their synthetic aggregate must never serve as an earned bill/fit claim.
    for phase_index in 0..3 {
        STATE.with(|s| s.borrow_mut().phase = phase_index);
        for old in [true, false] {
            for unit in [false, true] {
                for n in 0..8769 {
                    request(unit, old, n != 0, &f, &b);
                    acquired();
                }
            }
        }
    }
    let r = take();
    assert!(r.complete && !r.foreign_observation);
    assert_eq!(r.function, std::ptr::from_ref(&f) as usize);
    assert_eq!(r.budget, std::ptr::from_ref(&b) as usize);
    for old in 0..2 {
        for phase in 0..3 {
            for unit in 0..2 {
                let c = r.cells[old][phase][unit];
                let expected = if old == 0 {
                    8769 * ORIGINAL_RECEIVING
                } else {
                    COLD_RECEIVING + 8768 * WARM_RECEIVING
                };
                assert_eq!(
                    (c.attempts, c.requested, c.acquired, c.refused),
                    (8769, expected, expected, 0)
                );
                assert_eq!(c.primitive_banks, if old == 0 { 8769 } else { 1 });
            }
        }
    }
    drop(guard);
}
