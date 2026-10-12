//! Actual paid imported-helper classifier ownership through publication/Drop.
use super::*;
use crate::cleanup_plan::{
    ReplayPhysicalReservation, SelectedReplayReport, SelectedReplayReportForm,
};
use crate::test_allocation_observer::measure;

#[test]
fn paid_purity_local_refusal_keeps_original_ledger_and_report_through_drop() {
    // Source/compiler inputs and active-budget Rc are fixed context, excluded.
    let source = crate::check(
        "module purity; @id(\"purity.main\") fn main()->i64 {1}",
        "purity.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let function = &program.functions[0];
    let form = SelectedReplayReportForm::new(
        "SPX-G410",
        "retained static UTF-8 source-site authority disagrees",
    );
    let (((), metrics), overflowed, _) = crate::bounded_output::with_limit_usage(1 << 26, || {
        measure(|| {
            let acquired =
                SelectedReplayReport::try_acquire(ReplayPhysicalReservation::new(1 << 26), form)
                    .unwrap_or_else(|_| panic!("funded acquisition"));
            let backing = acquired.backing();
            let mut preparation = acquired
                .prepare(function)
                .unwrap_or_else(|_| panic!("funded setup"));
            let mut work = 0;
            let result = scalar_helpers::pure_records_checked(
                function,
                &program.types,
                &mut work,
                &mut |units| charge(&mut preparation, function, units),
            );
            assert!(matches!(
                result,
                Err(scalar_helpers::PurityRefusal::LocalLimit)
            ));
            let mut refusal = preparation.refuse(Refused::MISMATCH);
            let ledger = refusal.ledger_mut().unwrap() as *const ReplayPhysicalReservation;
            let charged = refusal.ledger_mut().unwrap().charged();
            assert!(charged > form.paid_bytes());
            let report = refusal.report().unwrap();
            assert_eq!((report.as_ptr(), report[0].message.as_ptr()), backing);
            assert_eq!(
                refusal.ledger_mut().unwrap() as *const ReplayPhysicalReservation,
                ledger
            );
            assert_eq!(refusal.ledger_mut().unwrap().charged(), charged);
            let report = refusal.into_published().expect("paid classifier refusal");
            assert_eq!((report.as_ptr(), report[0].message.as_ptr()), backing);
            drop(report);
        })
    });
    assert!(!overflowed && !metrics.invalid);
    assert!(metrics.allocated > 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(metrics.live, 0);
}
