//! Independent small witnesses for nested targets, owned joins and contracts.
use super::super::lexical_plan_tests;
use super::{with_strategy, Strategy};
use crate::cleanup_plan::replay::validate_structure;
use crate::cleanup_plan::{CleanupResultSource, ContractPhase, ExitContinuation, StatusProducer};
use crate::hir::{ResolvedFunction, ResolvedProgram};

const NESTED: &str = r#"module factored.coverage_nested;
@id("probe") fn probe(n:i64)->i64 {
 let mut outer=0;
 let mut total=0;
 while outer<n {
  outer=outer+1;
  let mut inner=0;
  while inner<2 {
   inner=inner+1;
   if inner==1 {continue;};
   if n==0 {break;};
   total=total+inner;
   0
  }
  if outer==2 {continue;};
  if total>4 {return total;};
  0
 }
 return total;
}
@id("main") fn main()->i64 {0}
"#;

const OWNED: &str = r#"module factored.coverage_owned;
@id("probe") fn probe(n:i64)->Bytes {
 let payload=if n==0 {bytes_zeroed(1usize)}else{bytes_zeroed(2usize)};
 let mut i=0;
 while i<1 {
  i=i+1;
  if n==1 {return payload;};
  0
 }
 payload
}
@id("main") fn main()->i64 {0}
"#;

const CONTRACTS: &str = r#"module factored.coverage_contracts;
@id("check_loop") fn check_loop(n:i64)->bool {
 let mut i=0;
 while i<n {i=i+1;0}
 return i==n;
}
@id("probe") fn probe(n:i64)->i64
 requires check_loop(1)
 ensures check_loop(2)
{
 let owner="live root";
 let mut i=0;
 while i<1 {
  i=i+1;
  if n==1 {return 11;};
  if n==2 {return 12;};
  0
 }
 13
}
@id("main") fn main()->i64 {0}
"#;

fn witness(source: &str) -> (ResolvedProgram, ResolvedFunction) {
    let (program, function) = lexical_plan_tests::fixture(source);
    crate::hir::validate(&program).unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    let unchanged = function.cleanup_plan.clone();
    for strategy in [Strategy::Enumerated, Strategy::LexicalFactored] {
        with_strategy(strategy, || validate_structure(&program, &function)).unwrap();
        assert_eq!(function.cleanup_plan, unchanged);
    }
    // Independently expand HIR Cartesian paths and enumerate the plan's small
    // automaton. Neither oracle calls weighted language equality, and both
    // preserve duplicate observation traces and terminal classes.
    lexical_plan_tests::assert_copy_scope_oracle(source);
    lexical_plan_tests::assert_plan_oracle(&function);
    (program, function)
}

fn rejected(program: &ResolvedProgram, function: &ResolvedFunction) {
    for strategy in [Strategy::Enumerated, Strategy::LexicalFactored] {
        assert_eq!(
            with_strategy(strategy, || validate_structure(program, function))
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
}

#[test]
fn forced_nested_loops_preserve_nearest_targets_and_complete_multisets() {
    let (program, function) = witness(NESTED);
    let control = function.cleanup_plan.lexical_control.as_ref().unwrap();
    assert_eq!(control.loops.len(), 2);
    let outer = control.loops[0].target.clone();
    let inner = control.loops[1].target.clone();
    assert_ne!(outer, inner);
    assert!(control.exits.iter().any(|exit| {
        exit.target == inner && exit.kind == crate::ast::LexicalExitKind::Continue
    }));
    let mut forged = function.clone();
    let exit = forged
        .cleanup_plan
        .lexical_control
        .as_mut()
        .unwrap()
        .exits
        .iter_mut()
        .find(|exit| exit.target == inner && exit.kind == crate::ast::LexicalExitKind::Continue)
        .unwrap();
    exit.target = outer.clone();
    rejected(&program, &forged);
    let mut forged = function.clone();
    let exit = forged
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| matches!(&exit.continuation, ExitContinuation::LoopLatch {target} if target == &inner))
        .unwrap();
    exit.continuation = ExitContinuation::LoopLatch { target: outer };
    rejected(&program, &forged);
}

#[test]
fn forced_conditional_owned_results_preserve_transfer_and_publication() {
    let (program, function) = witness(OWNED);
    assert_eq!(function.return_type, crate::hir::ResolvedType::Bytes);
    let owned_commit = function
        .cleanup_plan
        .exits
        .iter()
        .position(|exit| {
            matches!(
                exit.continuation,
                ExitContinuation::CommitResult {
                    source: CleanupResultSource::Owned { .. }
                }
            )
        })
        .expect("actual owned result publication is represented");
    let mut forged = function.clone();
    forged.cleanup_plan.exits[owned_commit].continuation = ExitContinuation::CommitResult {
        source: CleanupResultSource::Scalar {
            expression: function.body.id.clone(),
        },
    };
    rejected(&program, &forged);
    let mut forged = function.clone();
    let control = forged.cleanup_plan.lexical_control.as_mut().unwrap();
    let success = *control.function_success_exits.first().unwrap();
    control.function_success_exits.push(success);
    rejected(&program, &forged);
}

#[test]
fn forced_ensures_domains_preserve_failure_cleanup_and_return_multiplicity() {
    let (program, function) = witness(CONTRACTS);
    assert_eq!(
        function
            .cleanup_plan
            .lexical_control
            .as_ref()
            .unwrap()
            .function_success_exits
            .len(),
        3
    );
    let ensures = function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|source| {
            matches!(
                source.producer,
                StatusProducer::ContractFalse {
                    phase: ContractPhase::Ensures,
                    ..
                }
            )
        })
        .unwrap();
    let failures: Vec<_> = function
        .cleanup_plan
        .exits
        .iter()
        .enumerate()
        .filter(|(_, exit)| matches!(&exit.continuation, ExitContinuation::ReturnFailure {source} if function.cleanup_plan.status_sources.iter().any(|item| &item.id == source && matches!(item.producer, StatusProducer::ContractFalse {phase: ContractPhase::Ensures, ..}))))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(failures.len(), 3);
    for index in &failures {
        assert!(!function.cleanup_plan.exits[*index]
            .finalize_in_order
            .is_empty());
    }
    let mut forged = function.clone();
    forged.cleanup_plan.exits[failures[0]]
        .finalize_in_order
        .pop()
        .unwrap();
    rejected(&program, &forged);
    let mut forged = function.clone();
    let source = forged
        .cleanup_plan
        .status_sources
        .iter_mut()
        .find(|source| source.id == ensures.id)
        .unwrap();
    let StatusProducer::ContractFalse { phase, .. } = &mut source.producer else {
        unreachable!()
    };
    *phase = ContractPhase::Requires;
    rejected(&program, &forged);
}
