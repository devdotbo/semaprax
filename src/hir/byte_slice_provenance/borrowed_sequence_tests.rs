//! Full original cloned-input provenance oracle and actual removed backing.
use super::*;
use crate::hir::ResolvedProgram;
use crate::test_allocation_observer::measure;
const SOURCE: &str = r#"module borrowed.provenance;
@id("inspect") fn inspect(payload:own Bytes)->usize {
 let view=bytes_as_slice(payload);byte_len(view)+byte_len(view)
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(0)}
"#;
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "borrowed-provenance.spx").unwrap()).unwrap()
}
fn cloned_directory(program: &ResolvedProgram) -> Vec<ResolvedFunction> {
    let mut cleanup_functions = program.functions.clone();
    cleanup_functions.extend(
        program
            .function_instances
            .iter()
            .map(|i| i.function.clone()),
    );
    cleanup_functions
}
fn old(program: &ResolvedProgram) -> Result<BTreeMap<ValueId, ByteSliceProvenance>, Diagnostic> {
    let cleanup_functions = cloned_directory(program);
    derive_byte_slice_provenance(&cleanup_functions, &program.declarations)
}
fn borrowed(
    program: &ResolvedProgram,
) -> Result<BTreeMap<ValueId, ByteSliceProvenance>, Diagnostic> {
    derive_byte_slice_provenance_from(
        &mut program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function)),
        &program.declarations,
    )
}
#[test]
fn borrowed_complete_authored_instance_scan_preserves_owned_oracle_and_removed_requests() {
    let program = fixture();
    assert_eq!(program.function_instances.len(), 1);
    let expected = old(&program).unwrap();
    assert!(!expected.is_empty());
    assert_eq!(borrowed(&program).unwrap(), expected);
    let ((), old_metrics) = measure(|| drop(old(&program).unwrap()));
    let ((), current_metrics) = measure(|| drop(borrowed(&program).unwrap()));
    let ((), removed) = measure(|| drop(cloned_directory(&program)));
    for metrics in [old_metrics, current_metrics, removed] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
    assert!(removed.allocated > 0);
    assert_eq!(
        old_metrics.allocated - current_metrics.allocated,
        removed.allocated
    );
    assert_eq!(
        old_metrics.allocations - current_metrics.allocations,
        removed.allocations
    );
    assert_eq!(
        old_metrics.reallocations - current_metrics.reallocations,
        removed.reallocations
    );
}
#[test]
fn borrowed_provenance_preserves_first_illegal_result_view_diagnostic() {
    let mut program = fixture();
    let first = &mut program.functions[0];
    first.return_type = ResolvedType::Bytes;
    first.body.kind = ResolvedExprKind::BorrowPlace {
        operation: crate::hir::DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
        place: Place {
            root: first.result_id.clone(),
            projections: Vec::new(),
        },
    };
    let expected = old(&program).unwrap_err();
    let actual = borrowed(&program).unwrap_err();
    assert_eq!(actual.code, "SPX-H006");
    assert_eq!(
        actual.message,
        "provisional result view is outside its postcondition domain"
    );
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.path, expected.path);
    assert_eq!(actual.span, expected.span);
    assert_eq!(actual.help, expected.help);
}
