//! Contract loops have exact ordinary/Return proof domains and live contours.
use super::*;
use crate::cleanup_plan::ExecutionBoundaryKind;

const SOURCE: &str = r#"module test.contract_loop_boundaries;
@id("run") fn run(n:i64)->i64
 requires { let mut i=0; while i<1 { i=i+1; 0 } i==1 }
 ensures { let mut i=0; while i<2 { i=i+1; 0 } i==2 }
{
 let owner="live root";
 if n==1 { return 11; };
 if n==2 { return 12; };
 13
}
@id("main") fn main()->i64 {0}
"#;

fn fixture() -> (ResolvedProgram, ResolvedFunction) {
    let program =
        hir::resolve(&parse(SOURCE, Path::new("contract-loop-boundaries.spx")).unwrap()).unwrap();
    let function = function(&program, "run");
    validate_structure(&program, &function)
        .expect("typed contract loops replay in their exact domains");
    (program, function)
}

#[test]
fn contract_loops_cover_requires_ordinary_and_each_distinct_return_domain() {
    let (_, function) = fixture();
    let plan = &function.cleanup_plan;
    let control = plan.lexical_control.as_deref().unwrap();
    assert_eq!(control.execution_boundaries.len(), 5);
    assert_eq!(
        control.execution_boundaries[0].kind,
        ExecutionBoundaryKind::FunctionEntry
    );
    let sites = control
        .execution_boundaries
        .iter()
        .skip(2)
        .map(|item| &item.site)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        sites.len(),
        1,
        "one authored ensures loop is independently replicated three times"
    );
    for boundary in control.execution_boundaries.iter().skip(3) {
        let resumes = control
            .resumes
            .iter()
            .filter(|resume| resume.blocks.contains(&boundary.block))
            .collect::<Vec<_>>();
        assert_eq!(resumes.len(), 1);
        assert!(control
            .exits
            .iter()
            .any(|exit| exit.kind == crate::ast::LexicalExitKind::Return
                && exit.resume == Some(resumes[0].entry)));
        assert!(
            !boundary.finalize_in_order.is_empty(),
            "live root owner survives to the selected ensures loop"
        );
    }
}

#[test]
fn replicated_contract_boundary_cannot_borrow_another_return_contour_or_order() {
    let (program, baseline) = fixture();
    let mut changed = baseline.clone();
    changed
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap()
        .execution_boundaries
        .swap(3, 4);
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = baseline.clone();
    let control = changed.cleanup_plan.lexical_control.as_deref_mut().unwrap();
    control.execution_boundaries[3].block = control.execution_boundaries[4].block;
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = baseline.clone();
    changed
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap()
        .execution_boundaries[3]
        .finalize_in_order
        .clear();
    assert!(validate_structure(&program, &changed).is_err());
}

#[test]
fn requires_boundary_cannot_be_replaced_by_ensures_or_omitted() {
    let (program, baseline) = fixture();
    let mut changed = baseline.clone();
    let control = changed.cleanup_plan.lexical_control.as_deref_mut().unwrap();
    control.execution_boundaries.swap(1, 2);
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = baseline;
    changed
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap()
        .execution_boundaries
        .remove(1);
    assert!(validate_structure(&program, &changed).is_err());
}

#[test]
fn owned_return_boundary_keeps_exact_provisional_result_in_its_failure_contour() {
    let source = r#"module test.owned_contract_loop;
@id("run") fn run()->Bytes
 ensures { let mut i=0; while i<2 { i=i+1; 0 } byte_len(bytes_as_slice(result))==2usize }
{ return bytes_zeroed(2usize); }
@id("main") fn main()->i64 {0}
"#;
    let program =
        hir::resolve(&parse(source, Path::new("owned-contract-loop.spx")).unwrap()).unwrap();
    let baseline = function(&program, "run");
    validate_structure(&program, &baseline)
        .expect("owned result survives its exact Return ensures loop");
    let control = baseline.cleanup_plan.lexical_control.as_deref().unwrap();
    assert_eq!(control.execution_boundaries.len(), 2);
    assert!(control.execution_boundaries[1]
        .finalize_in_order
        .iter()
        .any(|action| { action.source.storage == StorageId::ProvisionalResult }));
    let mut changed = baseline;
    changed
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap()
        .execution_boundaries[1]
        .finalize_in_order
        .clear();
    assert!(validate_structure(&program, &changed).is_err());
}
