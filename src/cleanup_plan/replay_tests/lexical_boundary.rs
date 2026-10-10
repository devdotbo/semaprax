//! Admitted contract calls retain exact per-return proof domains. Inline
//! contract loops/mutation remain closed; loops execute in checked callees.
use super::*;
use crate::cleanup_plan::{ContractPhase, ExecutionBoundaryKind, StatusProducer};

const SOURCE: &str = r#"module test.contract_loop_boundaries;
@id("check_loop") fn check_loop(n:i64)->bool {
 let mut i=0;
 while i<n { i=i+1; 0 }
 return i==n;
}
@id("run") fn run(n:i64)->i64
 requires check_loop(1)
 ensures check_loop(2)
{
 let owner="live root";
 let mut i=0;
 while i<1 { i=i+1; 0 }
 if n==1 { return 11; };
 if n==2 { return 12; };
 13
}
@id("main") fn main()->i64 {0}
"#;

fn checked(source: &str, path: &str) -> ResolvedProgram {
    let ast = crate::check(source, Path::new(path)).expect("fixture is admitted source");
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    program
}
fn fixture() -> (ResolvedProgram, ResolvedFunction) {
    let program = checked(SOURCE, "contract-loop-boundaries.spx");
    let function = function(&program, "run");
    validate_structure(&program, &function).unwrap();
    (program, function)
}
fn ensures_failures(function: &ResolvedFunction) -> Vec<usize> {
    function
        .cleanup_plan
        .exits
        .iter()
        .enumerate()
        .filter_map(|(index, exit)| {
            let ExitContinuation::ReturnFailure { source } = &exit.continuation else {
                return None;
            };
            function
                .cleanup_plan
                .status_sources
                .iter()
                .any(|item| {
                    &item.id == source
                        && matches!(
                            item.producer,
                            StatusProducer::ContractFalse {
                                phase: ContractPhase::Ensures,
                                ..
                            }
                        )
                })
                .then_some(index)
        })
        .collect()
}

#[test]
fn contract_calls_cover_requires_ordinary_and_each_distinct_return_domain() {
    let (program, function) = fixture();
    let plan = &function.cleanup_plan;
    let control = plan.lexical_control.as_deref().unwrap();
    assert_eq!(control.execution_boundaries.len(), 2);
    assert_eq!(
        control.execution_boundaries[0].kind,
        ExecutionBoundaryKind::FunctionEntry
    );
    assert_eq!(
        control.execution_boundaries[1].kind,
        ExecutionBoundaryKind::LoopIteration
    );
    assert_eq!(control.function_success_exits.len(), 3);
    assert_eq!(
        control
            .exits
            .iter()
            .filter(|exit| exit.kind == crate::ast::LexicalExitKind::Return)
            .map(|exit| exit.resume.unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    let failures = ensures_failures(&function);
    assert_eq!(
        failures.len(),
        3,
        "one authored ensures failure in each exact result domain"
    );
    assert_eq!(
        failures
            .iter()
            .map(|index| plan.exits[*index].from)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert!(
        failures
            .iter()
            .all(|index| !plan.exits[*index].finalize_in_order.is_empty()),
        "live root owner survives into every selected postcondition"
    );
    let helper = super::function(&program, "check_loop");
    validate_structure(&program, &helper).unwrap();
    let boundaries = &helper
        .cleanup_plan
        .lexical_control
        .as_deref()
        .unwrap()
        .execution_boundaries;
    assert_eq!(
        boundaries.len(),
        2,
        "contract loops belong to the authenticated callee"
    );
    assert_eq!(boundaries[1].kind, ExecutionBoundaryKind::LoopIteration);
}

#[test]
fn replicated_contract_domains_cannot_borrow_another_return_contour_or_order() {
    let (program, baseline) = fixture();
    let mut changed = baseline.clone();
    changed
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap()
        .exits
        .swap(0, 1);
    assert!(validate_structure(&program, &changed).is_err());
    let failures = ensures_failures(&baseline);
    let mut changed = baseline.clone();
    changed.cleanup_plan.exits[failures[1]].from = changed.cleanup_plan.exits[failures[2]].from;
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = baseline;
    changed.cleanup_plan.exits[failures[1]]
        .finalize_in_order
        .clear();
    assert!(validate_structure(&program, &changed).is_err());
}

#[test]
fn requires_cannot_be_replaced_by_ensures_or_omitted_and_contract_loops_stay_closed() {
    let (program, baseline) = fixture();
    let mut changed = baseline.clone();
    let requires = changed
        .cleanup_plan
        .status_sources
        .iter()
        .position(|item| {
            matches!(
                item.producer,
                StatusProducer::ContractFalse {
                    phase: ContractPhase::Requires,
                    ..
                }
            )
        })
        .unwrap();
    let ensures = changed
        .cleanup_plan
        .status_sources
        .iter()
        .position(|item| {
            matches!(
                item.producer,
                StatusProducer::ContractFalse {
                    phase: ContractPhase::Ensures,
                    ..
                }
            )
        })
        .unwrap();
    changed.cleanup_plan.status_sources.swap(requires, ensures);
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = baseline;
    changed.cleanup_plan.status_sources.remove(requires);
    assert!(validate_structure(&program, &changed).is_err());
    for clause in ["requires", "ensures"] {
        let source=format!("module closed; @id(\"run\") fn run()->i64 {clause} {{ let mut i=0; while i<1 {{i=i+1;0}} i==1 }} {{0}} @id(\"main\") fn main()->i64{{0}}");
        let errors = crate::check(&source, "closed-contract-loop.spx").unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-T253"));
        assert!(errors.iter().any(|error| error.code == "SPX-U106"));
    }
    let helper = super::function(&program, "check_loop");
    let mut changed = helper.clone();
    changed
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap()
        .execution_boundaries
        .swap(0, 1);
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = helper;
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
fn owned_return_keeps_exact_provisional_result_in_its_postcondition_failure_contour() {
    let source = r#"module test.owned_contract;
@id("run") fn run(n:i64)->Bytes ensures n==0 {
 let mut i=0; while i<1 {i=i+1;0}
 return bytes_zeroed(2usize);
}
@id("main") fn main()->i64{0}
"#;
    let program = checked(source, "owned-contract.spx");
    let baseline = function(&program, "run");
    validate_structure(&program, &baseline).unwrap();
    let failures = ensures_failures(&baseline);
    assert_eq!(failures.len(), 1);
    assert!(baseline.cleanup_plan.exits[failures[0]]
        .finalize_in_order
        .iter()
        .any(|action| action.source.storage == StorageId::ProvisionalResult));
    let mut changed = baseline;
    changed.cleanup_plan.exits[failures[0]]
        .finalize_in_order
        .clear();
    assert!(validate_structure(&program, &changed).is_err());
}

#[test]
fn two_return_postconditions_reuse_exact_owned_call_argument_and_local_slots() {
    let source = r#"module test.owned_ensure_template;
@id("measure") fn measure(first:own Bytes,second:own Bytes)->usize {
 byte_len(bytes_as_slice(first))+byte_len(bytes_as_slice(second))
}
@id("run") fn run(choice:bool)->i64
 ensures {
  let measured=measure(bytes_zeroed(2usize),bytes_zeroed(3usize));
  let text="temporary";
  measured==5usize && string_len(text)==9
 }
{
 let root="root retained";
 let mut i=0; while i<1 {i=i+1;0}
 if choice {return 11;};
 return 12;
}
@id("main") fn main()->i64{0}
"#;
    let program = checked(source, "owned-ensure-template.spx");
    let baseline = function(&program, "run");
    validate_structure(&program, &baseline).unwrap();
    let plan = &baseline.cleanup_plan;
    let arguments = plan
        .slots
        .iter()
        .filter(|slot| matches!(slot.storage, StorageId::CallArgument { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        arguments.len(),
        2,
        "each authored owning argument has one supplemental slot"
    );
    for slot in arguments {
        assert_eq!(
            plan.regions
                .iter()
                .filter(|region| region.slots.contains(&slot.storage))
                .count(),
            1
        );
    }
    let control = plan.lexical_control.as_deref().unwrap();
    let returns = control
        .exits
        .iter()
        .filter(|exit| exit.kind == crate::ast::LexicalExitKind::Return)
        .collect::<Vec<_>>();
    assert_eq!(returns.len(), 2);
    assert_ne!(returns[0].resume, returns[1].resume);
    let failures = ensures_failures(&baseline);
    assert_eq!(failures.len(), 2);
    let mut changed = baseline.clone();
    changed.cleanup_plan.exits[failures[0]].from = changed.cleanup_plan.exits[failures[1]].from;
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = baseline;
    let region = changed
        .cleanup_plan
        .regions
        .iter()
        .find(|region| {
            region
                .slots
                .iter()
                .any(|slot| matches!(slot, StorageId::CallArgument { .. }))
        })
        .unwrap();
    let endpoint = region.normal_scope_end;
    changed.cleanup_plan.exits[endpoint.0 as usize]
        .leaves_regions
        .clear();
    assert!(validate_structure(&program, &changed).is_err());
}
