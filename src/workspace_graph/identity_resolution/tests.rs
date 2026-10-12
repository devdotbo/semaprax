use super::*;
use crate::ast::program_source::SyntheticProgramView;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use std::mem::size_of;

const SOURCE: &str = "module identity.workspace; @id(\"helper\") fn helper(value:i64)->i64 {value+1} @id(\"main\") fn main()->i64 {helper(1)}";

fn report_bytes() -> usize {
    // Self + one Diagnostic slot + fixed 48-byte message, on the actual ledger.
    size_of::<IdentityReservation>() + size_of::<Diagnostic>() + 48
}

#[test]
fn owned_and_borrowed_inputs_match_the_legacy_oracle_and_pay_one_report_each() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let oracle = crate::hir::resolve_with_function_reuse(&program, None, None, None, None).unwrap();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // Each full resolution runs on its own equivalent, unchanged real
    // workspace builder ledger; one shared arbitrary 1 MiB ledger across both
    // refused the second one's independent-replay cursor.
    let before = super::acquisitions();
    let (first, failed, first_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            resolve_module(None, &program.path, &owned, None).unwrap()
        });
    assert!(!failed);
    assert_eq!(super::acquisitions(), before + 1);
    let (second, failed, second_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            resolve_module(None, &program.path, &borrowed, None).unwrap()
        });
    assert!(!failed);
    assert_eq!(super::acquisitions(), before + 2);
    for parts in [&first, &second] {
        assert_eq!(parts.0, oracle.0);
        assert_eq!((parts.2, parts.3), (0, oracle.1.static_text()));
        assert!(parts.1.is_empty());
    }
    // Each cold module acquired one fixed report plus its identity backings.
    assert!(first_used + second_used > 2 * report_bytes());
    assert!(first_used > report_bytes());
    assert!(second_used > report_bytes());
}

/// Additive observed control for the earlier shared arbitrary 1 MiB ledger:
/// the original first owned invocation exhausts the independent replay cursor.
/// The borrowed sibling and all retries preserve that sticky first receipt.
/// Both results remain live until Drop inside the allocation observation.
#[test]
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

#[test]
fn zero_and_one_short_setup_is_inline_copy_with_no_allocation_and_sticky_budget() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let owned = Input::Owned(program.clone());
    for limit in [0, report_bytes() - 1] {
        let ((failure, observed), failed, used) = with_limit_usage(limit, || {
            measure(|| {
                resolve_module(None, &program.path, &owned, None)
                    .err()
                    .unwrap()
            })
        });
        assert!(matches!(failure, ModuleFailure::Setup(Refused::Budget(_))));
        assert!(failed);
        assert_eq!(used, 0);
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.live, 0);
    }
}

/// Independent Arc<str> backing: two control words plus text, word padded.
fn backing(text_bytes: usize) -> usize {
    let word = size_of::<usize>();
    (2 * word + text_bytes).div_ceil(word) * word
}

/// Observes the ACTUAL `resolve_module` acquisition (test-only hook, no
/// authority change) through refusal, sticky retry and returned-owner Drop.
/// Only the module-resolution boundary is claimed; the public outer
/// Setup-to-Vec graph/report layer stays explicitly unqualified.
fn assert_refusal(
    input: &Input<'_>,
    path: &str,
    limit: usize,
    charged: usize,
    requested: usize,
    remaining: usize,
) {
    // Warm process-static ordinary tables outside the observation.
    with_limit_usage(1 << 20, || resolve_module(None, path, input, None).unwrap());
    let before = super::acquisitions();
    let (((), observed), failed, used, receipt) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let failure = resolve_module(None, path, input, None).err().unwrap();
            let ModuleFailure::Report(report) = failure else {
                panic!("actual identity refusal lost its published report")
            };
            // Exactly one acquisition; its original pointers are the returned ones.
            assert_eq!(super::acquisitions(), before + 1);
            let (report_pointer, text_pointer) = super::ACQUIRED.with(|a| a.get().1.unwrap());
            assert_eq!(report.as_ptr() as usize, report_pointer);
            assert_eq!(report[0].message.as_ptr() as usize, text_pointer);
            // Original layout: one slot and 48 text bytes, never regrown or cloned.
            assert_eq!(report.len(), 1);
            assert_eq!(report.capacity(), 1);
            assert_eq!(report[0].message.len(), 48);
            assert_eq!(report[0].message.capacity(), 48);
            assert_eq!(report[0].code, "SPX-H006");
            // Sticky: zero and fitting retries refuse, and a fresh module
            // attempt stops at setup with the first receipt and no allocation.
            assert!(crate::bounded_output::active_refused());
            assert_eq!(crate::bounded_output::active_remaining(), Some(remaining));
            assert!(!crate::bounded_output::reserve_active_required(0));
            assert!(!crate::bounded_output::reserve_active_required(1));
            let live = current_metrics();
            let retry = resolve_module(None, path, input, None).err().unwrap();
            let ModuleFailure::Setup(Refused::Budget(retried)) = retry else {
                panic!("sticky retry acquired another report")
            };
            assert_eq!(
                (retried.requested, retried.remaining),
                (requested, remaining)
            );
            let after = current_metrics();
            assert_eq!(after.allocations, live.allocations);
            assert_eq!(after.live, live.live);
            assert_eq!(super::acquisitions(), before + 1);
            // The returned owner drops inside the observation.
            drop(report);
        })
    });
    assert!(failed);
    assert_eq!(used, charged);
    let receipt = receipt.unwrap();
    assert_eq!(receipt.requested, requested);
    assert_eq!(receipt.remaining, remaining);
    assert_eq!(receipt.floor, 0);
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}

fn refusal_inputs() -> (crate::ast::Program, usize) {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    // Third initial identity is the first function's explicit stable id.
    let function = backing(program.functions[0].stable_id.len());
    (program, function)
}

#[test]
fn first_identity_refusal_publishes_the_acquired_report_and_keeps_the_first_receipt() {
    let (program, _) = refusal_inputs();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // The first initial identity is the prelude Option; 11 text bytes.
    let identity = backing(11);
    for input in [&owned, &borrowed] {
        let report = report_bytes();
        assert_refusal(input, &program.path, report, report, identity, 0);
        assert_refusal(
            input,
            &program.path,
            report + identity - 1,
            report,
            identity,
            identity - 1,
        );
    }
}

#[test]
fn later_identity_refusal_transfers_the_acquired_report_after_funded_identities() {
    let (program, function) = refusal_inputs();
    let owned = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    // Option then Result (both 11 bytes) precede the first function.
    let identity = backing(11);
    let report = report_bytes();
    for input in [&owned, &borrowed] {
        // Second identity one short, then at zero remaining.
        assert_refusal(
            input,
            &program.path,
            report + 2 * identity - 1,
            report + identity,
            identity,
            identity - 1,
        );
        assert_refusal(
            input,
            &program.path,
            report + identity,
            report + identity,
            identity,
            0,
        );
        // Exactly two funded: the fitting larger budget reaches the function.
        assert_refusal(
            input,
            &program.path,
            report + 2 * identity,
            report + 2 * identity,
            function,
            0,
        );
    }
}

#[test]
fn source_errors_stay_legacy_owned_after_one_report_acquisition() {
    let mut malformed = crate::parse(SOURCE, "identity-workspace.spx").unwrap();
    malformed.functions[1].stable_id = malformed.functions[0].stable_id.clone();
    let legacy = crate::hir::resolve_with_function_reuse(&malformed, None, None, None, None)
        .err()
        .unwrap();
    let input = Input::Owned(malformed);
    let (failure, failed, used) = with_limit_usage(1 << 20, || {
        resolve_module(None, "identity-workspace.spx", &input, None)
            .err()
            .unwrap()
    });
    let ModuleFailure::Report(report) = failure else {
        panic!("source error was routed as setup")
    };
    assert_eq!(report, legacy);
    assert!(!failed);
    // The fixed report is acquired first; the owned diagnostic itself is
    // legacy and equal to the unpaid oracle whatever stage raised it.
    assert!(used >= report_bytes());
}


fn linked_import_resolution<T>(operation: impl FnOnce()->T)->T {
    crate::vec_ops::with_authenticated_linked_source(|| {
        crate::box_ops::with_authenticated_linked_source(operation)
    })
}

#[test]
fn actual_selective_import_receiver_matches_fresh_owned_for_vec_and_box() {
    use crate::workspace_graph::{expected_projection,index_authored};
    use expected_projection::private_layout::registry::LayoutRegistry;
    let cases=[
        (include_str!("../../../std/collections/src/collections.spx"),
         "module receiver.vec; use function @id(\"std.collections.vec.with-capacity\") from std.collections as allocate; use function @id(\"std.collections.vec.push\") from std.collections as append; use function @id(\"std.collections.vec.get\") from std.collections as read; @id(\"receiver.helper\") fn helper()->i64 {1} @id(\"receiver.main\") fn main()->i64 {let values=allocate<i64>(2usize); let values=append<i64>(values,42); read<i64>(values,0usize)}"),
        (include_str!("../../../std/mem/src/mem.spx"),
         "module receiver.box; use function @id(\"std.mem.box.new\") from std.mem as allocate; use function @id(\"std.mem.box.into-inner\") from std.mem as consume; @id(\"receiver.helper\") fn helper()->i64 {1} @id(\"receiver.main\") fn main()->i64 {consume<i64>(allocate<i64>(42))}"),
    ];
    for (provider,caller) in cases {
        let previous=[crate::parse(caller,"caller.spx").unwrap(),crate::parse(provider,"provider.spx").unwrap()];
        let authored=index_authored(&previous).unwrap();
        let registry=LayoutRegistry::new(&previous,&authored);
        let old_key=expected_projection::synthetic_program_with_registry(&previous[0],&authored,&previous,Some(&registry)).unwrap();
        let layout=registry.get(&previous[0],&authored,&previous).unwrap();
        let (old_hir,old_work)=linked_import_resolution(|| crate::hir::resolve_with_function_reuse(
            &old_key,None,None,None,layout.presentation(&previous[0],&old_key).unwrap(),
        )).unwrap();
        let pass=FrontendPass::paid_receiver_candidate(old_key,old_hir,old_work.costs());
        // The edited main misses the complete-key cache; an unchanged helper
        // remains a genuine selective candidate under the original environment.
        let edited=caller.replace("42","43");
        let current=[crate::parse(&edited,"caller.spx").unwrap(),crate::parse(provider,"provider.spx").unwrap()];
        let authored=index_authored(&current).unwrap();
        let registry=LayoutRegistry::new(&current,&authored);
        let owned=Input::Owned(expected_projection::synthetic_program_with_registry(&current[0],&authored,&current,Some(&registry)).unwrap());
        let borrowed=Input::construct(&current[0],&authored,&current,&registry,false).unwrap();
        assert!(borrowed.borrowed().is_some());
        let layout=registry.get(&current[0],&authored,&current).unwrap();
        let oracle=linked_import_resolution(|| crate::hir::resolve_with_function_reuse(
            owned.owned().unwrap(),None,None,None,layout.presentation(&current[0],owned.source()).unwrap(),
        )).unwrap();
        for input in [&owned,&borrowed] {
            // The public unauthenticated entry continues to reject foreign
            // retained wrappers: moving a caller scope does not widen it.
            let outside=crate::hir::resolve_source(input.source(),layout.presentation(&current[0],input.source()).unwrap());
            assert!(outside.is_err());
            let before=super::acquisitions();
            let (result,refused,_)=with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES,|| {
                resolve_module(Some(&pass),"caller.spx",input,layout.presentation(&current[0],input.source()).unwrap())
            });
            assert!(!refused);let result=result.unwrap();
            assert_eq!(result.0,oracle.0);assert_eq!(result.3,oracle.1.static_text());
            assert!(result.2>0);assert_eq!(super::acquisitions(),before+1);
            assert!(crate::hir::resolve_source(input.source(),layout.presentation(&current[0],input.source()).unwrap()).is_err());
        }
        // A retained generic wrapper with the wrong persistent identity must
        // still undergo ordinary source verification, even with a warm cache.
        let mut hostile=owned.into_owned().unwrap();
        let wrapper=hostile.functions.iter_mut().find(|function| !function.type_parameters.is_empty()).unwrap();
        wrapper.stable_id="receiver.wrong.wrapper".to_owned();
        let hostile=Input::Owned(hostile);
        let expected=linked_import_resolution(|| crate::hir::resolve_source(hostile.source(),layout.presentation(&current[0],hostile.source()).unwrap())).unwrap_err();
        let (actual,refused,_)=with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES,|| {
            resolve_module(Some(&pass),"caller.spx",&hostile,layout.presentation(&current[0],hostile.source()).unwrap())
        });
        assert!(!refused);
        let ModuleFailure::Report(actual)=actual.unwrap_err() else {panic!("wrong identity must preserve semantic report");};
        assert_eq!(actual,expected);
    }
}
