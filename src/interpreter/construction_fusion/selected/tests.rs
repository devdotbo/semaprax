//! Same source, current sealed owner, actual execution and receiving controls.
use super::{execute, refuse, reserve, Engine, Failure, Flat, Flow, Outcome, Row, Span, TextView};
use crate::cleanup_plan::{ExitContinuation, StatusLane};
use crate::hir::selected_fused_candidate::{validate_selected, ValidatedFusedSelection};
use crate::interpreter::{Utf8MaterializationBudget, Value};
use std::alloc::Layout;

const SOURCE: &str = include_str!("../../../../tests/fixtures/record-traversal-maximum.spx");
const PHYSICAL: usize = 64 * 1024 * 1024;
const FUEL: usize = 1_000_000;
fn parsed(source: &str) -> crate::ast::Program {
    crate::parse(source, "selected-flat-interpreter.spx").unwrap()
}
fn run(owner: &ValidatedFusedSelection<'_>) -> (Outcome, usize) {
    let (outcome, refused, used) =
        crate::bounded_output::with_limit_usage(PHYSICAL, || execute(owner, FUEL));
    assert!(!refused);
    (outcome.unwrap(), used)
}
#[test]
fn original_4096_executes_current_hir_with_one_backing_and_ordinary_string_charges() {
    let source = parsed(SOURCE);
    assert!(crate::check(SOURCE, "selected-flat-interpreter.spx").is_err());
    assert!(crate::hir::resolve(&source).is_err());
    let owner = validate_selected(&source).unwrap();
    let (outcome, used) = run(&owner);
    assert_eq!(outcome.result, 4096 * 4095 / 2);
    assert_eq!((outcome.byte_events, outcome.byte_payload), (1, 12_288));
    assert_eq!(
        (outcome.string_materializations, outcome.string_bytes),
        (4096, 12_288)
    );
    assert!(outcome.steps > 4096 && outcome.steps < FUEL);
    assert!(used >= Layout::array::<Row>(4096).unwrap().size());
    // Reentry has a fresh context and the same source/counters, not reused children.
    let (again, again_used) = run(&owner);
    assert_eq!(
        (again.result, again.byte_events, again.string_bytes),
        (outcome.result, outcome.byte_events, outcome.string_bytes)
    );
    assert_eq!(again_used, used);

    // Private conformance mirrors ordinary UnlimitedLegacy, not the retained
    // Project-only fixed UTF8 envelope. Keep the same count and first three bytes.
    let longer = SOURCE
        .replace("é\\u{0}", "é\\u{0}abcdefghijklmn")
        .replace("str_len_bytes(text) == 3", "str_len_bytes(text) == 17");
    assert_ne!(longer, SOURCE);
    let longer_source = parsed(&longer);
    let longer_owner = validate_selected(&longer_source).unwrap();
    let (longer_outcome, longer_used) = run(&longer_owner);
    assert_eq!(longer_outcome.result, outcome.result);
    assert_eq!(
        (
            longer_outcome.string_materializations,
            longer_outcome.string_bytes
        ),
        (4096, 4096 * 17)
    );
    assert_eq!(longer_used - used, 4096 * 14);

    // The unchanged <=32 ordinary control uses the existing evaluator and budget.
    let control_text = SOURCE.replace("4096", "32");
    let control =
        crate::hir::resolve(&crate::check(&control_text, "ordinary32.spx").unwrap()).unwrap();
    let entry = &control.functions[0];
    let admitted = std::collections::BTreeMap::from([(entry.id.as_str(), entry)]);
    let (result, _, _, utf8) = crate::interpreter::evaluate_resolved_entry_with_utf8_budget(
        entry,
        &[],
        &admitted,
        &control,
        FUEL,
        false,
        Utf8MaterializationBudget::fixed(),
    );
    assert!(matches!(result, Ok(Value::Int(496))), "{result:?}");
    assert_eq!(utf8, (32, 96));
    assert!(validate_selected(&parsed(&control_text)).is_err());
}
#[test]
fn actual_flat_allocations_match_target_layout_without_per_child_payload_or_map_nodes() {
    // 33 keeps the observer's fixed4096-entry inventory independent of original4096.
    let source = parsed(&SOURCE.replace("4096", "33"));
    let owner = validate_selected(&source).unwrap();
    let ((result, metrics), refused, _) = crate::bounded_output::with_limit_usage(PHYSICAL, || {
        owner
            .with_joined(|joined| {
                crate::test_allocation_observer::measure(|| {
                    reserve(Layout::new::<Engine<'_, '_>>()).unwrap();
                    let mut engine = Box::new(Engine::new(joined.selection(), FUEL).unwrap());
                    let result = engine
                        .expression(&joined.selection().function().body, 0)
                        .unwrap();
                    assert!(matches!(result, Flat::Scalar(Value::Int(528))));
                    engine.settle_exit(true).unwrap();
                    assert!(engine.rows.is_empty() && engine.pending.is_none());
                    assert_eq!((engine.events, engine.payload), (1, 99));
                    // All funded allocations are still invocation-local, then physically
                    // freed here. The observer sees real System allocation/deallocation.
                    drop(engine);
                    528
                })
            })
            .unwrap()
    });
    assert!(!refused);
    assert_eq!(result, 528);
    assert!(!metrics.invalid);
    assert_eq!(metrics.reallocations, 0);
    // 33 String backings + Engine + flat slots + Row backing + State + payload + three
    // simultaneous core tables + two address-stable identity boxes.
    assert_eq!(metrics.allocations, 33 + 10);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.freed, metrics.allocated);
    assert_eq!(metrics.highwater, metrics.allocated);
}
/// Successful checked arithmetic must not build (allocate and free) the status
/// it would select on overflow; four such operations per row surfaced as 132
/// extra allocations at 33 rows. Overflow still selects the exact status.
#[test]
fn successful_checked_arithmetic_allocates_nothing_and_overflow_keeps_exact_status() {
    use crate::ast::BinaryOp;
    use crate::cleanup_plan::StatusCase;
    let combine = crate::interpreter::combine;
    let (sums, metrics) = crate::test_allocation_observer::measure(|| {
        [
            combine(BinaryOp::Add, Value::Int(1), Value::Int(2)),
            combine(BinaryOp::Sub, Value::Int(1), Value::Int(2)),
            combine(BinaryOp::Add, Value::Usize(1), Value::Usize(2)),
        ]
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 0);
    assert!(matches!(sums[0], Some(Ok(Value::Int(3)))));
    assert!(matches!(sums[1], Some(Ok(Value::Int(-1)))));
    assert!(matches!(sums[2], Some(Ok(Value::Usize(3)))));
    for (op, case) in [
        (BinaryOp::Add, StatusCase::AddOverflow),
        (BinaryOp::Sub, StatusCase::SubOverflow),
    ] {
        let lhs = if op == BinaryOp::Add {
            i64::MAX
        } else {
            i64::MIN
        };
        let Some(Err(status)) = combine(op, Value::Int(lhs), Value::Int(1)) else {
            panic!("{op:?} overflow lost its status")
        };
        assert_eq!(status, crate::runtime_status::normalize_arithmetic(case));
    }
}
/// Fuel exhaustion is out of band: no plan exit, status or result. Only a
/// certified empty ownership state (before the Vec allocation) settles. Once an
/// owner exists, no finalizer is invented and cleanup stays incomplete; the
/// frame's physical Drop still returns every allocation.
#[test]
fn fuel_exhaustion_settles_only_certified_empty_ownership_without_status_or_result() {
    let source = parsed(&SOURCE.replace("4096", "33"));
    let owner = validate_selected(&source).unwrap();
    let (baseline, _) = run(&owner);
    for (fuel, settled) in [
        (1, true),
        (baseline.steps / 4, false),
        (baseline.steps / 2, false),
        (baseline.steps * 3 / 4, false),
        (baseline.steps - 1, false),
    ] {
        let ((outcome, metrics), refused, _) =
            crate::bounded_output::with_limit_usage(PHYSICAL, || {
                crate::test_allocation_observer::measure(|| execute(&owner, fuel))
            });
        assert!(!refused);
        let Err(Failure::FuelExhausted {
            steps,
            cleanup_complete,
        }) = outcome
        else {
            panic!("fuel {fuel}: {outcome:?}")
        };
        assert_eq!((steps, cleanup_complete), (fuel, settled), "fuel {fuel}");
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
    // The same owner still executes completely afterwards.
    assert_eq!(run(&owner).0.result, baseline.result);
    let selected = include_str!("../selected.rs");
    for guard in [
        "let settled = engine.settle(&result);",
        "Err(Flow::Failure(_)) => self.settle_exit(false),",
        "Err(_) => self.settle_empty(),",
        "&& source.lane == StatusLane::OperationFailure",
        "|| self.registry.state.is_some()",
    ] {
        assert!(selected.contains(guard), "{guard}");
    }
    assert!(!selected.contains("engine.settle_exit(result.is_ok())"));
}
/// A guard refusal stopping at an expression that owns a language failure exit
/// selects no status, so it must not borrow that exit's finalizers: with owners
/// live it stays unsettled. The actual language failure at the same site still
/// settles through its exact exit, and an empty engine settles any flow.
#[test]
fn out_of_band_flows_never_reuse_the_failure_exit_of_their_stopping_site() {
    let changed = SOURCE.replace("4096", "33").replace(
        "{ total } else { 0 }",
        "{ total + 9223372036854775807 } else { 0 }",
    );
    let source = parsed(&changed);
    let owner = validate_selected(&source).unwrap();
    let ((), refused, _) = crate::bounded_output::with_limit_usage(PHYSICAL, || {
        owner
            .with_joined(|joined| {
                let selection = joined.selection();
                let mut empty = Engine::new(selection, FUEL).unwrap();
                empty.failed_site = Some(selection.tail());
                assert!(empty.settle(&Err(refuse())).is_ok());
                let mut engine = Engine::new(selection, FUEL).unwrap();
                let result = engine.expression(&selection.function().body, 0);
                assert!(matches!(result, Err(Flow::Failure(_))));
                let site = engine.failed_site.expect("overflowing tail addition");
                assert!(selection.function().cleanup_plan.exits.iter().any(
                    |exit| matches!(&exit.continuation,
                        ExitContinuation::ReturnFailure { source }
                            if source.expression == site.id
                                && source.lane == StatusLane::OperationFailure)
                ));
                assert!(!engine.rows.is_empty());
                assert!(engine.settle(&Err(refuse())).is_err());
                assert!(!engine.rows.is_empty(), "no finalizer ran for the refusal");
                engine.settle(&result).unwrap();
                assert!(engine.rows.is_empty() && engine.pending.is_none());
            })
            .unwrap()
    });
    assert!(!refused);
}
#[test]
fn full_execution_exact_one_short_refuses_without_publication_or_drop_refund() {
    let source = parsed(&SOURCE.replace("4096", "33"));
    let owner = validate_selected(&source).unwrap();
    let (baseline, exact) = run(&owner);
    let (outcome, refused, used) =
        crate::bounded_output::with_limit_usage(exact, || execute(&owner, FUEL));
    assert!(!refused);
    assert_eq!(used, exact);
    assert_eq!(outcome.unwrap().result, baseline.result);
    let ((first, retry), refused, used) =
        crate::bounded_output::with_limit_usage(exact - 1, || {
            let first = execute(&owner, FUEL);
            // Failure poisons this actual physical invocation; cleanup never refunds.
            let retry = execute(&owner, FUEL);
            (first, retry)
        });
    assert!(refused);
    assert!(first.is_err() && retry.is_err());
    assert!(used < exact);
}
#[test]
fn current_context_scope_generation_and_live_payload_loans_guard_receiving_views() {
    let source = parsed(&SOURCE.replace("4096", "33"));
    let owner = validate_selected(&source).unwrap();
    let ((), refused, _) = crate::bounded_output::with_limit_usage(PHYSICAL, || {
        owner
            .with_joined(|joined| {
                let mut engine = Engine::new(joined.selection(), FUEL).unwrap();
                engine
                    .expression(&joined.selection().function().body, 0)
                    .unwrap();
                let scope = engine.registry.enter().unwrap();
                let state = engine.registry.state.as_deref().unwrap();
                let text = TextView {
                    row: 0,
                    context: std::ptr::from_ref(state) as usize,
                    scope: scope.unwrap(),
                    generation: engine.generation,
                };
                assert_eq!(
                    engine
                        .bytes(Span::Text(text), |bytes| <[u8; 3]>::try_from(bytes)
                            .unwrap())
                        .unwrap(),
                    [195, 169, 0]
                );
                for mutant in [
                    TextView {
                        context: text.context ^ 1,
                        ..text
                    },
                    TextView { row: 33, ..text },
                    TextView {
                        generation: text.generation - 1,
                        ..text
                    },
                    TextView {
                        scope: u64::MAX,
                        ..text
                    },
                ] {
                    assert!(engine.bytes(Span::Text(mutant), <[u8]>::len).is_err());
                }
                let super::super::Backing::Leased(lease) =
                    &engine.rows[0].payload.as_ref().unwrap().bytes
                else {
                    panic!()
                };
                let view = engine.registry.borrow(lease).unwrap();
                assert!(engine.sort().is_err());
                engine.registry.leave(scope).unwrap();
                assert!(engine.bytes(Span::Text(text), <[u8]>::len).is_err());
                assert!(engine.bytes(Span::Payload(view, 3), <[u8]>::len).is_err());
                engine.sort().unwrap();
                assert!(engine.bytes(Span::Text(text), <[u8]>::len).is_err());
                engine.settle_exit(true).unwrap();
            })
            .unwrap()
    });
    assert!(!refused);
}
/// Required right operands yield their exact Boolean; skipped right operands
/// never execute, so their failure is not selected.
#[test]
fn lazy_boolean_truth_table_and_skipped_right_operand_failure() {
    const TAIL: &str =
        "if good && count == 33 && vec_len<Row>(renewed) == 33usize { total } else { 0 }";
    const FAILING: &str = "i64_from_usize(18446744073709551615usize) == 0";
    let rows = SOURCE.replace("4096", "33");
    assert!(rows.contains(TAIL));
    let outcome = |condition: &str| {
        let changed = rows.replace(TAIL, &format!("if {condition} {{ total }} else {{ 0 }}"));
        assert_ne!(changed, rows);
        let source = parsed(&changed);
        let owner = validate_selected(&source).unwrap();
        let (outcome, refused, _) =
            crate::bounded_output::with_limit_usage(PHYSICAL, || execute(&owner, FUEL));
        assert!(!refused);
        outcome
    };
    let mut cases: Vec<(String, i64)> = [
        ("count == 33 && count == 33", 528),
        ("count == 33 && count == 32", 0),
        ("count == 32 && count == 33", 0),
        ("count == 32 && count == 32", 0),
        ("count == 33 || count == 33", 528),
        ("count == 33 || count == 32", 528),
        ("count == 32 || count == 33", 528),
        ("count == 32 || count == 32", 0),
    ]
    .into_iter()
    .map(|(condition, expected)| (condition.to_owned(), expected))
    .collect();
    cases.push((format!("count == 32 && {FAILING}"), 0));
    cases.push((format!("count == 33 || {FAILING}"), 528));
    for (condition, expected) in &cases {
        assert_eq!(outcome(condition).unwrap().result, *expected, "{condition}");
    }
    for condition in [
        format!("count == 33 && {FAILING}"),
        format!("count == 32 || {FAILING}"),
    ] {
        let Err(Failure::Execution {
            status: Some(status),
            cleanup_complete: true,
            ..
        }) = outcome(&condition)
        else {
            panic!("required right operand failure was lost: {condition}")
        };
        assert_eq!(
            status,
            crate::conformance::NormalizedStatus::try_new(
                crate::string_ops::CONVERT_STATUS_DOMAIN,
                crate::string_ops::CONVERT_OUT_OF_RANGE_CODE,
                crate::conformance::StatusClass::Adapter,
                crate::conformance::Retryability::Known(false),
            )
            .unwrap()
        );
    }
}

#[test]
fn checked_suffix_failure_keeps_exact_status_and_settles_before_reentry() {
    let changed = SOURCE.replace(
        "{ total } else { 0 }",
        "{ total + 9223372036854775807 } else { 0 }",
    );
    assert_ne!(changed, SOURCE);
    let source = parsed(&changed);
    let owner = validate_selected(&source).unwrap();
    for _ in 0..3 {
        let (outcome, refused, _) =
            crate::bounded_output::with_limit_usage(PHYSICAL, || execute(&owner, FUEL));
        assert!(!refused);
        let Failure::Execution {
            status: Some(status),
            steps,
            cleanup_complete,
        } = outcome.unwrap_err()
        else {
            panic!("wrong failure")
        };
        let expected = crate::runtime_status::normalize_arithmetic(
            crate::cleanup_plan::StatusCase::AddOverflow,
        );
        assert_eq!(status, expected);
        assert!(cleanup_complete);
        assert!(steps > 4096 && steps < FUEL);
    }
    let converted = SOURCE.replace(
        "{ total } else { 0 }",
        "{ i64_from_usize(18446744073709551615usize) } else { 0 }",
    );
    assert_ne!(converted, SOURCE);
    let converted_source = parsed(&converted);
    let converted_owner = validate_selected(&converted_source).unwrap();
    let (outcome, refused, _) =
        crate::bounded_output::with_limit_usage(PHYSICAL, || execute(&converted_owner, FUEL));
    assert!(!refused);
    let Failure::Execution {
        status: Some(status),
        cleanup_complete: true,
        ..
    } = outcome.unwrap_err()
    else {
        panic!("conversion status or settlement was lost")
    };
    assert_eq!(
        status,
        crate::conformance::NormalizedStatus::try_new(
            crate::string_ops::CONVERT_STATUS_DOMAIN,
            crate::string_ops::CONVERT_OUT_OF_RANGE_CODE,
            crate::conformance::StatusClass::Adapter,
            crate::conformance::Retryability::Known(false),
        )
        .unwrap()
    );
    let normal_source = parsed(SOURCE);
    assert_eq!(
        run(&validate_selected(&normal_source).unwrap()).0.result,
        4096 * 4095 / 2
    );
}
