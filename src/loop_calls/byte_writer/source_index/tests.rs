use super::*;
const SOURCE: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
fn parsed(source: &str) -> Program {
    crate::parse(source, "source-helper-index.spx").unwrap()
}
fn relay(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|f| f.name == "relay")
        .unwrap()
}
#[test]
fn source_phase_reuses_proof_cursor_and_duplicate_target_without_new_backing() {
    let program = parsed(SOURCE);
    let index = SourceIndex::new(&program);
    assert!(index.target(relay(&program)));
    assert_eq!(index.proofs.borrow().entries.len(), 1);
    let (repeated, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || index.target(relay(&program)));
    assert!(repeated);
    assert!(!overflowed);
    assert_eq!(used, 0);
    let leaf = program.functions.iter().find(|f| f.name == "leaf").unwrap();
    assert!(index.target(leaf));
    assert_eq!(index.proofs.borrow().entries.len(), 2);
    assert!(index.proofs.borrow().cursor.is_some());
    assert!(index.owned_capacity_bytes() > 0);
    assert!(crate::check(SOURCE, "source-index-admitted.spx").is_ok());
    assert!(
        !index.target(&relay(&program).clone()),
        "copied declaration metadata is not the defining module's body"
    );
}
#[test]
fn source_index_exact_and_one_short_reservations_keep_failure_sticky() {
    let program = parsed(SOURCE);
    let (admitted, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            SourceIndex::new(&program).target(relay(&program))
        });
    assert!(admitted);
    assert!(!overflowed);
    assert!(required > 0);
    let index = SourceIndex::new(&program);
    let (admitted, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || index.target(relay(&program)));
    assert!(admitted);
    assert!(!overflowed);
    assert_eq!(used, required);
    let short = SourceIndex::new(&program);
    let (admitted, overflowed, _) =
        crate::bounded_output::with_limit_usage(required - 1, || short.target(relay(&program)));
    assert!(!admitted);
    assert!(overflowed);
    assert_eq!(short.failure().unwrap().code, "SPX-G171");
    assert!(
        short.proofs.borrow().cursor.is_none(),
        "refused cursor backing was never allocated"
    );
    assert!(
        !short.target(relay(&program)),
        "a later larger budget cannot repair this phase failure"
    );
    let empty = SourceIndex::new(&program);
    let (admitted, _, _) =
        crate::bounded_output::with_limit_usage(0, || empty.target(relay(&program)));
    assert!(!admitted);
    assert_eq!(
        empty.proofs.borrow().entries.capacity(),
        0,
        "first reservation refuses before any backing allocation"
    );
}
#[test]
fn source_phase_keeps_irrelevant_modules_allocation_free_and_reproves_changed_bodies() {
    let plain = parsed("module plain; @id(\"main\") fn main()->i64 {0}");
    let index = SourceIndex::new(&plain);
    let (_, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || index.target(&plain.functions[0]));
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(index.owned_capacity_bytes(), 0);
    for source in [
        SOURCE.replace(
            "requires index<byte_len(bytes_as_slice(output))",
            "requires string_len(\"allocated\")==9",
        ),
        SOURCE.replace(
            "next=bytes_set(next,index,value);",
            "next=leaf(next,index,value);",
        ),
        SOURCE.replace(
            "next=bytes_set(next,index,value);",
            "let ignored=missing(); next=bytes_set(next,index,value);",
        ),
    ] {
        let program = parsed(&source);
        let index = SourceIndex::new(&program);
        assert!(!index.target(relay(&program)));
        assert!(
            index.failure().is_none(),
            "ordinary closure refusal is distinct from reservation failure"
        );
        assert!(crate::check(&source, "source-index-hostile.spx").is_err());
    }
}
