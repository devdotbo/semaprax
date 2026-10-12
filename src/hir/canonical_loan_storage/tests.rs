//! Storage-only payment controls and actual current constructor parity.
//! Independently rebuilt plan scratch/diagnostic text are not covered by the
//! storage-only observer. Whole compiler gates retain their original inequality.
use super::*;
use crate::hir::validation::{constructor_seed::ConstructorSeed, owner_renewal};
use crate::hir::{hir_error, HirValidator};
use crate::loan_plan::{Loan, LoanEndpoint, LoanPlan, LoanProgramPoint};
use std::collections::BTreeMap;
mod old;

fn fixture() -> ResolvedProgram {
    let source = crate::check(
        r#"module loan.storage;
@id("packet") record Packet { @id("payload") payload:Bytes, @id("sibling") sibling:Bytes, }
@id("first") fn first(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.payload);
 byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)
}
@id("second") fn second(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.sibling); byte_len(view)+byte_len(view)
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(0)}
"#,
        "canonical-loan-storage.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn same(actual: &Storage<'_>, expected: &old::Inventory) {
    assert_eq!(actual.ids.len(), expected.ids.len());
    for (key, id) in &expected.ids {
        assert_eq!(actual.id(key), Some(id));
    }
    assert_eq!(actual.liveness.len(), expected.liveness.len());
    for (key, index) in &expected.liveness {
        assert_eq!(actual.live(key), Some(index));
        assert_eq!(actual.origin(*index), expected.origins.get(*index));
    }
    assert_eq!(actual.origins, expected.origins);
}
fn message(error: Diagnostic) -> (&'static str, String) {
    (error.code, error.message)
}

#[test]
fn funded_current_storage_matches_saved_complete_inventory_and_rejects_transplants() {
    let program = fixture();
    let directory = crate::hir::function_directory::caller::prepare(&program).unwrap();
    let seed = ConstructorSeed::authenticate(&program, Some(&directory)).unwrap();
    assert!(!program.function_instances.is_empty());
    let expected = old::Inventory::rebuild(&program).unwrap();
    let storage = caller::prepare(&seed).unwrap();
    same(&storage, &expected);
    assert_eq!(storage.origins.len(), 9);
    let mut offset = 0;
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        let plan = crate::loan_plan::build_plan(&program, function).unwrap();
        for (local, loan) in plan.loans.iter().enumerate() {
            assert_eq!(storage.origin(offset + local), Some(&loan.origin));
        }
        offset += plan.loans.len();
    }
    assert_eq!(
        caller::validate(&program, None, Some(&directory)),
        owner_renewal::validate_with_directory(&program, None, Some(&directory))
    );
    for reorder in [false, true] {
        let mut clone = program.clone();
        if reorder {
            clone.functions.reverse();
        }
        let foreign_directory = crate::hir::function_directory::caller::prepare(&clone).unwrap();
        let foreign_seed = ConstructorSeed::authenticate(&clone, Some(&foreign_directory)).unwrap();
        let error = HirValidator::with_storage(foreign_seed, &storage)
            .err()
            .unwrap();
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(
            error.message,
            "canonical shared-loan storage does not belong to the current program"
        );
    }
    let mut drift = program.clone();
    drift
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "first")
        .unwrap()
        .loan_plan
        .loans[0]
        .origin
        .projections[0] =
        crate::hir::PlaceProjection::Field(crate::hir::DeclarationId::new("sibling"));
    assert_eq!(crate::hir::validate(&drift).unwrap_err().code, "SPX-H006");
    // A later loan-builder error cannot replace the existing constructor NUL
    // error. Seed still performs the complete original preflight first.
    let mut hostile = program.clone();
    hostile.entrypoint = crate::hir::DeclarationId::new("bad\0entry");
    hostile.functions[0].body.id = ExpressionId::from_owned("bad\0body".into());
    assert_eq!(
        message(caller::validate(&hostile, None, None).unwrap_err()),
        message(owner_renewal::validate_with_directory(&hostile, None, None).unwrap_err())
    );
}

fn plan(prefix: &str) -> LoanPlan {
    let point = |name: &str, phase| LoanProgramPoint {
        expression: ExpressionId::from_owned(format!("{prefix}.{name}")),
        phase,
    };
    let loans = (0..2)
        .map(|i| Loan {
            id: LoanId(i),
            site: point(&format!("site{i}"), LoanPointPhase::Before).expression,
            origin: crate::hir::Place {
                root: crate::hir::ValueId::new(format!("{prefix}.owner{i}")),
                projections: vec![crate::hir::PlaceProjection::Field(
                    crate::hir::DeclarationId::new("field"),
                )],
            },
            parent: None,
            start: point(&format!("site{i}"), LoanPointPhase::Before),
            ends: Vec::new(),
            end_edges: Vec::new(),
            cause: LoanCause::SliceView,
        })
        .collect();
    LoanPlan {
        schema: "test-storage-only",
        loans,
        endpoints: vec![
            LoanEndpoint {
                point: point("before", LoanPointPhase::Before),
                live_before: vec![LoanId(0), LoanId(1)].into_boxed_slice(),
                starts: Box::new([]),
                kills: Box::new([]),
                live_after: vec![LoanId(99)].into_boxed_slice(),
            },
            LoanEndpoint {
                point: point("after", LoanPointPhase::After),
                live_before: vec![LoanId(99)].into_boxed_slice(),
                starts: Box::new([]),
                kills: Box::new([]),
                live_after: vec![LoanId(0)].into_boxed_slice(),
            },
        ],
        edges: Vec::new(),
    }
}

#[test]
fn plan_batch_priority_last_local_id_wins_and_moved_origins_match_saved_receiver() {
    let program = fixture();
    let plans = vec![plan("first"), plan("second")];
    let pointers = plans
        .iter()
        .flat_map(|plan| {
            plan.loans
                .iter()
                .map(|loan| loan.origin.projections.as_ptr())
        })
        .collect::<Vec<_>>();
    let expected = old::from_plans(plans.clone()).unwrap();
    let actual = caller::from_plans(&program, plans).unwrap();
    same(&actual, &expected);
    for (origin, pointer) in actual.origins.iter().zip(pointers) {
        assert_eq!(origin.projections.as_ptr(), pointer);
    }
    let empty = crate::loan_plan::LoanPlan {
        schema: "test-storage-only",
        loans: Vec::new(),
        endpoints: Vec::new(),
        edges: Vec::new(),
    };
    same(
        &caller::from_plans(&program, vec![empty.clone()]).unwrap(),
        &old::from_plans(vec![empty]).unwrap(),
    );
    let mut last = plan("last");
    last.loans[1].id = LoanId(0);
    last.endpoints[0].live_before = vec![LoanId(0)].into_boxed_slice();
    let old = old::from_plans(vec![last.clone()]).unwrap();
    let current = caller::from_plans(&program, vec![last]).unwrap();
    same(&current, &old);
    assert!(current.liveness.iter().all(|(_, ordinal)| *ordinal == 1));
    for attack in 0..4 {
        let mut first = plan("attack");
        let mut second = plan("later");
        match attack {
            0 => {
                first.endpoints[0].live_before = vec![LoanId(99)].into_boxed_slice();
                first.loans[0].start.phase = LoanPointPhase::After;
            }
            1 => {
                first.endpoints[0].live_before =
                    vec![LoanId(0), LoanId(0), LoanId(99)].into_boxed_slice();
            }
            2 => {
                first.loans[0].start.phase = LoanPointPhase::After;
                second.endpoints[0].live_before = vec![LoanId(99)].into_boxed_slice();
            }
            3 => {
                second.loans[0].site = first.loans[0].site.clone();
            }
            _ => unreachable!(),
        }
        let plans = vec![first, second];
        let old = old::from_plans(plans.clone()).err().unwrap();
        let current = caller::from_plans(&program, plans).err().unwrap();
        assert_eq!(message(current), message(old));
    }
}

#[test]
fn actual_storage_payer_literal_capacities_exact_short_and_drop_keep_first_receipt() {
    use crate::test_allocation_observer::measure;
    use std::mem::size_of;
    let program = fixture();
    // Two plans: local2/probeIDs2/probeLive3 each; global buffers grow
    // IDs2→4, endpoints3→6, origins2→4. These are literal requested layouts,
    // not observed calibration or attached-plan sizing authority.
    let bytes = 4 * size_of::<(LoanId, usize)>()
        + 4 * size_of::<(IdKey, LoanId, usize)>()
        + 6 * size_of::<(LiveKey, Option<usize>, usize)>()
        + 6 * size_of::<(IdKey, LoanId)>()
        + 9 * size_of::<(LiveKey, usize)>()
        + 6 * size_of::<Place>();
    for allowance in [bytes, bytes - 1, 0] {
        let plans = vec![plan("first"), plan("second")];
        // Explicitly excluded held subject, input plan/payload and fixed Rc
        // ledger setup. Observe EVERY new probe/global buffer through Drop.
        let (((), observed), overflowed, used, receipt) =
            crate::bounded_output::with_limit_usage_refusal(allowance, || {
                measure(|| {
                    let result = caller::from_plans(&program, plans);
                    assert_eq!(result.is_ok(), allowance == bytes);
                    if let Ok(storage) = result {
                        assert_eq!(storage.origins.len(), 4);
                        drop(storage);
                    }
                    if allowance < bytes {
                        assert!(!crate::bounded_output::reserve_active_required(0));
                    }
                })
            });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(observed.allocated <= used);
        assert_eq!(overflowed, allowance < bytes);
        if allowance == bytes {
            assert_eq!(used, bytes);
            assert_eq!(observed.allocated, bytes);
            assert!(receipt.is_none());
        } else {
            assert!(receipt.is_some());
        }
    }
    let (_, overflowed, used, receipt) = crate::bounded_output::with_limit_usage_refusal(7, || {
        assert!(crate::bounded_output::reserve_active_required(2));
        let error = caller::capacity_failure(5, 1);
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.is_empty());
        assert!(!crate::bounded_output::reserve_active_required(0));
    });
    assert!(overflowed);
    assert_eq!(used, 3);
    assert_eq!(receipt.unwrap().requested, 5);
}
