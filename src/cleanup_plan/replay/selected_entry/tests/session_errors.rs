//! Selected-local error data only; no whole-engine funding or application claim.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
const FORM: ReportForm = ReportForm::new("SPX-T781", "selected session refusal");
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module selected.refusal; @id(\"main:é\") fn main()->i64 {0}",
            "session.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn acquire<'a>(
    program: &'a dyn ReplayContext,
    function: &'a ResolvedFunction,
    bytes: usize,
) -> Session<'a> {
    Acquired::try_acquire(PhysicalReservation::new(bytes), FORM)
        .ok()
        .unwrap()
        .prepare(function)
        .ok()
        .unwrap()
        .into_paid_session(program, function)
        .ok()
        .unwrap()
}
fn same(left: &Diagnostic, right: &Diagnostic) {
    assert_eq!(left.code, right.code);
    assert_eq!(left.severity, right.severity);
    assert_eq!(left.message, right.message);
    assert_eq!(left.path, right.path);
    assert_eq!(left.span, right.span);
    assert_eq!(left.help, right.help);
}
fn bytes() -> usize {
    // Independent full carrier census: no former-header credit or padding fit.
    std::mem::size_of::<(
        semantic_refusal::Receipt,
        expression_facts::errors::EngineError,
        Result<selected_context::Context<'static>, expression_facts::errors::EngineError>,
        Result<crate::hir::FunctionExecutionId, expression_facts::errors::EngineError>,
        Result<(), expression_facts::errors::EngineError>,
        Option<expression_facts::errors::EngineError>,
    )>()
}
#[test]
fn selected_session_local_refusals_preserve_complete_original_and_raw_owned_oracle() {
    let program = fixture();
    let context = primitive_fixture::Context::new(&program);
    let function = &program.functions[0];
    // Both forge identity-by-address; profile failure must still precede owner.
    for profile in [false, true] {
        let mut foreign = program.functions[0].clone();
        if profile {
            foreign.effects.push("foreign.effect".to_owned());
        }
        let mut original = acquire(&context, function, usize::MAX);
        let expression_facts::errors::EngineError::Owned(old) = original
            .validate_scalar_paid_original(&foreign)
            .unwrap_err()
        else {
            panic!("complete original owned refusal remains independently executable")
        };
        let mut current = acquire(&context, function, usize::MAX);
        let error = current.validate_scalar_paid(&foreign).unwrap_err();
        let expression_facts::errors::EngineError::Structural(receipt) = error else {
            panic!("local selected error must carry data without a String")
        };
        assert_eq!(receipt.stage(), "selected session");
        assert_eq!(
            receipt.source().as_str().as_ptr(),
            foreign.body.id.as_str().as_ptr()
        );
        same(&old, &receipt.legacy(&foreign));
        assert!(old.message.contains(if profile {
            "module-local primitive context"
        } else {
            "not held by its current context"
        }));
        assert!(matches!(
            current.validate_scalar_paid(&program.functions[0]),
            Err(expression_facts::errors::EngineError::Failed)
        ));
    }
}
#[test]
fn selected_session_receipt_full_carrier_exact_short_zero_sticky_report_and_drop() {
    let program = fixture();
    let context = primitive_fixture::Context::new(&program);
    let function = &program.functions[0];
    let (_, overflow, base) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let owner = acquire(&context, function, usize::MAX);
        owner.usage().1
    });
    assert!(!overflow);
    for (extra, semantic) in [(0, false), (bytes() - 1, false), (bytes(), true)] {
        let (_, overflow, ()) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let (_, observed) = measure(|| {
                let mut owner = acquire(&context, function, base + extra);
                let backing = owner.report.as_ref().unwrap().backing();
                let ledger = owner.budget.physical.as_deref().unwrap() as *const _;
                let before = current_metrics();
                let error = selected_context::paid::refusal(
                    function,
                    &mut owner.budget,
                    "selected lexical function is not held by its current context",
                );
                assert_eq!(
                    matches!(&error, expression_facts::errors::EngineError::Structural(_)),
                    semantic
                );
                assert_eq!(current_metrics().allocations, before.allocations);
                if semantic {
                    assert_eq!(owner.usage().1, base + bytes());
                }
                let mut refusal = owner.into_failed_preparation().refuse_engine(error);
                assert_eq!(refusal.report().unwrap().as_ptr(), backing.0);
                assert_eq!(refusal.report().unwrap()[0].message.as_ptr(), backing.1);
                assert_eq!(refusal.ledger_mut().unwrap() as *const _, ledger);
                if semantic {
                    assert_eq!(
                        refusal.structural_receipt().unwrap().stage(),
                        "selected session"
                    );
                }
                let first = refusal.ledger_mut().unwrap().reserve(0).unwrap_err();
                assert_eq!(refusal.ledger_mut().unwrap().reserve(1).unwrap_err(), first);
                assert_eq!(refusal.ledger_mut().unwrap().reserve(0).unwrap_err(), first);
                drop(refusal.into_report(FORM).ok().unwrap());
                assert_eq!(current_metrics().allocations, before.allocations);
            });
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        });
        assert!(!overflow);
    }
    for work in [0, 5, 6] {
        let (_, overflow, ()) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let mut owner = acquire(&context, function, usize::MAX);
            owner.budget.remaining = work;
            let error = selected_context::paid::refusal(
                function,
                &mut owner.budget,
                "selected profile cursor is empty",
            );
            assert_eq!(
                matches!(error, expression_facts::errors::EngineError::Structural(_)),
                work == 6
            );
            assert_eq!(owner.budget.remaining, 0);
        });
        assert!(!overflow);
    }
}

#[test]
fn selected_session_publication_keeps_first_named_boundary_across_later_atom_kinds() {
    let program = fixture();
    let context = primitive_fixture::Context::new(&program);
    let function = &program.functions[0];
    for selected_first in [false, true] {
        let (_, overflow, ()) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let mut owner = acquire(&context, function, usize::MAX);
            // Both complete receiving controls are prepaid before their atoms.
            let selected = selected_context::paid::refusal(
                function,
                &mut owner.budget,
                "selected lexical function is not held by its current context",
            );
            selected_context::paid::acquire::<selected_context::Context<'_>>(
                function,
                &mut owner.budget,
            )
            .unwrap();
            let structural =
                <expression_facts::errors::EngineError as semantic_refusal::Error>::structural(
                    function,
                    semantic_refusal::Atom::UnknownStatus(function.body.id.clone()),
                );
            let (first, later) = if selected_first {
                (selected, structural)
            } else {
                (structural, selected)
            };
            let mut preparation = owner.into_failed_preparation();
            preparation.receipt = Some(first);
            let refusal = preparation.refuse_engine(later);
            assert_eq!(
                refusal.refused().unwrap().stage(),
                Some(if selected_first {
                    "selected session"
                } else {
                    "structural replay"
                })
            );
            assert_eq!(
                refusal.structural_receipt().unwrap().stage(),
                if selected_first {
                    "selected session"
                } else {
                    "structural replay"
                }
            );
        });
        assert!(!overflow);
    }
}

#[allow(
    dead_code,
    reason = "complete independent historical enum layout, no constructors or runtime work"
)]
mod prior_layout {
    use super::*;
    pub(super) enum Atom {
        RepeatedStatus(ExpressionId),
        UnknownStatus(ExpressionId),
        NoncontiguousSlot(usize),
        RepeatedStorage(StorageId),
        DeadCallFlag(ExpressionId, LivenessFlagId),
        RepeatedCallFlag(ExpressionId, LivenessFlagId),
        RepeatedVariantEpoch(ExpressionId),
        MismatchedFailure(ExpressionId),
    }
    pub(super) struct Receipt {
        source: ExpressionId,
        atom: Atom,
    }
    pub(super) enum EngineError {
        Failed,
        Owned(Diagnostic),
        Fact(expression_facts::errors::FactReceipt),
        Structural(Receipt),
    }
}
#[test]
fn selected_session_atom_keeps_every_success_receiving_layout_equal_to_complete_old_sum() {
    fn equal<A, B>() {
        assert_eq!(std::mem::size_of::<A>(), std::mem::size_of::<B>());
        assert_eq!(std::mem::align_of::<A>(), std::mem::align_of::<B>());
    }
    use expression_facts::errors::EngineError as New;
    use prior_layout as old;
    equal::<old::Atom, semantic_refusal::Atom>();
    equal::<old::Receipt, semantic_refusal::Receipt>();
    equal::<old::EngineError, New>();
    equal::<Option<old::EngineError>, Option<New>>();
    equal::<Option<old::Receipt>, Option<semantic_refusal::Receipt>>();
    equal::<Result<(), old::EngineError>, Result<(), New>>();
    equal::<
        Result<crate::hir::FunctionExecutionId, old::EngineError>,
        Result<crate::hir::FunctionExecutionId, New>,
    >();
    equal::<
        Result<selected_context::Context<'static>, old::EngineError>,
        Result<selected_context::Context<'static>, New>,
    >();
}

mod source;
