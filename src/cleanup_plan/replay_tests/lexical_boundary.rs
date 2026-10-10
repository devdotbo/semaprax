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
    // The original source is deliberately normalized to ordinary control flow.
    // Only actual loop returns require distinct lexical result domains.
    let source = SOURCE.replace(
        "while i<1 { i=i+1; 0 }\n if n==1 { return 11; };\n if n==2 { return 12; };",
        "while i<1 { i=i+1; if n==1 { return 11; }; if n==2 { return 12; }; 0 }",
    );
    let program = checked(&source, "contract-loop-boundaries.spx");
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
fn original_acyclic_returns_keep_ordinary_normalization_graph_and_cache_replay() {
    let ast = crate::check(SOURCE, "ordinary-contract-boundaries.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let replay = crate::check(&canonical, "ordinary-contract-boundaries.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&replay));
    assert_eq!(
        crate::graph::to_json(&ast).unwrap(),
        crate::graph::to_json(&replay).unwrap()
    );
    let mut program = checked(SOURCE, "ordinary-contract-boundaries.spx");
    let run = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "run")
        .unwrap();
    assert!(run.cleanup_plan.lexical_control.is_none());
    let encoded = crate::cache_codec::encode(&run.cleanup_plan).unwrap();
    run.cleanup_plan = crate::cache_codec::decode(&encoded).unwrap();
    assert_eq!(
        encoded,
        crate::cache_codec::encode(&run.cleanup_plan).unwrap()
    );
    hir::validate(&program).unwrap();
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
    assert!(
        helper.cleanup_plan.lexical_control.is_none(),
        "ordinary terminal-return callee retains its legacy profile"
    );
    let ResolvedExprKind::Call {
        callee: required, ..
    } = &function.requires[0].kind
    else {
        panic!("requires directly calls its checked helper");
    };
    let ResolvedExprKind::Call {
        callee: ensured, ..
    } = &function.ensures[0].kind
    else {
        panic!("ensures directly calls its checked helper");
    };
    assert_eq!(required, &helper.id);
    assert_eq!(ensured, &helper.id);
    let calls = plan
        .status_sources
        .iter()
        .filter(|source| {
            matches!(&source.producer,
        StatusProducer::PropagatedCall { callee } if callee == &helper.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        2,
        "each authored call retains one exact propagated producer"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|source| source.id.expression == function.requires[0].id)
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|source| source.id.expression == function.ensures[0].id)
            .count(),
        1
    );
    for (expression, expected_count) in
        [(&function.requires[0].id, 1), (&function.ensures[0].id, 3)]
    {
        assert_eq!(
            plan.exits
                .iter()
                .filter(|exit| {
                    matches!(&exit.continuation, ExitContinuation::ReturnFailure { source }
                if &source.expression == expression
                    && source.lane == crate::cleanup_plan::StatusLane::OperationFailure)
                })
                .count(),
            expected_count,
            "call failure routes retain each exact contract domain"
        );
    }
}

#[test]
fn replicated_contract_domains_cannot_borrow_another_return_contour_or_order() {
    let (program, baseline) = fixture();
    let (_, repeated) = fixture();
    assert_eq!(
        baseline.cleanup_plan.status_sources, repeated.cleanup_plan.status_sources,
        "normal success and two Return ensures domains preserve canonical producer order"
    );
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
    let mut changed = baseline.clone();
    changed.cleanup_plan.exits[failures[1]]
        .finalize_in_order
        .clear();
    assert!(validate_structure(&program, &changed).is_err());
    assert!(baseline.cleanup_plan.status_sources.len() >= 2);
    let mut changed = baseline;
    changed.cleanup_plan.status_sources.swap(0, 1);
    assert!(
        validate_structure(&program, &changed).is_err(),
        "replicated ensures do not grant authority to repair canonical status order"
    );
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
        let source = format!(
            "module closed; @id(\"run\") fn run()->i64 {clause} {{ let mut i=0; while i<1 {{i=i+1;0}} i==1 }} {{0}} @id(\"main\") fn main()->i64{{0}}"
        );
        let errors = crate::check(&source, "closed-contract-loop.spx").unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-T253"));
        assert!(errors.iter().any(|error| error.code == "SPX-U106"));
    }
    let caller = super::function(&program, "run");
    let mut changed = caller.clone();
    changed
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap()
        .execution_boundaries
        .swap(0, 1);
    assert!(validate_structure(&program, &changed).is_err());
    let mut changed = caller;
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
fn two_return_postconditions_reuse_owned_temporaries_and_refuse_resource_transfers() {
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
    let errors = crate::check(source, Path::new("closed-owning-contract.spx")).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.code == "SPX-O105" && error.message.contains("measure")));
    assert_eq!(
        errors
            .iter()
            .filter(|error| error.code == "SPX-O105" && error.message.contains("measure"))
            .count(),
        2,
        "both inline constructors remain closed; only named fresh locals may transfer"
    );
    assert!(
        !errors
            .iter()
            .any(|error| error.code == "SPX-O105" && error.message.contains("local binding")),
        "the literal String local is independently fresh in this clause"
    );
    let source = source.replace(
        "let measured=measure(bytes_zeroed(2usize),bytes_zeroed(3usize));\n  let text=\"temporary\";\n  measured==5usize && string_len(text)==9",
        "let measured=string_len(\"first\")+string_len(\"second\");\n  measured==11",
    );
    let ordinary = checked(&source, "ordinary-owned-ensure-template.spx");
    let ordinary_run = function(&ordinary, "run");
    assert!(ordinary_run.cleanup_plan.lexical_control.is_none());
    validate_structure(&ordinary, &ordinary_run).unwrap();
    let source = source.replace(
        "while i<1 {i=i+1;0}\n if choice {return 11;};\n return 12;",
        "while i<1 {i=i+1;if choice {return 11;};return 12;}12",
    );
    let program = checked(&source, "owned-ensure-template.spx");
    let baseline = function(&program, "run");
    validate_structure(&program, &baseline).unwrap();
    let plan = &baseline.cleanup_plan;
    let mut expected = BTreeSet::new();
    let mut pending = vec![&baseline.ensures[0]];
    while let Some(expression) = pending.pop() {
        if matches!(&expression.kind, ResolvedExprKind::String(_)) {
            expected.insert(expression.id.clone());
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    assert_eq!(expected.len(), 2);
    let temporaries = plan
        .slots
        .iter()
        .filter(|slot| {
            matches!(&slot.storage,
        StorageId::Temporary(expression) if expected.contains(expression))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        temporaries.len(),
        2,
        "each authored owned temporary has one exact slot across domains"
    );
    for slot in temporaries {
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
    assert_eq!(
        failures.len(),
        3,
        "two loop returns and the ordinary loop-zero result"
    );
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
                .any(|slot| matches!(slot, StorageId::Temporary(expression) if expected.contains(expression)))
        })
        .unwrap();
    let endpoint = region.normal_scope_end;
    changed.cleanup_plan.exits[endpoint.0 as usize]
        .leaves_regions
        .clear();
    assert!(validate_structure(&program, &changed).is_err());
}

#[test]
fn fresh_contract_local_owner_regions_replay_in_all_selected_return_domains() {
    let source = r#"module test.fresh_contract_regions;
@id("inspect") fn inspect(input:own Bytes)->bool {byte_len(bytes_as_slice(input))==2usize}
@id("run") fn run(n:i64)->i64
 requires {let required=bytes_zeroed(2usize);inspect(required)}
 ensures {let checked=bytes_zeroed(3usize);let text="ok";string_len(text)==2 && byte_len(bytes_as_slice(checked))==3usize}
{
 let root="live root";let mut i=0;
 while i<1 {i=i+1;if n==1 {return 11;};if n==2 {return 12;};0}
 13
}
@id("main") fn main()->i64{0}
"#;
    let program = checked(source, "fresh-contract-regions.spx");
    let baseline = function(&program, "run");
    validate_structure(&program, &baseline).unwrap();
    let failures = ensures_failures(&baseline);
    assert_eq!(failures.len(), 3);
    let ResolvedExprKind::Block { statements, .. } = &baseline.ensures[0].kind else {
        panic!("ensures block")
    };
    let locals = statements
        .iter()
        .filter_map(|statement| match statement {
            hir::ResolvedStatement::Let { binding, .. } => {
                Some(StorageId::Value(binding.id.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 2);
    for local in &locals {
        assert_eq!(
            baseline
                .cleanup_plan
                .slots
                .iter()
                .filter(|slot| &slot.storage == local)
                .count(),
            1
        );
        let region = baseline
            .cleanup_plan
            .regions
            .iter()
            .find(|region| region.slots.contains(local))
            .unwrap();
        assert!(
            baseline.cleanup_plan.exits[region.normal_scope_end.0 as usize]
                .leaves_regions
                .contains(&region.id)
        );
        let mut forged = baseline.clone();
        forged.cleanup_plan.exits[region.normal_scope_end.0 as usize]
            .leaves_regions
            .clear();
        assert!(validate_structure(&program, &forged).is_err());
    }
    for index in failures {
        let mut forged = baseline.clone();
        forged.cleanup_plan.exits[index].finalize_in_order.clear();
        assert!(
            validate_structure(&program, &forged).is_err(),
            "a failed ensures must retain root-owner settlement"
        );
    }
    let mut cached = program.clone();
    let run = cached
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "run")
        .unwrap();
    let bytes = crate::cache_codec::encode(&run.cleanup_plan).unwrap();
    run.cleanup_plan = crate::cache_codec::decode(&bytes).unwrap();
    assert_eq!(
        bytes,
        crate::cache_codec::encode(&run.cleanup_plan).unwrap()
    );
    hir::validate(&cached).unwrap();
}
