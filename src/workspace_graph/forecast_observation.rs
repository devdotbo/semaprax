//! Fixed-storage, test-only observation of completed semantic cached forecasts.
//! It changes no debit, forecast, source validation, or admission result.
use std::cell::{Cell, RefCell};
const SLOTS: usize = 256;
const PATH: usize = 256;
#[derive(Clone, Copy)]
struct Record {
    path: [u8; PATH],
    len: usize,
    semantic: bool,
    authority_request: bool,
    values: [usize; 17],
}
struct State {
    entries: [Option<Record>; SLOTS],
    count: usize,
    oversized_paths: usize,
    forecasts: usize,
    authority_requests: usize,
}
impl State {
    const fn new() -> Self {
        Self {
            entries: [None; SLOTS],
            count: 0,
            oversized_paths: 0,
            forecasts: 0,
            authority_requests: 0,
        }
    }
}
thread_local! {
    static ACTIVE: Cell<bool> = const {Cell::new(false)};
    static STATE: RefCell<State> = const {RefCell::new(State::new())};
}
pub(crate) fn record(path: &str, mode: u8, semantic: bool, values: [usize; 17]) {
    if mode != 7 || !ACTIVE.with(Cell::get) {
        return;
    }
    capture(path, semantic, false, values);
}
pub(crate) fn authority_index(
    path: &str,
    indexed: usize,
    node: usize,
    requested: usize,
    remaining: Option<usize>,
) {
    if !ACTIVE.with(Cell::get) {
        return;
    }
    let mut values = [0; 17];
    values[..5].copy_from_slice(&[
        indexed,
        node,
        requested,
        remaining.unwrap_or(usize::MAX),
        usize::from(remaining.is_some()),
    ]);
    capture(path, false, true, values);
}
fn capture(path: &str, semantic: bool, authority_request: bool, values: [usize; 17]) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if path.len() > PATH {
            state.oversized_paths += 1;
            return;
        }
        let mut stored = Record {
            path: [0; PATH],
            len: path.len(),
            semantic,
            authority_request,
            values,
        };
        stored.path[..path.len()].copy_from_slice(path.as_bytes());
        let index = state.count % SLOTS;
        state.entries[index] = Some(stored);
        state.count += 1;
        if authority_request {
            state.authority_requests += 1;
        } else {
            state.forecasts += 1;
        }
    });
}
/// The callback runs under unchanged production budgets. Only reporting after
/// it returns allocates/serializes. Ring truncation is explicitly reported.
pub(crate) fn observe<T>(application: &str, operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|a| a.set(self.0));
        }
    }
    assert!(!ACTIVE.with(Cell::get), "forecast observers cannot nest");
    STATE.with(|s| *s.borrow_mut() = State::new());
    let guard = Restore(ACTIVE.with(|a| a.replace(true)));
    let result = operation();
    drop(guard);
    STATE.with(|state| {
        let state=state.borrow();
        eprintln!("{}",serde_json::json!({"schema":"semaprax.forecast-observation.v1",
            "application":application,"completed_mode7_forecasts":state.forecasts,"authority_index_requests":state.authority_requests,
            "retained_records":state.count.min(SLOTS),"discarded_prefix_records":state.count.saturating_sub(SLOTS),
            "oversized_path_records":state.oversized_paths,
            "scope":"completed per-module static forecasts; not allocation observations or proof of application fit"}));
        for ordinal in state.count.saturating_sub(SLOTS)..state.count {
            let item=state.entries[ordinal%SLOTS].unwrap();
            let v=item.values;
            if item.authority_request {
                assert_eq!(v[0].checked_mul(v[1]),Some(v[2]));
                eprintln!("{}",serde_json::json!({"schema":"semaprax.authority-index-request.v1",
                    "application":application,"ordinal":ordinal,
                    "caller_path":std::str::from_utf8(&item.path[..item.len]).unwrap(),
                    "indexed_declarations":v[0],"btree_node_upper":v[1],"requested_bytes":v[2],
                    "enclosing_remaining":(v[4]!=0).then_some(v[3]),
                    "scope":"request before actual authority-index reservation; no grant or allocation observation"}));
                continue;
            }
            assert_eq!(v[13],v[0]+v[10]+v[11]+v[12]);
            eprintln!("{}",serde_json::json!({"schema":"semaprax.forecast-component.v1",
                "application":application,"ordinal":ordinal,
                "path":std::str::from_utf8(&item.path[..item.len]).unwrap(),
                "retention":if item.semantic {"Semantic"} else {"Parsed"},
                "raw_total":v[0],"raw_strings":v[1],"raw_exact_expressions":v[2],
                "raw_literal_absence_bytes":v[3],"runtime_total":v[4],"runtime_strings":v[5],
                "runtime_exact_expressions":v[6],"structure_input":v[7],"string_input":v[8],
                "fixed_upper_before_mode7":v[9],"fixed_upper_after_mode7":v[10],
                "identity_occurrence_upper":v[11],"identity_carrier_upper":v[12],
                "retained_clone_and_hir":v[13],"generic_instances":v[14],"transient_import_clone":v[15],
                "original_mode7_fixed_upper":(v[16]!=usize::MAX).then_some(v[16]),
                "original_formula_overflow":v[16]==usize::MAX}));
        }
    });
    result
}
