use super::*;
const HELPERS: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
fn program(source: &str) -> ResolvedProgram {
    let source = crate::check(source, "validation-frontier.spx").unwrap();
    crate::hir::resolve(&source).unwrap()
}
#[test]
fn deferred_frontier_seals_exact_current_member_and_never_forces_future_cells() {
    let program = program(HELPERS);
    let mut frontier = Frontier::new(&program);
    assert!(frontier.row.get().is_none());
    assert_eq!(frontier.proof_count.get(), 0);
    let current = frontier.current().unwrap();
    current.sites().unwrap();
    current.has(Family::ByteSet);
    assert!(current.row.families[Family::ByteSet as usize]
        .get()
        .is_some());
    current.finish().unwrap();
    assert!(
        frontier.current().is_err(),
        "finished current cannot mint another capability"
    );
    frontier.advance().unwrap();
    assert!(frontier.row.get().is_none());
    assert!(frontier.advance().is_err());
}
#[test]
fn deferred_frontier_reuses_exact_positive_target_proofs() {
    let program = program(HELPERS);
    let mut frontier = Frontier::new(&program);
    for function in &program.functions {
        let current = frontier.current().unwrap();
        let expected = checked_bindings(&program, function).unwrap();
        let actual = current.sites().unwrap();
        for (at, binding) in expected {
            assert!(std::ptr::eq(actual.get(at).unwrap(), binding));
        }
        current.finish().unwrap();
        frontier.advance().unwrap();
    }
    assert_eq!(frontier.proof_count.get(), 2);
}

#[test]
fn current_row_completion_rejects_foreign_stale_and_repeated_advancement() {
    let program = program(HELPERS);
    let mut frontier = Frontier::new(&program);
    let mut foreign = Frontier::new(&program);
    assert!(frontier.advance().is_err());
    let current = frontier.current().unwrap();
    current.complete().unwrap();
    drop(current);
    assert!(
        frontier.advance().is_err(),
        "admission completion does not authorize member advancement"
    );
    assert!(frontier.row.get().is_some());
    foreign.current().unwrap().finish().unwrap();
    assert!(
        frontier.advance().is_err(),
        "foreign completion belongs only to its phase"
    );
    foreign.advance().unwrap();
    let current = frontier.current().unwrap();
    let stale = Current {
        frontier: current.frontier,
        row: current.row,
        ordinal: current.ordinal + 1,
        owner: None,
    };
    assert!(!stale.matches(&program, current.function()));
    assert!(stale.sites().is_err());
    assert!(stale.finish().is_err());
    current.finish().unwrap();
    assert!(frontier.current().is_err());
    frontier.advance().unwrap();
    assert!(
        frontier.advance().is_err(),
        "completed ordinal is consumed once"
    );
    let current = frontier.current().unwrap();
    let foreign_current = foreign.current().unwrap();
    assert!(std::ptr::eq(current.function(), foreign_current.function()));
    let wrong_row = Current {
        frontier: current.frontier,
        row: foreign_current.row,
        ordinal: current.ordinal,
        owner: None,
    };
    assert!(!wrong_row.matches(&program, current.function()));
    assert!(
        wrong_row.finish().is_err(),
        "equal HIR pointers do not authorize another phase's row"
    );
    current.finish().unwrap();
    frontier.advance().unwrap();
    drop(foreign_current);
}
#[test]
fn current_row_drop_releases_actual_member_backing_without_refunding_debits() {
    let program = program(HELPERS);
    let ((capacities, observed), refused, debit) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            crate::test_allocation_observer::measure(|| {
                let mut frontier = Frontier::new(&program);
                let current = frontier.current().unwrap();
                current.sites().unwrap();
                assert!(current.has(Family::ByteSet));
                let held = frontier.owned_capacity_bytes();
                let remaining = crate::bounded_output::active_remaining().unwrap();
                current.finish().unwrap();
                frontier.advance().unwrap();
                assert!(frontier.row.get().is_none());
                let phase_only = frontier.owned_capacity_bytes();
                assert_eq!(
                    crate::bounded_output::active_remaining(),
                    Some(remaining),
                    "Drop cannot refund cumulative admission"
                );
                drop(frontier);
                (held, phase_only)
            })
        });
    assert!(!refused);
    assert!(debit > 0);
    assert!(
        capacities.0 > capacities.1,
        "member family storage is released at advance"
    );
    assert!(
        capacities.1 > 0,
        "the phase retains its independently owned census cursor"
    );
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
