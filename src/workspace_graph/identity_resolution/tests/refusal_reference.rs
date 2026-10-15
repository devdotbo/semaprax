//! Verbatim historical physical-threshold control, retained for source review.
//! Later paid representations change its threshold; this is not a current gate.
use super::*;
#[allow(dead_code)]
fn one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let input = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // Warm process-static ordinary tables outside the observation.
    with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        resolve_module(None, &program.path, &input, None).unwrap()
    });
    let ((outcome, observed), failed, _, receipt) = with_limit_usage_refusal(1 << 20, || {
        measure(|| {
            let stage0 = resolve_module(None, &program.path, &input, None);
            let refused_after_stage0 = crate::bounded_output::active_refused();
            let stage1 = resolve_module(None, &program.path, &borrowed, None);
            let sticky_zero = crate::bounded_output::reserve_active_required(0);
            let sticky_fit = crate::bounded_output::reserve_active_required(1);
            let live = current_metrics();
            let retry = resolve_module(None, &program.path, &input, None);
            let after = current_metrics();
            let unchanged = (after.allocations, after.live) == (live.allocations, live.live);
            let retry_setup = matches!(retry, Err(ModuleFailure::Setup(Refused::Budget(_))));
            let second_setup = matches!(stage1, Err(ModuleFailure::Setup(Refused::Budget(_))));
            let message = match &stage0 {
                Err(ModuleFailure::Report(report)) => report.iter().any(|d| {
                    d.message
                        .contains("physical expression-fact cursor reservation")
                }),
                _ => false,
            };
            let first_code = match &stage0 {
                Err(ModuleFailure::Report(report)) => report.first().map(|d| d.code),
                _ => None,
            };
            drop((stage0, stage1, retry));
            (
                (second_setup, refused_after_stage0, first_code, message),
                (sticky_zero, sticky_fit, retry_setup, unchanged),
            )
        })
    });
    let ((second_setup, refused_after_stage0, first_code, message), sticky) = outcome;
    assert!(
        second_setup,
        "borrowed sibling must preserve the first refusal at setup"
    );
    assert!(
        refused_after_stage0,
        "the original first owned call exhausted this ledger"
    );
    assert!(failed);
    assert_eq!(first_code, Some("SPX-H006"));
    assert!(
        message,
        "first owned run must refuse the physical expression-fact cursor reservation"
    );
    assert_eq!(sticky, (false, false, true, true));
    let receipt = receipt.expect("the first global refusal is recorded");
    assert!(receipt.requested > receipt.remaining);
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
