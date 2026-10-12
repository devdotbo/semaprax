//! The actual primitive lexical caller owns its paid outward report.
use super::*;
use crate::cleanup_plan::{
    ReplayPhysicalRefusal, SelectedReplayCause, SelectedReplayRefusal, SelectedReplayUnpaidReason,
};
use crate::test_allocation_observer::{measure, Metrics};

fn replay_under(
    build: &WorkspaceGraphBuild,
    limit: usize,
) -> (Result<bool, SelectedReplayRefusal>, Metrics, bool, usize) {
    let ((answer, metrics), overflowed, used) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                replay_lexical_paid(
                    writer(build),
                    &build.hir.modules,
                    &build.hir.declarations,
                    REPORT,
                )
            })
        });
    (answer, metrics, overflowed, used)
}
fn first_receipt(refusal: &mut SelectedReplayRefusal) -> ReplayPhysicalRefusal {
    let ledger = refusal.ledger_mut().expect("retained ledger");
    let first = ledger.reserve(0).unwrap_err();
    assert_eq!(ledger.reserve(1).unwrap_err(), first);
    first
}
fn assert_public(report: &[Diagnostic]) {
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].code, "SPX-G410");
    assert_eq!(
        report[0].message,
        "retained static UTF-8 source-site authority disagrees"
    );
    assert_eq!(report[0].message.capacity(), report[0].message.len());
    assert!(report[0].path.is_none() && report[0].span.is_none() && report[0].help.is_none());
}
fn lexical_anchor(build: &WorkspaceGraphBuild) -> &hir::ResolvedFunction {
    &writer(build).functions[0]
}

#[test]
fn actual_caller_zero_and_report_one_short_return_the_unpaid_ledger_without_allocating() {
    let build = build(LEXICAL_SOURCE);
    let slot = std::mem::size_of::<Diagnostic>();
    for (limit, charged, reason) in [
        (0, 0, "cleanup replay report carrier reservation refused"),
        (
            slot - 1,
            0,
            "cleanup replay report carrier reservation refused",
        ),
        (
            REPORT.paid_bytes() - 1,
            slot,
            "cleanup replay report text reservation refused",
        ),
    ] {
        let (answer, metrics, overflowed, used) = replay_under(&build, limit);
        let Err(mut refusal) = answer else {
            panic!("a one-short enclosing budget cannot fund the report carrier")
        };
        assert!(!metrics.invalid);
        assert_eq!(
            (metrics.allocations, metrics.allocated, metrics.live),
            (0, 0, 0),
            "no Diagnostic, Vec, String or preparation backing before refusal"
        );
        assert!(overflowed);
        assert_eq!(used, charged);
        assert!(!refusal.is_paid());
        assert_eq!(refusal.setup_reason().map(|r| r.message()), Some(reason));
        assert_eq!(
            first_receipt(&mut refusal),
            ReplayPhysicalRefusal::EnclosingBudget
        );
        assert_eq!(refusal.ledger_mut().unwrap().charged(), charged);
        // The unpaid refusal stays typed; it never claims funding.
        let (unpaid, typed) = measure(|| refusal.into_published());
        assert_eq!(typed.allocations, 0);
        let unpaid = unpaid.expect_err("unpaid carrier");
        assert!(matches!(
            unpaid.reason(),
            SelectedReplayUnpaidReason::Carrier(_)
        ));
        // Only the named unfunded boundary allocates the same literal.
        assert_public(&unpaid.into_unfunded());
    }
}

#[test]
fn actual_caller_paid_setup_refusal_keeps_original_carrier_ledger_and_drop() {
    let build = build(LEXICAL_SOURCE);
    let paid = REPORT.paid_bytes();
    let (answer, metrics, overflowed, used) = replay_under(&build, paid);
    let Err(mut refusal) = answer else {
        panic!("an exact report payment leaves no preparation backing")
    };
    assert!(overflowed);
    assert_eq!(used, paid);
    assert!(refusal.is_paid());
    assert_public(refusal.report().unwrap());
    assert_eq!(
        refusal.setup_reason().map(|r| r.message()),
        Some("cleanup replay physical reservation exceeds the bound")
    );
    assert_eq!(
        refusal.refused().map(|r| r.cause()),
        Some(SelectedReplayCause::Setup)
    );
    assert_eq!(
        first_receipt(&mut refusal),
        ReplayPhysicalRefusal::EnclosingBudget
    );
    assert_eq!(refusal.ledger_mut().unwrap().charged(), paid);
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 2, "exactly the paid slot and text");
    assert_eq!(
        metrics.reallocations, 0,
        "publication kept the original backing"
    );
    assert_eq!(metrics.allocated, paid);
    assert_eq!(
        metrics.live, paid,
        "the report is still owned by the caller"
    );
    let (report, moved) = measure(|| refusal.into_published());
    assert_eq!(moved.allocations, 0, "paid report moves out unchanged");
    let report = report.expect("paid G410 carrier");
    assert_public(&report);
    let (_, dropped) = measure(|| drop(report));
    assert_eq!((dropped.freed, dropped.allocations), (paid, 0));
}

#[test]
fn actual_caller_authentication_refusal_allocates_and_discards_nothing() {
    for attack in [0, 1] {
        let mut build = build(LEXICAL_SOURCE);
        let function = writer_mut(&mut build)
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        match attack {
            0 => function.name.push('x'),
            _ => function.return_type = hir::ResolvedType::Bool,
        }
        let (answer, metrics, _, _) = replay_under(&build, MAX_CHANGE_BUILDER_BYTES);
        let Err(refusal) = answer else {
            panic!("attack {attack} must refuse")
        };
        // Carrier slot and text plus the neutral preparation (ledger box and
        // prepaid refusal text). A discarded legacy Vec<Diagnostic> or String
        // before the transfer would appear here as a free or a reallocation.
        assert!(!metrics.invalid);
        assert_eq!(
            (metrics.freed, metrics.reallocations),
            (0, 0),
            "attack {attack}: nothing allocated and discarded before return"
        );
        assert_eq!(metrics.live, metrics.allocated, "attack {attack}");
        assert!(refusal.is_paid(), "attack {attack}");
        assert_public(refusal.report().unwrap());
        let refused = refusal.refused().unwrap();
        assert_eq!(
            refused.cause(),
            SelectedReplayCause::Mismatch,
            "attack {attack}"
        );
        assert!(refusal.receipt().is_none(), "no budget or Session receipt");
    }
}

#[test]
fn actual_caller_authentication_and_session_refusals_transfer_one_paid_report() {
    for attack in [0, 1, 3] {
        let mut build = build(LEXICAL_SOURCE);
        let function = writer_mut(&mut build)
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        match attack {
            0 => function.name.push('x'),
            1 => function.return_type = hir::ResolvedType::Bool,
            _ => {
                function
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .exits[0]
                    .site = function.body.id.clone()
            }
        }
        let (answer, _, _, _) = replay_under(&build, MAX_CHANGE_BUILDER_BYTES);
        let Err(mut refusal) = answer else {
            panic!("attack {attack} must refuse")
        };
        assert!(refusal.is_paid(), "attack {attack}");
        assert_public(refusal.report().unwrap());
        assert_eq!(refusal.setup_reason(), None, "refused after preparation");
        if attack == 3 {
            // The independently replayed cause is the Session's own retained
            // Diagnostic, distinguishable from a source/namespace mismatch
            // without any help text, clone or format.
            assert_eq!(
                refusal.refused().map(|r| r.cause()),
                Some(SelectedReplayCause::Session)
            );
            assert_eq!(refusal.receipt().expect("Session receipt").code, "SPX-H006");
        }
        let first = first_receipt(&mut refusal);
        if attack < 2 {
            assert_eq!(
                first,
                ReplayPhysicalRefusal::LocalLimit,
                "rejected on transfer"
            );
        }
        // The paid report restores no admission: the owner remains failed.
        let mut preparation = refusal.into_preparation().expect("retained owner");
        let anchor = lexical_anchor(&build);
        assert!(preparation.charge(anchor, 0, "restore").is_err());
        assert!(preparation
            .reserve_array::<u8>(anchor, 0, "restore")
            .is_err());
        assert!(preparation.try_charge(anchor, 0, "restore").is_err());
        drop(preparation);
        // The legacy pass-through route publishes the identical literal.
        assert_public(&build.source_graph_schemas().unwrap_err());
    }
}

#[test]
fn actual_caller_one_short_budget_refusal_retains_the_prepaid_receipt() {
    let build = build(LEXICAL_SOURCE);
    let (answer, _, overflowed, used) = replay_under(&build, MAX_CHANGE_BUILDER_BYTES);
    assert!(matches!(answer, Ok(true)) && !overflowed);
    let (answer, _, overflowed, _) = replay_under(&build, used - 1);
    let Err(refusal) = answer else {
        panic!("one byte short of the successful route must refuse")
    };
    assert!(overflowed);
    assert!(refusal.is_paid());
    assert_public(refusal.report().unwrap());
    let cause = refusal.refused().unwrap().cause();
    assert!(
        matches!(
            cause,
            SelectedReplayCause::Budget | SelectedReplayCause::Session
        ),
        "{cause:?}"
    );
    // Wherever the last debit falls, the first receipt is the budget's
    // prepaid refusal text, moved once rather than formatted afresh.
    let receipt = refusal.receipt().expect("prepaid budget receipt");
    assert_eq!(receipt.code, "SPX-H006");
    assert_eq!(
        receipt.message.capacity(),
        lexical_anchor(&build).id.as_str().len() + 512
    );
    assert!(receipt.message.starts_with("cleanup plan for function `"));
}

#[test]
fn reporting_views_publish_the_callers_prepaid_form_and_keep_success_unchanged() {
    const FORM: crate::cleanup_plan::SelectedReplayReportForm =
        crate::cleanup_plan::SelectedReplayReportForm::new("SPX-T781", "owning remap control");
    let retained = build(LEXICAL_SOURCE);
    assert_eq!(
        retained.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v77"
    );
    let view = retained.into_change_view_reporting(FORM).unwrap();
    assert!(view
        .modules
        .iter()
        .any(|module| module.source_graph_schema == "semaprax.graph.v77"));
    let forged = || {
        let mut forged = build(LEXICAL_SOURCE);
        writer_mut(&mut forged)
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap()
            .name
            .push('x');
        forged
    };
    let Err(report) = forged().into_change_view_reporting(FORM) else {
        panic!("forged retained lexical source must refuse")
    };
    assert_eq!(report.len(), 1);
    assert_eq!(
        (report[0].code, report[0].message.as_str()),
        (FORM.code(), FORM.message())
    );
    assert_eq!(report[0].message.capacity(), FORM.message().len());
    // The typed route: the paid carrier for exactly FORM moves out, while a
    // different requested form fails closed to the unpaid boundary.
    let Err(refusal) = forged().into_change_view_paid(FORM) else {
        panic!("forged retained lexical source must refuse")
    };
    let (published, moved) = measure(|| refusal.into_report(FORM));
    assert_eq!(moved.allocations, 0);
    assert_eq!(published.expect("paid FORM").len(), 1);
    let Err(refusal) = forged().into_change_view_paid(FORM) else {
        panic!("forged retained lexical source must refuse")
    };
    let (other, typed) = measure(|| refusal.into_report(REPORT));
    assert_eq!(typed.allocations, 0, "no diagnostic is rebuilt");
    assert_eq!(
        other
            .expect_err("a FORM carrier never stands in for G410")
            .reason(),
        SelectedReplayUnpaidReason::FormMismatch
    );
}
