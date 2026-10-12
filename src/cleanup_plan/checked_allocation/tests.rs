use super::*;
const SOURCE: &str = r#"module checked.allocation;
@id("checked.text") fn text() -> string { string_concat("a", "b") }
@id("checked.main") fn main() -> i64 { let text = text(); string_len(text) }
"#;
fn source() -> Program {
    crate::parse(SOURCE, "checked-allocation.spx").unwrap()
}

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
    prepared
        .with_function(&execution, |current| {
            assert!(current.held_by(
                prepared.ordinary_program(),
                &prepared.ordinary_program().functions[0]
            ));
            assert!(!current.held_by(&foreign, &foreign.functions[0]));
            assert!(!current.held_by(prepared.ordinary_program(), &foreign.functions[0]));
            assert_eq!(current.function().id.as_str(), "checked.text");
            assert!(std::ptr::eq(current.program(), prepared.ordinary_program()));
            // Preparation has not reinterpreted any ordinary plan as v20.
            assert!(current
                .program()
                .functions
                .iter()
                .all(|f| f.cleanup_plan.schema != super::super::CLEANUP_PLAN_SCHEMA_V20));
        })
        .unwrap();
    assert!(prepared
        .with_function(
            &FunctionExecutionId::Monomorphic(DeclarationId::new("foreign")),
            |_| ()
        )
        .is_err());
}

#[test]
fn preparation_refuses_escaped_results_and_preserves_source_ownership_diagnostics() {
    let source = source();
    assert!(matches!(
        prepare(&source, &DeclarationId::new("checked.text")),
        Err(Refusal::Boundary(_))
    ));
    let bad = crate::parse("module checked.bad; @id(\"checked.bad.main\") fn main() -> i64 { let text=\"x\"; let first=string_take(text); string_len(text) }", "checked-bad.spx").unwrap();
    let old = hir::resolve(&bad).unwrap_err();
    let Err(Refusal::Source(actual)) = prepare(&bad, &DeclarationId::new("checked.bad.main"))
    else {
        panic!("must retain full original source refusal")
    };
    assert_eq!(actual, old);
}

#[test]
fn candidate_allocation_branches_precede_initialization_and_preserve_commit_ownership() {
    use crate::cleanup_plan::{
        AllocationCase, AllocationOperation, CleanupTerminator, CleanupTransition, EdgeCondition,
        ExitContinuation, StatusLane, StatusProducer,
    };
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
    prepared
        .with_function(
            &FunctionExecutionId::Monomorphic(DeclarationId::new("checked.text")),
            |current| {
                let candidate =
                    super::super::build::checked_allocation::candidate(current).unwrap();
                let validate = |plan: &crate::cleanup_plan::CleanupPlan| {
                    super::super::replay::ordinary_status::validate_allocation_projection(
                        current, plan,
                    )
                };
                assert!(
                    validate(&candidate).is_ok(),
                    "literal and concat producers independently discovered"
                );
                assert!(
                    validate(&current.function().cleanup_plan).is_err(),
                    "ordinary attachment is not v20 evidence"
                );
                let position = candidate
                    .status_sources
                    .iter()
                    .position(|row| row.id.lane == StatusLane::AllocationFailure)
                    .unwrap();
                let mut missing = candidate.clone();
                missing.status_sources.remove(position);
                assert!(validate(&missing).is_err());
                let mut duplicate = candidate.clone();
                duplicate
                    .status_sources
                    .insert(position, candidate.status_sources[position].clone());
                assert!(validate(&duplicate).is_err());
                let mut wrong = candidate.clone();
                wrong.status_sources[position].producer = StatusProducer::CheckedAllocation {
                    operation: AllocationOperation::BytesCopy,
                    normalized_cases: [
                        AllocationCase::PayloadRefused,
                        AllocationCase::SizeOverflow,
                    ],
                };
                assert!(validate(&wrong).is_err());
                let mut swapped = candidate.clone();
                let StatusProducer::CheckedAllocation {
                    normalized_cases, ..
                } = &mut swapped.status_sources[position].producer
                else {
                    panic!("allocation row")
                };
                normalized_cases.swap(0, 1);
                assert!(validate(&swapped).is_err());
                let mut wrong_lane = candidate.clone();
                wrong_lane.status_sources[position].id.lane = StatusLane::OperationFailure;
                assert!(validate(&wrong_lane).is_err());
                assert_eq!(
                    current.function().cleanup_plan,
                    super::super::build::build_plan(current.program(), current.function()).unwrap()
                );
            },
        )
        .unwrap();
}

#[test]
fn independent_allocation_status_replay_keeps_take_total_and_implicit_reads_cloning() {
    use crate::cleanup_plan::{AllocationOperation, StatusProducer};
    let source = crate::parse(
        r#"module checked.read;
@id("checked.read.main") fn main() -> i64 {
    let text = "abc";
    let copy = text;
    let moved = string_take(text);
    string_len(copy) + string_len(moved)
}
"#,
        "checked-read.spx",
    )
    .unwrap();
    let prepared = prepare(&source, &DeclarationId::new("checked.read.main")).unwrap();
    prepared
        .with_function(
            &FunctionExecutionId::Monomorphic(DeclarationId::new("checked.read.main")),
            |current| {
                let candidate =
                    super::super::build::checked_allocation::candidate(current).unwrap();
                assert!(
                    super::super::replay::ordinary_status::validate_allocation_projection(
                        current, &candidate
                    )
                    .is_ok()
                );
                let operations: Vec<_> = candidate
                    .status_sources
                    .iter()
                    .filter_map(|row| match row.producer {
                        StatusProducer::CheckedAllocation { operation, .. } => Some(operation),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    operations,
                    vec![
                        AllocationOperation::StringLiteral,
                        AllocationOperation::StringClone
                    ],
                    "take forwards the original carrier; an ordinary owning read still clones"
                );
            },
        )
        .unwrap();
}

fn allocation_refused() -> crate::conformance::NormalizedStatus {
    crate::conformance::NormalizedStatus::try_new(
        crate::cleanup_plan::ALLOCATION_DOMAIN_V1,
        crate::cleanup_plan::AllocationCase::PayloadRefused.code(),
        crate::conformance::StatusClass::Adapter,
        crate::conformance::Retryability::Known(false),
    )
    .unwrap()
}

#[test]
fn coverage_refuses_uncatalogued_materialization_before_any_candidate() {
    let source = crate::parse(
        r#"module checked.sort;
@id("checked.sort.main") fn main() -> i64 {
    let values = vec_of<i64>(3, 1);
    let sorted = vec_sort<i64>(values);
    if vec_len<i64>(sorted) == 2usize { 1 } else { 0 }
}
"#,
        "checked-sort.spx",
    )
    .unwrap();
    let prepared = prepare(&source, &DeclarationId::new("checked.sort.main"))
        .expect("ordinary source remains admitted");
    assert!(
        matches!(prepared.receive(), Err(Refusal::Coverage(_))),
        "sort scratch has no failure inventory; the whole closure refuses"
    );
}

#[test]
fn validated_oracle_injects_each_allocation_failure_with_sticky_canonical_cleanup() {
    use crate::cleanup_plan::execute::{CleanupExecutionError, CleanupScenario};
    use crate::cleanup_plan::StatusLane;
    use crate::conformance::{OperationOutcome, TraceEventKind, TraceOutcome, TraceResult};
    let source = source();
    let validated = prepare(&source, &DeclarationId::new("checked.main"))
        .unwrap()
        .receive()
        .expect("full projected replay");
    let text = validated
        .program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "checked.text")
        .unwrap();
    let rows: Vec<_> = text
        .cleanup_plan
        .status_sources
        .iter()
        .map(|row| row.id.clone())
        .collect();
    assert_eq!(
        rows.len(),
        4,
        "two literal headers, concat status, concat payload"
    );
    assert!(matches!(rows[0].lane, StatusLane::AllocationFailure));
    assert!(matches!(rows[1].lane, StatusLane::AllocationFailure));
    assert!(matches!(rows[2].lane, StatusLane::OperationFailure));
    assert!(matches!(rows[3].lane, StatusLane::AllocationFailure));
    let run = |scenario| {
        super::super::execute::checked_allocation::execute_for_conformance(
            &validated, &text.id, scenario,
        )
    };
    let refused = allocation_refused();
    // (failing row, finalized temporaries, committed group present)
    for (failing, finalized, committed) in [(0usize, 0usize, false), (1, 1, false), (3, 0, true)] {
        let mut scenario = CleanupScenario::new(format!("allocation-{failing}"), None);
        for (index, id) in rows.iter().enumerate().take(failing + 1) {
            scenario.operations.insert(
                id.clone(),
                if index == failing {
                    OperationOutcome::Failure(refused.clone())
                } else {
                    OperationOutcome::Success
                },
            );
        }
        let trace = run(scenario).expect("injected allocation failure is a language outcome");
        let TraceOutcome::Failure {
            selected_source,
            status,
        } = &trace.outcome
        else {
            panic!("allocation refusal never publishes a result")
        };
        assert_eq!(selected_source, &rows[failing]);
        assert_eq!(status, &refused);
        let count = |pick: fn(&TraceEventKind) -> bool| {
            trace
                .events
                .iter()
                .filter(|event| pick(&event.event))
                .count()
        };
        assert_eq!(
            count(|event| matches!(event, TraceEventKind::SelectFailure { .. })),
            1
        );
        assert_eq!(
            count(|event| matches!(event, TraceEventKind::ResultCommit { .. })),
            0
        );
        assert_eq!(
            count(|event| matches!(event, TraceEventKind::FinalizeEnd { .. })),
            finalized
        );
        // Concat's group commits before its payload allocation; the failure
        // exit cannot reacquire or finalize callee-owned arguments.
        assert_eq!(
            count(
                |event| matches!(event, TraceEventKind::CallCommit { arguments, .. } if arguments.len() == 2)
            ),
            usize::from(committed)
        );
    }
    let mut success = CleanupScenario::new("allocation-success", Some(TraceResult::String));
    for id in &rows {
        success
            .operations
            .insert(id.clone(), OperationOutcome::Success);
    }
    assert!(matches!(
        run(success).unwrap().outcome,
        TraceOutcome::Success {
            result: TraceResult::String
        }
    ));
    let mut foreign = CleanupScenario::new("allocation-wrong-domain", None);
    foreign.operations.insert(
        rows[0].clone(),
        OperationOutcome::Failure(crate::conformance::NormalizedStatus::arithmetic(
            crate::cleanup_plan::StatusCase::AddOverflow,
        )),
    );
    assert!(matches!(
        run(foreign),
        Err(CleanupExecutionError::HarnessInvariant(_))
    ));
    // The ordinary oracle never selects allocation replay for a v20 plan.
    let ordinary = crate::cleanup_plan::execute::execute_for_conformance(
        validated.program(),
        &text.id,
        CleanupScenario::new("ordinary", Some(TraceResult::String)),
    );
    assert!(matches!(
        ordinary,
        Err(CleanupExecutionError::InvalidProgram(_))
    ));
}

#[test]
fn release_lease_binds_allocator_families_and_every_engine_refuses() {
    use super::lease::{capture, Engine, Release};
    let source = source();
    let validated = prepare(&source, &DeclarationId::new("checked.main"))
        .unwrap()
        .receive()
        .unwrap();
    let lease = capture(&validated).expect("String owners use the allocator-owned family");
    assert!(std::ptr::eq(lease.validated(), &validated));
    assert_eq!(lease.releases().collect::<Vec<_>>(), vec![Release::String]);
    for engine in [Engine::Native, Engine::Wasm, Engine::Interpreter] {
        assert!(matches!(lease.receive(engine), Err(Refusal::Lease(_))));
    }
}

#[test]
fn aggregate_metadata_cannot_inherit_native_inline_nonallocation_authority() {
    let source = crate::parse(
        r#"module checked.aggregate;
@id("checked.aggregate.Pair") record Pair { left: i64, right: i64 }
@id("checked.aggregate.main") fn main() -> i64 {
    let pair = Pair { left: 1, right: 2 };
    pair.left + pair.right
}
"#,
        "checked-aggregate.spx",
    )
    .unwrap();
    let prepared = prepare(&source, &DeclarationId::new("checked.aggregate.main"))
        .expect("ordinary Copy-record source remains admitted");
    let Err(Refusal::Coverage(diagnostic)) = prepared.receive() else {
        panic!("interpreter map/Arc metadata requires its checked receiver")
    };
    assert!(diagnostic.message.contains("aggregate carrier metadata"));
}

#[test]
fn array_metadata_is_ordinary_valid_but_requires_a_checked_cross_engine_receiver() {
    for (length, value) in [(0, "[]"), (2, "[1, 2]"), (0, "[0; 0]"), (2, "[7; 2]")] {
        let input = format!(
            r#"module checked.array;
@id("checked.array.main") fn main() -> i64 {{
    let data: [u8; {length}] = {value};
    0
}}
"#
        );
        let source = crate::parse(&input, "checked-array.spx").unwrap();
        let prepared = prepare(&source, &DeclarationId::new("checked.array.main"))
            .expect("ordinary fixed arrays retain their existing admission");
        let Err(Refusal::Coverage(diagnostic)) = prepared.receive() else {
            panic!("empty and nonempty interpreter Arc carriers are physical allocations")
        };
        assert!(diagnostic.message.contains("array carrier metadata"));
    }
}
