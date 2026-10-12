//! Test-only, thread-bound observation of actual requested Rust Layout backing.
//! Other threads use System unchanged. The observed realloc route deliberately
//! allocates/copies/frees, retaining both buffers until copy completion; this is
//! an actual conforming allocator, not an estimated replacement-overlap charge.
//! These metrics are requested backing bytes, not malloc usable bytes or RSS.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};

// Fixed test instrumentation capacity: the original 4096-row selected gate
// holds 4096 title allocations plus its frame, row/slot tables and owned
// backing. Four entries per maximum row leave a bounded margin for those
// fixed allocations. This changes no runtime/compiler quota: exhaustion still
// marks observation invalid, and the fixed tracking array is excluded from the
// measured requested layouts just as before.
const SLOTS: usize = 16_384;
#[derive(Clone, Copy)]
struct Slot {
    pointer: usize,
    bytes: usize,
}
const EMPTY: Slot = Slot {
    pointer: 0,
    bytes: 0,
};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Metrics {
    pub(crate) allocations: usize,
    pub(crate) reallocations: usize,
    pub(crate) allocated: usize,
    pub(crate) freed: usize,
    pub(crate) live: usize,
    pub(crate) highwater: usize,
    pub(crate) invalid: bool,
}
struct State {
    slots: [Slot; SLOTS],
    metrics: Metrics,
}
impl State {
    const fn new() -> Self {
        Self {
            slots: [EMPTY; SLOTS],
            metrics: Metrics {
                allocations: 0,
                reallocations: 0,
                allocated: 0,
                freed: 0,
                live: 0,
                highwater: 0,
                invalid: false,
            },
        }
    }
    fn allocated(&mut self, pointer: usize, bytes: usize) {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.pointer == 0) else {
            INVALID.with(|invalid| invalid.set(true));
            return;
        };
        *slot = Slot { pointer, bytes };
        let (Some(allocated), Some(live), Some(count)) = (
            self.metrics.allocated.checked_add(bytes),
            self.metrics.live.checked_add(bytes),
            self.metrics.allocations.checked_add(1),
        ) else {
            INVALID.with(|invalid| invalid.set(true));
            return;
        };
        self.metrics.allocated = allocated;
        self.metrics.live = live;
        self.metrics.allocations = count;
        self.metrics.highwater = self.metrics.highwater.max(live);
    }
    fn freed(&mut self, pointer: usize, bytes: usize) {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.pointer == pointer) else {
            return;
        };
        if slot.bytes != bytes {
            INVALID.with(|invalid| invalid.set(true));
        }
        let actual = slot.bytes;
        *slot = EMPTY;
        let (Some(freed), Some(live)) = (
            self.metrics.freed.checked_add(actual),
            self.metrics.live.checked_sub(actual),
        ) else {
            INVALID.with(|invalid| invalid.set(true));
            return;
        };
        self.metrics.freed = freed;
        self.metrics.live = live;
    }
}
thread_local! {
    // Const fixed storage is initialized before the phase; the observer never
    // allocates a registry, grows a container or formats text in a callback.
    static STATE: RefCell<State> = const {RefCell::new(State::new())};
    static ACTIVE: Cell<bool> = const {Cell::new(false)};
    static INVALID: Cell<bool> = const {Cell::new(false)};
}
fn active() -> bool {
    ACTIVE.try_with(Cell::get).unwrap_or(false)
}
fn record(callback: impl FnOnce(&mut State)) {
    let result = STATE.try_with(|state| match state.try_borrow_mut() {
        Ok(mut state) => callback(&mut state),
        Err(_) => {
            let _ = INVALID.try_with(|invalid| invalid.set(true));
        }
    });
    if result.is_err() {
        let _ = INVALID.try_with(|invalid| invalid.set(true));
    }
}
struct ObservedSystem;
#[global_allocator]
static ALLOCATOR: ObservedSystem = ObservedSystem;
// SAFETY: all pointers/layouts come from GlobalAlloc's caller. System remains
// the allocator of every backing; realloc copies min(old,new) before releasing
// the old allocation and leaves it intact if the new allocation fails.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for ObservedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the valid caller layout unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if active() && !pointer.is_null() {
            record(|state| state.allocated(pointer as usize, layout.size()));
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the valid caller layout unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if active() && !pointer.is_null() {
            record(|state| state.allocated(pointer as usize, layout.size()));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if active() {
            record(|state| state.freed(pointer as usize, layout.size()));
        }
        // SAFETY: this allocation still belongs to System with its original layout.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if !active() {
            // SAFETY: preserve ordinary System behavior outside the probe thread.
            return unsafe { System.realloc(pointer, layout, size) };
        }
        let Ok(next) = Layout::from_size_align(size, layout.align()) else {
            INVALID.with(|invalid| invalid.set(true));
            return std::ptr::null_mut();
        };
        // SAFETY: allocate disjoint replacement backing of the requested alignment.
        let replacement = unsafe { System.alloc(next) };
        if replacement.is_null() {
            return replacement;
        }
        record(|state| {
            state.metrics.reallocations = state.metrics.reallocations.saturating_add(1);
            state.allocated(replacement as usize, size);
        });
        // SAFETY: both allocations are valid/nonoverlapping for min(old,new) bytes.
        unsafe { std::ptr::copy_nonoverlapping(pointer, replacement, layout.size().min(size)) };
        record(|state| state.freed(pointer as usize, layout.size()));
        // SAFETY: copied bytes are retained in replacement; release original once.
        unsafe { System.dealloc(pointer, layout) };
        replacement
    }
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
    }
}
/// Read the live counters inside an already active, nonnested observation.
/// This copies fixed instrumentation only and never changes its pointer table.
pub(crate) fn current_metrics() -> Metrics {
    assert!(active(), "live metrics require an active observation");
    STATE.with(|state| {
        let mut metrics = state.borrow().metrics;
        metrics.invalid = INVALID.with(Cell::get);
        metrics
    })
}
// Keep instrumentation outside the per-operation generic body. These helpers
// allocate nothing and retain the same observation and unwind boundaries.
fn begin_observation() -> Guard {
    assert!(!active(), "allocation observation cannot nest");
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.slots.fill(EMPTY);
        state.metrics = Metrics::default();
    });
    INVALID.with(|invalid| invalid.set(false));
    ACTIVE.with(|active| active.set(true));
    Guard
}
fn finish_observation(guard: Guard) -> Metrics {
    drop(guard);
    STATE.with(|state| {
        let mut metrics = state.borrow().metrics;
        metrics.invalid = INVALID.with(Cell::get);
        metrics
    })
}
pub(crate) fn measure<T>(operation: impl FnOnce() -> T) -> (T, Metrics) {
    let guard = begin_observation();
    let result = operation();
    let metrics = finish_observation(guard);
    (result, metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spare_capacity_and_replacement_overlap_are_observed_from_allocator_layouts() {
        let (capacities, metrics) = measure(|| {
            let mut bytes = Vec::<u8>::with_capacity(17);
            bytes.resize(17, 42);
            let first = bytes.capacity();
            bytes.reserve_exact(19);
            let second = bytes.capacity();
            assert!(bytes.iter().all(|value| *value == 42));
            std::hint::black_box(&bytes);
            (first, second)
        });
        assert_eq!(capacities, (17, 36));
        assert!(!metrics.invalid);
        assert_eq!(metrics.reallocations, 1);
        assert_eq!(metrics.allocated, 53);
        assert_eq!(metrics.freed, 53);
        assert_eq!(metrics.highwater, 53);
        assert_eq!(metrics.live, 0);
    }
    #[test]
    fn observation_boundaries_preserve_local_result_and_unwind_drop_timing() {
        struct Witness<'a>(&'a Cell<Option<bool>>);
        impl Drop for Witness<'_> {
            fn drop(&mut self) {
                self.0.set(Some(active()));
            }
        }
        let local = Cell::new(None);
        let returned = Cell::new(None);
        let (result, _) = measure(|| {
            let _local = Witness(&local);
            Witness(&returned)
        });
        assert_eq!(local.get(), Some(true));
        assert_eq!(returned.get(), None);
        assert!(!active());
        drop(result);
        assert_eq!(returned.get(), Some(false));

        let unwound = Cell::new(None);
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            measure(|| {
                let _local = Witness(&unwound);
                panic!("observe unwind boundary");
            });
        }));
        assert!(failure.is_err());
        assert_eq!(unwound.get(), Some(true));
        assert!(!active());
        let (value, _) = measure(|| {
            assert!(active());
            7
        });
        assert_eq!(value, 7);
        assert!(!active());
    }
    #[test]
    fn registry_overflow_is_invalid_evidence_instead_of_a_partial_success_count() {
        let mut owners = Vec::with_capacity(SLOTS + 1);
        let (_, metrics) = measure(|| {
            for _ in 0..=SLOTS {
                owners.push(Box::new(7u8));
            }
            std::hint::black_box(&owners);
        });
        assert!(metrics.invalid);
        assert!(metrics.live > 0);
        drop(owners);
    }
}
