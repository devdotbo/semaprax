use super::*;
const HELPERS: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
fn program(source: &str) -> ResolvedProgram {
    let source = crate::check(source, "validation-frontier.spx").unwrap();
    crate::hir::resolve(&source).unwrap()
}
#[test]
fn deferred_frontier_seals_exact_current_member_and_never_forces_future_cells() {
    let program = program(HELPERS);
    let frontier = Frontier::new(&program);
    assert!(frontier.head.get().is_none());
    assert_eq!(frontier.proof_count.get(), 0);
    let current = frontier.current().unwrap();
    current.sites().unwrap();
    current.has(Family::ByteSet);
    assert!(current.row.next.get().is_none());
    frontier.advance(&current).unwrap();
    assert!(!current.matches(&program, &program.functions[0]));
    assert!(current.sites().is_err());
    assert!(frontier.advance(&current).is_err());
}
#[test]
fn deferred_frontier_reuses_exact_positive_target_proofs() {
    let program = program(HELPERS);
    let frontier = Frontier::new(&program);
    for function in &program.functions {
        let current = frontier.current().unwrap();
        let expected = checked_bindings(&program, function).unwrap();
        let actual = current.sites().unwrap();
        for (at, binding) in expected {
            assert!(std::ptr::eq(actual.get(at).unwrap(), binding));
        }
        frontier.advance(&current).unwrap();
    }
    assert_eq!(frontier.proof_count.get(), 2);
}
