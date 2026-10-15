//! Transition-only proof. Earlier/later engine children remain owned Diagnostics.
use super::*;
use crate::cleanup_plan::CallArgumentTransfer;
use crate::test_allocation_observer::{current_metrics, measure};
mod old;
const FORM: ReportForm = ReportForm::new("SPX-T781", "typed transition refusal");
fn fixture() -> ResolvedProgram {
    let id = format!("owner:é.{}", "x".repeat(1024));
    let source = format!(
        "module transition.atoms; @id(\"{id}\") fn main()->i64 {{let a=bytes_zeroed(1usize);0}}"
    );
    crate::hir::resolve(&crate::check(&source, "atoms.spx").unwrap()).unwrap()
}
fn empty() -> PathState {
    PathState {
        renewals: BTreeMap::new(),
        live_order: Vec::new(),
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
        returned: None,
    }
}
fn render(error: expression_facts::errors::EngineError, function: &ResolvedFunction) -> Diagnostic {
    match error {
        expression_facts::errors::EngineError::Structural(receipt) => receipt.legacy(function),
        expression_facts::errors::EngineError::Owned(error) => error,
        other => panic!("unexpected {other:?}"),
    }
}
#[test]
fn transition_atoms_preserve_saved_error_order_text_and_mutated_state() {
    let program = fixture();
    let function = &program.functions[0];
    let leaves = collect_leaves(function).unwrap();
    let place = leaves.values().next().unwrap().place.clone();
    let storage = BTreeSet::from([place.storage.clone()]);
    let flags = validate_place(function, &place, &storage, &leaves).unwrap();
    let call = function.body.id.clone();
    let one = CallArgumentTransfer {
        parameter_index: 0,
        source: place.clone(),
    };
    let commit = CleanupTransition::CallCommit {
        call: call.clone(),
        arguments: vec![one.clone(), one],
    };
    let failure = StatusSourceId {
        expression: call.clone(),
        lane: StatusLane::OperationFailure,
    };
    let select = CleanupTransition::SelectFailure {
        source: failure.clone(),
    };
    for selected in [false, true] {
        for mode in 0..5 {
            let mut state = empty();
            let transition = if mode < 2 { &commit } else { &select };
            if mode == 1 {
                state.live_order = flags.clone();
            }
            if mode == 3 {
                state.pending_failure = Some(failure.clone());
            }
            if mode == 4 {
                state.published = true;
                state.selected_failure = Some(failure.clone());
            }
            let mut expected = state.clone();
            let mut old_budget = if selected {
                ReplayBudget::with_physical(function, usize::MAX).unwrap()
            } else {
                ReplayBudget::new()
            };
            let mut budget = if selected {
                ReplayBudget::with_physical(function, usize::MAX).unwrap()
            } else {
                ReplayBudget::new()
            };
            let old = old::ordinary(
                &program,
                function,
                transition,
                &mut expected,
                &storage,
                &leaves,
                None,
                &mut old_budget,
            );
            let actual = execute_replay_transition::<expression_facts::errors::EngineError>(
                &program,
                function,
                transition,
                &mut state,
                &storage,
                &leaves,
                None,
                &mut budget,
            );
            assert_eq!(state, expected);
            match (old, actual) {
                (Ok(()), Ok(())) => assert_eq!(mode, 3),
                (Err(old), Err(actual)) => {
                    let actual = render(actual, function);
                    assert_eq!(actual.code, old.code);
                    assert_eq!(actual.message, old.message);
                }
                other => panic!("mode {mode} selected {selected}: {other:?}"),
            }
        }
    }
    // An ordinary conditional inventory with overlapping flags must retain the
    // original mutation-before-refusal and exact repeated-flag diagnostic.
    let mut state = empty();
    state.conditional_variants.push(ReplayConditionalVariant {
        root: place.clone(),
        variant: DeclarationId::new("variant"),
        cases: vec![
            (DeclarationId::new("a"), flags.clone()),
            (DeclarationId::new("b"), flags),
        ],
    });
    let mut expected = state.clone();
    let old = old::ordinary(
        &program,
        function,
        &commit,
        &mut expected,
        &storage,
        &leaves,
        None,
        &mut ReplayBudget::new(),
    )
    .unwrap_err();
    let actual = execute_replay_transition::<expression_facts::errors::EngineError>(
        &program,
        function,
        &commit,
        &mut state,
        &storage,
        &leaves,
        None,
        &mut ReplayBudget::new(),
    )
    .unwrap_err();
    assert_eq!(state, expected);
    assert_eq!(render(actual, function).message, old.message);
}
#[test]
fn held_paid_transition_mismatch_keeps_source_operands_first_report_and_drop_without_rendering() {
    let program = fixture();
    let function = &program.functions[0];
    let transition = CleanupTransition::SelectFailure {
        source: StatusSourceId {
            expression: function.body.id.clone(),
            lane: StatusLane::OperationFailure,
        },
    };
    let storage = BTreeSet::new();
    let leaves = collect_leaves(function).unwrap();
    let (_, overflowed, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let (_, observed) = measure(|| {
            let mut owner = Acquired::try_acquire(PhysicalReservation::new(usize::MAX), FORM)
                .ok()
                .unwrap()
                .prepare(function)
                .ok()
                .unwrap()
                .into_paid_session(&program, function)
                .ok()
                .unwrap();
            let report = owner.report.as_ref().unwrap().backing();
            let ledger = owner.budget.physical.as_deref().unwrap() as *const _;
            let before = current_metrics();
            let usage = owner.usage();
            let error = execute_replay_transition::<expression_facts::errors::EngineError>(
                &program,
                function,
                &transition,
                &mut empty(),
                &storage,
                &leaves,
                None,
                &mut owner.budget,
            )
            .unwrap_err();
            assert_eq!(current_metrics().allocations, before.allocations);
            assert_eq!(owner.usage(), usage);
            let expression_facts::errors::EngineError::Structural(receipt) = &error else {
                panic!("typed mismatch")
            };
            assert_eq!(
                receipt.source().as_str().as_ptr(),
                function.body.id.as_str().as_ptr()
            );
            owner.failed = true;
            assert!(matches!(
                owner.validate_scalar_paid(function),
                Err(expression_facts::errors::EngineError::Failed)
            ));
            let mut refused = owner.into_failed_preparation().refuse_engine(error);
            assert_eq!(refused.report().unwrap().as_ptr(), report.0);
            assert_eq!(refused.report().unwrap()[0].message.as_ptr(), report.1);
            assert_eq!(refused.ledger_mut().unwrap() as *const _, ledger);
            assert!(refused.structural_receipt().is_some());
            drop(refused.into_report(FORM).ok().unwrap());
        });
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.live, 0);
    });
    assert!(!overflowed);
}
