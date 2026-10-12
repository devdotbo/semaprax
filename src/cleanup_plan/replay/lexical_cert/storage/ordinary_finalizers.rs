//! Predicate controls use the actual resolved owned String-variant finalizer facts.
//! Full application acceptance remains the unchanged ShiftSim owner.
use super::*;

fn fixture() -> (
    ResolvedFunction,
    Vec<crate::cleanup_plan::FinalizeAction>,
    BTreeSet<StorageId>,
    Leaves,
) {
    let parsed = crate::check(
        r#"module paid.finalizer;
@id("payload") variant Payload {
    @id("payload.empty") Empty,
    @id("payload.text") Text {
        @id("payload.text.value") value: string,
    },
}
@id("probe") fn probe(value:own Payload)->i64 {0}
@id("main") fn main()->i64 {0}"#,
        "paid-finalizer.spx",
    )
    .unwrap();
    let mut program = crate::hir::resolve(&parsed).unwrap();
    let index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "probe")
        .unwrap();
    let function = program.functions.remove(index);
    let actions = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            exit.finalize_in_order
                .iter()
                .any(|action| action.active_case.is_some())
        })
        .expect("actual conditional exit")
        .finalize_in_order
        .iter()
        .filter(|action| action.active_case.is_some())
        .cloned()
        .collect::<Vec<_>>();
    assert!(!actions.is_empty(), "actual conditional finalizer fixture");
    let known = function
        .cleanup_plan
        .slots
        .iter()
        .map(|slot| slot.storage.clone())
        .collect();
    let leaves = Leaves::new(
        actions
            .iter()
            .map(|action| {
                (
                    action.guard_flag,
                    Leaf {
                        place: action.source.clone(),
                        lifecycle: action.lifecycle_id.clone(),
                    },
                )
            })
            .collect(),
    );
    (function, actions, known, leaves)
}

#[test]
fn ordinary_paid_conditional_finalizers_keep_legacy_authority_and_selected_refusal() {
    let (function, actions, known, leaves) = fixture();
    validate_finalizer_actions(&function, &actions, &known, &leaves).unwrap();
    let ((result, observed), refused, used) =
        crate::bounded_output::with_limit_usage(1 << 20, || {
            crate::test_allocation_observer::measure(|| {
                let mut budget = ReplayBudget::new();
                assert!(!budget.selected() && budget.pays_allocations());
                finalizer_actions(&function, &actions, &known, &leaves, &mut budget)
            })
        });
    result.unwrap();
    assert!(!refused && !observed.invalid);
    assert_eq!(observed.live, 0);
    assert!(observed.allocated <= used && observed.highwater <= used);
    let (exact, refused, exact_used) = crate::bounded_output::with_limit_usage(used, || {
        finalizer_actions(
            &function,
            &actions,
            &known,
            &leaves,
            &mut ReplayBudget::new(),
        )
    });
    exact.unwrap();
    assert!(!refused);
    assert_eq!(exact_used, used);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(used - 1, || {
        finalizer_actions(
            &function,
            &actions,
            &known,
            &leaves,
            &mut ReplayBudget::new(),
        )
    });
    assert!(refused && short.is_err());

    let mut selected = ReplayBudget::new();
    selected.admission = AdmissionMode::SelectedRetained;
    selected.payer = AllocationPayer::SelectedReservation;
    selected.physical = Some(Box::new(physical_storage::PhysicalReservation::new(
        1 << 20,
    )));
    assert!(
        finalizer_actions(&function, &actions, &known, &leaves, &mut selected)
            .unwrap_err()
            .message
            .contains("outside the primitive profile")
    );

    let mut cases = vec![actions.clone()];
    let mut wrong = actions.clone();
    wrong[0].active_case.as_mut().unwrap().case = function.id.clone();
    cases.push(wrong);
    let mut wrong = actions.clone();
    wrong[0].active_case.as_mut().unwrap().variant = function.id.clone();
    cases.push(wrong);
    let mut wrong = actions.clone();
    wrong[0].active_case.as_mut().unwrap().storage = StorageId::ProvisionalResult;
    cases.push(wrong);
    let mut wrong = actions.clone();
    wrong[0].lifecycle_id = function.id.clone();
    cases.push(wrong);
    let mut duplicate = actions.clone();
    duplicate.push(actions[0].clone());
    cases.push(duplicate);
    for actions in cases {
        let expected = validate_finalizer_actions(&function, &actions, &known, &leaves);
        let (actual, refused, _) = crate::bounded_output::with_limit_usage(1 << 20, || {
            finalizer_actions(
                &function,
                &actions,
                &known,
                &leaves,
                &mut ReplayBudget::new(),
            )
        });
        assert!(!refused);
        assert_eq!(actual, expected);
    }
}
