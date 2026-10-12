use super::*;
const SOURCE: &str = r#"module checked.allocation;
@id("checked.text") fn text() -> string { string_concat("a", "b") }
@id("checked.main") fn main() -> i64 { let text = text(); string_len(text) }
"#;
fn source() -> Program { crate::parse(SOURCE, "checked-allocation.spx").unwrap() }

#[test]
fn preparation_holds_original_source_full_ordinary_proof_and_exact_current_functions() {
    let source = source();
    let entry = DeclarationId::new("checked.main");
    let prepared = prepare(&source, &entry).expect("full current source proof");
    assert!(std::ptr::eq(prepared.source(), &source));
    assert_eq!(prepared.entry(), &entry);
    assert!(hir::validate(prepared.ordinary_program()).is_ok());
    let execution = FunctionExecutionId::Monomorphic(DeclarationId::new("checked.text"));
    let foreign = hir::resolve(&source).unwrap();
    prepared.with_function(&execution, |current| {
        assert!(current.held_by(prepared.ordinary_program(), &prepared.ordinary_program().functions[0]));
        assert!(!current.held_by(&foreign, &foreign.functions[0]));
        assert!(!current.held_by(prepared.ordinary_program(), &foreign.functions[0]));
        assert_eq!(current.function().id.as_str(), "checked.text");
        assert!(std::ptr::eq(current.program(), prepared.ordinary_program()));
        // Preparation has not reinterpreted any ordinary plan as v20.
        assert!(current.program().functions.iter().all(|f| f.cleanup_plan.schema != super::super::CLEANUP_PLAN_SCHEMA_V20));
    }).unwrap();
    assert!(prepared.with_function(&FunctionExecutionId::Monomorphic(DeclarationId::new("foreign")), |_| ()).is_err());
}

#[test]
fn preparation_refuses_escaped_results_and_preserves_source_ownership_diagnostics() {
    let source = source();
    assert!(matches!(prepare(&source, &DeclarationId::new("checked.text")), Err(Refusal::Boundary(_))));
    let bad = crate::parse("module checked.bad; @id(\"checked.bad.main\") fn main() -> i64 { let text=\"x\"; let first=string_take(text); string_len(text) }", "checked-bad.spx").unwrap();
    let old = hir::resolve(&bad).unwrap_err();
    let Err(Refusal::Source(actual)) = prepare(&bad, &DeclarationId::new("checked.bad.main")) else { panic!("must retain full original source refusal") };
    assert_eq!(actual, old);
}

#[test]
fn candidate_allocation_branches_precede_initialization_and_preserve_commit_ownership() {
    use crate::cleanup_plan::{AllocationCase, AllocationOperation, CleanupTerminator,
        CleanupTransition, EdgeCondition, ExitContinuation, StatusLane, StatusProducer};
    let source = source();
    let prepared = prepare(&source, &DeclarationId::new("checked.main")).unwrap();
    prepared.with_function(&FunctionExecutionId::Monomorphic(DeclarationId::new("checked.text")), |current| {
        let old = super::super::build::build_plan(current.program(), current.function()).unwrap();
        assert_eq!(old, current.function().cleanup_plan, "ordinary canonical attachment remains exact");
        let candidate = super::super::build::checked_allocation::candidate(current).unwrap();
        assert_eq!(candidate.schema, super::super::CLEANUP_PLAN_SCHEMA_V20);
        assert_eq!(candidate.slots, old.slots);
        assert_eq!(candidate.entry_state, old.entry_state);
        let allocation_sources: Vec<_> = candidate.status_sources.iter()
            .filter(|source| source.id.lane == StatusLane::AllocationFailure).collect();
        assert_eq!(allocation_sources.len(), 3, "two literal headers and one concatenation");
        for source in allocation_sources {
            let StatusProducer::CheckedAllocation { operation, normalized_cases } = &source.producer else {
                panic!("closed allocation provenance required")
            };
            assert_eq!(*normalized_cases, [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow]);
            let edge = candidate.edges.iter().find(|edge| edge.condition == EdgeCondition::StatusNonzero(source.id.clone())).unwrap();
            let failure = &candidate.blocks[edge.to.0 as usize];
            assert_eq!(failure.transitions, vec![CleanupTransition::SelectFailure { source: source.id.clone() }]);
            let CleanupTerminator::Exit(exit) = &failure.terminator else { panic!("canonical failure exit") };
            let exit = &candidate.exits[exit.0 as usize];
            assert_eq!(exit.continuation, ExitContinuation::ReturnFailure { source: source.id.clone() });
            if *operation == AllocationOperation::StringConcat {
                // Concat has already accepted its staged group. Its allocator
                // adapter must settle that callee group on physical refusal;
                // this failure exit cannot reacquire caller-owned arguments.
                assert!(exit.finalize_in_order.is_empty());
                assert!(candidate.blocks.iter().any(|block| block.transitions.iter().any(|transition|
                    matches!(transition, CleanupTransition::CallCommit { call, arguments } if call == &source.id.expression && arguments.len() == 2))));
            }
        }
        assert_eq!(current.function().cleanup_plan, old, "candidate does not replace authoritative ordinary plan");
    }).unwrap();
}


#[test]
fn independent_allocation_status_replay_rejects_missing_swapped_and_invented_rows() {
    use crate::cleanup_plan::{AllocationCase, AllocationOperation, StatusLane, StatusProducer};
    let source = source();
    let prepared = prepare(&source, &DeclarationId::new("checked.main")).unwrap();
    prepared.with_function(&FunctionExecutionId::Monomorphic(DeclarationId::new("checked.text")), |current| {
        let candidate = super::super::build::checked_allocation::candidate(current).unwrap();
        let validate = |plan: &crate::cleanup_plan::CleanupPlan| {
            super::super::replay::ordinary_status::validate_allocation_projection(current, plan)
        };
        assert!(validate(&candidate).is_ok(), "literal and concat producers independently discovered");
        assert!(validate(&current.function().cleanup_plan).is_err(), "ordinary attachment is not v20 evidence");
        let position = candidate.status_sources.iter().position(|row| row.id.lane == StatusLane::AllocationFailure).unwrap();
        let mut missing = candidate.clone();
        missing.status_sources.remove(position);
        assert!(validate(&missing).is_err());
        let mut duplicate = candidate.clone();
        duplicate.status_sources.insert(position, candidate.status_sources[position].clone());
        assert!(validate(&duplicate).is_err());
        let mut wrong = candidate.clone();
        wrong.status_sources[position].producer = StatusProducer::CheckedAllocation {
            operation: AllocationOperation::BytesCopy,
            normalized_cases: [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow],
        };
        assert!(validate(&wrong).is_err());
        let mut swapped = candidate.clone();
        let StatusProducer::CheckedAllocation { normalized_cases, .. } = &mut swapped.status_sources[position].producer else { panic!("allocation row") };
        normalized_cases.swap(0, 1);
        assert!(validate(&swapped).is_err());
        let mut wrong_lane = candidate.clone();
        wrong_lane.status_sources[position].id.lane = StatusLane::OperationFailure;
        assert!(validate(&wrong_lane).is_err());
        assert_eq!(current.function().cleanup_plan, super::super::build::build_plan(current.program(), current.function()).unwrap());
    }).unwrap();
}

#[test]
fn independent_allocation_status_replay_keeps_take_total_and_implicit_reads_cloning() {
    use crate::cleanup_plan::{AllocationOperation, StatusProducer};
    let source = crate::parse(r#"module checked.read;
@id("checked.read.main") fn main() -> i64 {
    let text = "abc";
    let copy = text;
    let moved = string_take(text);
    string_len(copy) + string_len(moved)
}
"#, "checked-read.spx").unwrap();
    let prepared = prepare(&source, &DeclarationId::new("checked.read.main")).unwrap();
    prepared.with_function(&FunctionExecutionId::Monomorphic(DeclarationId::new("checked.read.main")), |current| {
        let candidate = super::super::build::checked_allocation::candidate(current).unwrap();
        assert!(super::super::replay::ordinary_status::validate_allocation_projection(current, &candidate).is_ok());
        let operations: Vec<_> = candidate.status_sources.iter().filter_map(|row| match row.producer {
            StatusProducer::CheckedAllocation { operation, .. } => Some(operation),
            _ => None,
        }).collect();
        assert_eq!(operations, vec![AllocationOperation::StringLiteral, AllocationOperation::StringClone],
            "take forwards the original carrier; an ordinary owning read still clones");
    }).unwrap();
}
