//! Additive controls for the leased oracle account and its payer routes.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::allocation::{AdmissionMode, AllocationPayer};
use crate::cleanup_plan::replay::ReplayBudget;
use crate::hir::funded::FundedError;
use crate::hir::ResolvedFunction;

fn program() -> crate::hir::ResolvedProgram {
    let source = crate::check(
        "module replay.oracle; @id(\"oracle.main\") fn main()->i64 {7}",
        "oracle-account.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}

fn account_cost(function: &ResolvedFunction, bytes: usize) -> usize {
    // Positive charges bootstrap the refusal backing first: id + 512 bytes.
    function.id.as_str().len() + 512 + bytes
}

#[test]
fn account_charges_cumulatively_and_first_refusal_is_sticky() {
    let scope = OracleScope::open(10).unwrap();
    let lease = scope.lease();
    lease.start().unwrap();
    assert_eq!(lease.charge(4), Ok(()));
    assert_eq!(lease.charge(0), Ok(()));
    assert_eq!(lease.charge(6), Ok(()));
    assert_eq!(
        lease.charge(1),
        Err(OracleRefusal::Exhausted { requested: 1 })
    );
    // Zero and smaller retries remain refused after the first refusal.
    assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
    assert_eq!(lease.charge(1), Err(OracleRefusal::Sticky));
    // A start after the refusal is rejected; completing the earlier one is fine.
    assert_eq!(lease.start(), Err(OracleRefusal::Sticky));
    lease.complete().unwrap();
    let receipt = scope.finish();
    assert_eq!(receipt.allowance, 10);
    assert_eq!(receipt.charged, 10);
    assert_eq!(
        receipt.first_refusal,
        Some(OracleFirstRefusal {
            requested: 1,
            remaining: 0,
            ordinal: 4
        })
    );
    // Attempts include the two sticky retries.
    assert_eq!(receipt.calls, 6);
    assert_eq!((receipt.started, receipt.completed), (1, 1));
}

#[test]
fn first_receipt_keeps_requested_remaining_and_ordinal_across_retries() {
    let scope = OracleScope::open(10).unwrap();
    let lease = scope.lease();
    lease.charge(7).unwrap();
    assert_eq!(
        lease.charge(5),
        Err(OracleRefusal::Exhausted { requested: 5 })
    );
    assert_eq!(lease.charge(1), Err(OracleRefusal::Sticky));
    let receipt = scope.finish();
    assert_eq!(receipt.charged, 7);
    assert_eq!(
        receipt.first_refusal,
        Some(OracleFirstRefusal {
            requested: 5,
            remaining: 3,
            ordinal: 2
        })
    );
    assert_eq!(receipt.calls, 3);
}

#[test]
fn complete_cannot_exceed_started_and_counters_fail_closed_on_wrap() {
    let scope = OracleScope::open(4).unwrap();
    let lease = scope.lease();
    assert_eq!(lease.complete(), Err(OracleRefusal::Unbalanced));
    lease.start().unwrap();
    lease.complete().unwrap();
    assert_eq!(lease.complete(), Err(OracleRefusal::Unbalanced));
    let receipt = scope.receipt();
    assert_eq!((receipt.started, receipt.completed), (1, 1));
    // Wrapped call count refuses and records the first refusal instead of aliasing.
    ACCOUNT.with(|account| {
        let mut state = account.get().unwrap();
        state.receipt.calls = usize::MAX;
        state.receipt.started = usize::MAX;
        account.set(Some(state));
    });
    assert_eq!(lease.charge(0), Err(OracleRefusal::Overflow));
    assert!(lease.refused());
    assert_eq!(lease.start(), Err(OracleRefusal::Sticky));
    drop(scope);
    // A wrapped epoch counter refuses to open and leaves no account behind.
    EPOCH.with(|epoch| epoch.set(u64::MAX));
    assert_eq!(OracleScope::open(1).err(), Some(OracleRefusal::Overflow));
    assert!(ACCOUNT.with(|account| account.get().is_none()));
    EPOCH.with(|epoch| epoch.set(0));
}

// Each probe becomes an ambiguous (compile error) call if the type is Send/Sync.
trait AmbiguousIfSend<A> {
    fn probe() {}
}
impl<T: ?Sized> AmbiguousIfSend<()> for T {}
impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
trait AmbiguousIfSync<A> {
    fn probe() {}
}
impl<T: ?Sized> AmbiguousIfSync<()> for T {}
impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}

#[test]
fn lease_and_scope_are_neither_send_nor_sync() {
    <OracleLease as AmbiguousIfSend<_>>::probe();
    <OracleLease as AmbiguousIfSync<_>>::probe();
    <OracleScope as AmbiguousIfSend<_>>::probe();
    <OracleScope as AmbiguousIfSync<_>>::probe();
}

#[test]
fn zero_allowance_refuses_one_byte_and_authenticates_zero() {
    let scope = OracleScope::open(0).unwrap();
    assert_eq!(scope.lease().charge(0), Ok(()));
    assert_eq!(
        scope.lease().charge(1),
        Err(OracleRefusal::Exhausted { requested: 1 })
    );
    assert_eq!(scope.lease().charge(0), Err(OracleRefusal::Sticky));
}

#[test]
fn nested_missing_expired_and_unwound_leases_fail_closed() {
    let scope = OracleScope::open(8).unwrap();
    assert!(OracleScope::open(8).is_err());
    let stale = scope.lease();
    assert_eq!(stale.charge(1), Ok(()));
    drop(scope);
    assert_eq!(stale.charge(0), Err(OracleRefusal::Missing));
    assert!(stale.refused());
    let next = OracleScope::open(8).unwrap();
    assert_eq!(stale.charge(0), Err(OracleRefusal::Expired));
    assert_eq!(next.receipt().charged, 0);
    drop(next);
    let unwound = std::panic::catch_unwind(|| {
        let _scope = OracleScope::open(1).unwrap();
        panic!("unwind with a live oracle scope");
    });
    assert!(unwound.is_err());
    // The panic restored the thread, so a new scope may be opened.
    drop(OracleScope::open(1).unwrap());
}

#[test]
fn oracle_payer_never_touches_production_active_and_ordinary_routes_stay() {
    let program = program();
    let function = &program.functions[0];
    let (receipt, refused, charged, production) = with_limit_usage_refusal(1 << 20, || {
        let scope = OracleScope::open(account_cost(function, 64)).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, scope.lease());
        assert_eq!(budget.admission, AdmissionMode::Ordinary);
        assert_ne!(budget.payer, AllocationPayer::SelectedReservation);
        budget.reserve_bytes(function, 64, "exact").unwrap();
        let refusal = budget
            .reserve_bytes(function, 1, "one byte over")
            .unwrap_err();
        assert!(refusal.message.contains("physical reservation exhausted"));
        // Sticky: even the zero request is now refused, on the account only.
        assert!(budget.reserve_bytes(function, 0, "zero").is_err());
        scope.finish()
    });
    assert!(!refused && production.is_none());
    assert_eq!(charged, 0, "production ACTIVE must not be charged");
    assert_eq!(receipt.charged, account_cost(function, 64));
    let first = receipt.first_refusal.unwrap();
    assert_eq!((first.requested, first.remaining), (1, 0));
    // Outside an opted scope ordinary constructors keep their original payer.
    assert_eq!(
        ReplayBudget::unobserved().payer,
        AllocationPayer::LegacyUnobserved
    );
    let (_, _, _, _) = with_limit_usage_refusal(usize::MAX, || {
        assert_eq!(ReplayBudget::new().payer, AllocationPayer::ActiveBuilder);
        assert_eq!(
            ReplayBudget::with_skeleton_limit(0).payer,
            AllocationPayer::ActiveBuilder
        );
    });
}

#[test]
fn funded_renewal_assembly_pays_the_leased_account_exact_short_zero_and_sticky() {
    let source = include_str!("../../../../../tests/owned_data/byte_helper_renewal.spx");
    let checked = crate::check(source, "oracle-funded-renewal.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    // Returns (helpers published?, receipt, sticky zero-byte retry) of one run.
    let run = |allowance: usize| {
        crate::test_allocation_observer::measure(|| {
            let scope = OracleScope::open(allowance).unwrap();
            let lease = scope.lease();
            let result = crate::cleanup_plan::renewal_profile::bindings_funded(
                &program,
                main,
                &mut |bytes| lease.charge(bytes),
            )
            .map(|sites| sites.has_helpers());
            let retry = lease.charge(0);
            (result, scope.finish(), retry)
        })
    };
    let ((result, receipt, retry), metrics) = run(1 << 20);
    assert_eq!(result, Ok(true));
    assert_eq!(retry, Ok(()));
    let required = receipt.charged;
    assert!(required > 0 && receipt.first_refusal.is_none());
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated <= required);
    let ((exact, receipt, _), metrics) = run(required);
    assert_eq!(exact, Ok(true));
    assert_eq!(receipt.charged, required);
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated <= required);
    let ((short, receipt, retry), metrics) = run(required - 1);
    let first = receipt.first_refusal.expect("one byte short refuses");
    assert_eq!(
        short,
        Err(FundedError::Refused(OracleRefusal::Exhausted {
            requested: first.requested
        }))
    );
    assert!(first.requested > first.remaining && receipt.charged < required);
    assert_eq!(retry, Err(OracleRefusal::Sticky));
    assert!(!metrics.invalid && metrics.live == 0);
    let ((zero, receipt, retry), metrics) = run(0);
    assert!(matches!(
        zero,
        Err(FundedError::Refused(OracleRefusal::Exhausted { .. }))
    ));
    assert_eq!(receipt.charged, 0);
    assert_eq!(receipt.first_refusal.map(|first| first.remaining), Some(0));
    assert_eq!(retry, Err(OracleRefusal::Sticky));
    assert!(!metrics.invalid && metrics.live == 0);
}

#[test]
fn work_account_is_finite_checked_sticky_and_scope_bound() {
    // `open` grants no work units: zero authenticates, any unit refuses, and the
    // refusal is sticky for bytes, work and starts alike.
    let scope = OracleScope::open(8).unwrap();
    let lease = scope.lease();
    assert_eq!(lease.charge_work(0), Ok(()));
    assert_eq!(
        lease.charge_work(1),
        Err(OracleRefusal::Exhausted { requested: 1 })
    );
    assert_eq!(lease.charge_work(0), Err(OracleRefusal::Sticky));
    assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
    assert!(lease.refused());
    assert_eq!(lease.start(), Err(OracleRefusal::Sticky));
    let receipt = scope.finish();
    assert_eq!(
        (receipt.work_allowance, receipt.work, receipt.work_calls),
        (0, 0, 3)
    );
    assert_eq!(
        receipt.first_work_refusal,
        Some(OracleFirstRefusal {
            requested: 1,
            remaining: 0,
            ordinal: 2
        })
    );
    assert_eq!(
        (receipt.first_refusal, receipt.charged, receipt.calls),
        (None, 0, 1)
    );
    // Cumulative exact allowance; work units never consume bytes.
    let scope = OracleScope::open_with_work(4, 10).unwrap();
    let lease = scope.lease();
    assert_eq!(lease.charge(4), Ok(()));
    for units in [4, 6, 0] {
        assert_eq!(lease.charge_work(units), Ok(()));
    }
    assert_eq!(
        lease.charge_work(1),
        Err(OracleRefusal::Exhausted { requested: 1 })
    );
    assert_eq!(lease.charge_work(0), Err(OracleRefusal::Sticky));
    let receipt = scope.finish();
    assert_eq!(
        (receipt.charged, receipt.work, receipt.work_calls),
        (4, 10, 5)
    );
    assert_eq!(
        receipt.first_work_refusal,
        Some(OracleFirstRefusal {
            requested: 1,
            remaining: 0,
            ordinal: 4
        })
    );
    // A wrapped total refuses as overflow; it never aliases a smaller total.
    let scope = OracleScope::open_with_work(0, usize::MAX).unwrap();
    let lease = scope.lease();
    assert_eq!(lease.charge_work(usize::MAX), Ok(()));
    assert_eq!(lease.charge_work(1), Err(OracleRefusal::Overflow));
    assert_eq!(lease.charge_work(0), Err(OracleRefusal::Sticky));
    let receipt = scope.finish();
    assert_eq!(receipt.work, usize::MAX);
    assert_eq!(
        receipt.first_work_refusal,
        Some(OracleFirstRefusal {
            requested: 1,
            remaining: 0,
            ordinal: 2
        })
    );
    // A wrapped attempt counter refuses and records the first refusal.
    let scope = OracleScope::open_with_work(0, 5).unwrap();
    let lease = scope.lease();
    ACCOUNT.with(|account| {
        let mut state = account.get().unwrap();
        state.receipt.work_calls = usize::MAX;
        account.set(Some(state));
    });
    assert_eq!(lease.charge_work(0), Err(OracleRefusal::Overflow));
    assert!(lease.refused());
    assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
    assert_eq!(
        scope.finish().first_work_refusal,
        Some(OracleFirstRefusal {
            requested: 0,
            remaining: 5,
            ordinal: usize::MAX
        })
    );
    // Nested scopes cannot widen the allowance; stale leases fail closed.
    let scope = OracleScope::open_with_work(0, 3).unwrap();
    assert!(OracleScope::open_with_work(0, 100).is_err());
    let stale = scope.lease();
    assert_eq!(stale.charge_work(1), Ok(()));
    drop(scope);
    assert_eq!(stale.charge_work(0), Err(OracleRefusal::Missing));
    assert!(stale.refused());
    let next = OracleScope::open_with_work(0, 3).unwrap();
    assert_eq!(stale.charge_work(0), Err(OracleRefusal::Expired));
    assert_eq!(next.receipt().work, 0);
    drop(next);
    let unwound = std::panic::catch_unwind(|| {
        let scope = OracleScope::open_with_work(0, 1).unwrap();
        scope.lease().charge_work(1).unwrap();
        panic!("unwind with a live work account");
    });
    assert!(unwound.is_err());
    let fresh = OracleScope::open_with_work(0, 1).unwrap();
    assert_eq!(
        fresh.receipt().work,
        0,
        "the unwound account left no work behind"
    );
}

const HELPERS: &str = include_str!("../../../../../tests/owned_data/byte_helper_renewal.spx");

#[test]
fn byte_helper_census_work_callback_charges_the_cumulative_account_work_receipt() {
    use crate::loop_calls::byte_writer::{
        checked_bindings, checked_bindings_funded_with_work, HelperBindings,
    };
    let checked = crate::check(HELPERS, "oracle-census-work.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let names = |sites: HelperBindings<'_>| {
        sites
            .into_iter()
            .map(|(at, binding)| (at.as_str().to_owned(), binding.id.as_str().to_owned()))
            .collect::<Vec<_>>()
    };
    let old = names(checked_bindings(&program, main).unwrap());
    assert!(!old.is_empty());
    let run = |bytes: usize, work: usize| {
        let scope = OracleScope::open_with_work(bytes, work).unwrap();
        let lease = scope.lease();
        let result = checked_bindings_funded_with_work(
            &program,
            main,
            &mut |b| lease.charge(b),
            &mut |units| lease.charge_work(units),
        )
        .map(names);
        let retry = (lease.charge(0), lease.charge_work(0));
        (result, scope.finish(), retry)
    };
    let (result, receipt, retry) = run(1 << 20, 1 << 25);
    assert_eq!(result, Ok(old.clone()));
    assert_eq!(retry, (Ok(()), Ok(())));
    let (bytes, work) = (receipt.charged, receipt.work);
    assert!(
        bytes > 0 && work > 0,
        "census bytes and work are both charged"
    );
    assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none());
    let (exact, receipt, _) = run(bytes, work);
    assert_eq!(exact, Ok(old));
    assert_eq!((receipt.charged, receipt.work), (bytes, work));
    // One work unit short refuses on the work account only, unchanged and sticky.
    let (short, receipt, retry) = run(1 << 20, work - 1);
    let first = receipt
        .first_work_refusal
        .expect("work refusal is recorded");
    assert_eq!(
        short,
        Err(FundedError::Refused(OracleRefusal::Exhausted {
            requested: first.requested
        }))
    );
    assert!(first.requested > first.remaining && receipt.work < work);
    assert!(receipt.first_refusal.is_none());
    assert_eq!(
        retry,
        (Err(OracleRefusal::Sticky), Err(OracleRefusal::Sticky))
    );
    // `open` itself grants no work, so the census refuses at its first unit.
    let (zero, receipt, _) = run(1 << 20, 0);
    assert!(matches!(
        zero,
        Err(FundedError::Refused(OracleRefusal::Exhausted { .. }))
    ));
    assert_eq!(receipt.work, 0);
    // One byte short refuses on the byte account; work stays below its total.
    let (short, receipt, retry) = run(bytes - 1, 1 << 25);
    assert!(matches!(
        short,
        Err(FundedError::Refused(OracleRefusal::Exhausted { .. }))
    ));
    assert!(receipt.first_refusal.is_some() && receipt.first_work_refusal.is_none());
    assert_eq!(
        retry,
        (Err(OracleRefusal::Sticky), Err(OracleRefusal::Sticky))
    );
    // The ordinary depth ceiling is unchanged: a generous account buys nothing,
    // and the census verdict is not an account refusal.
    let mut deep = main.clone();
    let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut deep.body.kind else {
        panic!("main block")
    };
    for _ in 0..600 {
        let inner = tail.clone();
        tail.kind = crate::hir::ResolvedExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: inner,
        };
    }
    assert!(checked_bindings(&program, &deep).is_err());
    let scope = OracleScope::open_with_work(1 << 20, 1 << 25).unwrap();
    let lease = scope.lease();
    assert!(matches!(
        checked_bindings_funded_with_work(
            &program,
            &deep,
            &mut |b| lease.charge(b),
            &mut |units| lease.charge_work(units),
        ),
        Err(FundedError::Census(_))
    ));
    let receipt = scope.finish();
    assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none());
}

type Verdict = Result<bool, crate::hir::funded::FundedError<OracleRefusal>>;
type Build<'f, O> = dyn Fn(
        crate::hir::funded::Reserve<'_, OracleRefusal>,
    ) -> Result<O, crate::hir::funded::FundedError<OracleRefusal>>
    + 'f;

/// One scoped finite run. The account is owned before the production ledger
/// scope and the allocation observer; the fixture and the expected snapshot are
/// built by the caller outside. Inside the measured scope the output is built,
/// compared in place without allocating, and dropped; only a heap-free verdict
/// leaves it.
fn measured<O>(
    allowance: usize,
    build: &Build<'_, O>,
    same: &dyn Fn(&O) -> bool,
) -> (
    Verdict,
    OracleReceipt,
    Result<(), OracleRefusal>,
    crate::test_allocation_observer::Metrics,
) {
    let scope = OracleScope::open(allowance).unwrap();
    let lease = scope.lease();
    let ((verdict, retry, metrics), refused, active) =
        crate::bounded_output::with_limit_usage(0, || {
            let ((verdict, retry), metrics) = crate::test_allocation_observer::measure(|| {
                let verdict = build(&mut |bytes| lease.charge(bytes)).map(|output| same(&output));
                (verdict, lease.charge(0))
            });
            (verdict, retry, metrics)
        });
    assert!(!refused);
    assert_eq!(active, 0, "the production ledger is never charged");
    (verdict, scope.finish(), retry, metrics)
}

/// Finite baseline, exact, one-byte-short and zero controls against the finite
/// account. No measured byte is subtracted or normalized: every run, refused
/// ones included, must have allocated no more than the account was charged.
fn assert_measured_controls<O>(label: &str, build: &Build<'_, O>, same: &dyn Fn(&O) -> bool) {
    let within = |metrics: &crate::test_allocation_observer::Metrics, receipt: &OracleReceipt| {
        assert!(!metrics.invalid, "{label}");
        assert_eq!(metrics.live, 0, "{label}: output dropped inside the scope");
        assert!(
            metrics.allocated <= receipt.charged,
            "{label}: {metrics:?} {receipt:?}"
        );
    };
    let (verdict, receipt, retry, metrics) = measured(1 << 20, build, same);
    assert_eq!(verdict, Ok(true), "{label}");
    assert_eq!(retry, Ok(()));
    let required = receipt.charged;
    assert!(required > 0 && receipt.first_refusal.is_none(), "{label}");
    assert!(
        metrics.allocated > 0,
        "{label}: the measured run really allocates"
    );
    within(&metrics, &receipt);
    let (verdict, receipt, _, metrics) = measured(required, build, same);
    assert_eq!(verdict, Ok(true), "{label}");
    assert_eq!(receipt.charged, required);
    assert!(receipt.first_refusal.is_none());
    within(&metrics, &receipt);
    let (verdict, receipt, retry, metrics) = measured(required - 1, build, same);
    let first = receipt.first_refusal.expect("one byte short refuses");
    assert_eq!(
        verdict,
        Err(FundedError::Refused(OracleRefusal::Exhausted {
            requested: first.requested
        })),
        "{label}"
    );
    assert_eq!(first.remaining, required - 1 - receipt.charged, "{label}");
    assert!(first.requested > first.remaining && receipt.charged < required);
    assert_eq!(retry, Err(OracleRefusal::Sticky));
    within(&metrics, &receipt);
    let (verdict, receipt, retry, metrics) = measured(0, build, same);
    assert!(matches!(
        verdict,
        Err(FundedError::Refused(OracleRefusal::Exhausted { .. }))
    ));
    assert_eq!(receipt.charged, 0);
    assert_eq!(receipt.first_refusal.map(|first| first.remaining), Some(0));
    assert_eq!(retry, Err(OracleRefusal::Sticky));
    assert_eq!(
        metrics.allocated, 0,
        "{label}: nothing is built before its debit"
    );
    within(&metrics, &receipt);
}

const CONCAT: &str = r#"module oracle.concat;
@id("o.main") fn main()->i64 {
 let mut text=string_from_i64(1);
 let mut count=0;
 while count<2 {
  text=string_concat(text,"x");
  count=count+1;
  0
 }
 string_len(text)
}"#;

const READS: &str = r#"module oracle.reads;
@id("o.reads") fn reads(text: own string)->i64 {
 let mut owner=text;
 let mut count=0;
 while string_len(owner)>0 && string_len(owner)<4 {
  owner=string_concat(owner,"x");
  count=count+1;
  0
 }
 count
}
@id("app.main") fn main()->i64 {0}
"#;

#[test]
fn measured_assembly_output_is_paid_by_the_finite_account_through_drop() {
    let checked = crate::check(HELPERS, "oracle-measured-assembly.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ids: Vec<_> = crate::loop_calls::byte_writer::checked_bindings(&program, main)
        .unwrap()
        .into_iter()
        .map(|(at, _)| at.clone())
        .collect();
    let old = crate::cleanup_plan::renewal_profile::bindings(&program, main);
    let expected: Vec<Option<String>> = ids
        .iter()
        .map(|at| old.get(at).map(|binding| binding.id.as_str().to_owned()))
        .collect();
    assert!(old.has_helpers() && expected.iter().any(Option::is_some));
    let has_helpers = old.has_helpers();
    assert_measured_controls(
        "assembly",
        &|reserve| crate::cleanup_plan::renewal_profile::bindings_funded(&program, main, reserve),
        &|sites| {
            sites.has_helpers() == has_helpers
                && ids.iter().zip(&expected).all(|(at, want)| {
                    sites.get(at).map(|binding| binding.id.as_str()) == want.as_deref()
                })
        },
    );
}

#[test]
fn measured_concat_moving_and_reads_inventories_are_paid_through_drop() {
    let checked = crate::check(CONCAT, "oracle-measured-concat.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = &program.functions[0];
    let operands = crate::string_ops::same_owner_concat_operands(function);
    assert!(!operands.is_empty());
    assert_measured_controls(
        "concat",
        &|reserve| crate::string_ops::same_owner_concat_operands_funded(function, reserve),
        &|out| out.iter().eq(operands.iter()),
    );
    let moving = crate::literal_format::checked_moving_string_operands(function).unwrap();
    assert!(!moving.is_empty());
    assert_measured_controls(
        "moving",
        &|reserve| crate::literal_format::checked_moving_string_operands_funded(function, reserve),
        &|out| out.iter().eq(moving.iter()),
    );
    let diagnostics = crate::check(READS, "oracle-measured-reads.spx").unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-O002");
    assert_eq!(
        diagnostics[0].message,
        "string parameters own their input implicitly; an explicit ownership mode is not admitted"
    );
    let admitted = READS.replace("text: own string", "text: string");
    let checked = crate::check(&admitted, "oracle-measured-reads.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let reads_fn = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "o.reads")
        .unwrap();
    let reads = crate::string_ops::conditions::function_reads(reads_fn);
    assert!(!reads.is_empty());
    assert_measured_controls(
        "function_reads",
        &|reserve| crate::string_ops::conditions::function_reads_funded(reads_fn, reserve),
        &|out| out.iter().eq(reads.iter()),
    );
}

#[test]
fn measured_single_site_binding_pays_every_fallback_before_acquiring() {
    let source = r#"module oracle.site;
@id("app.main") fn main()->i64 {
 let mut values=vec_with_capacity<i64>(1usize);
 let mut index=0usize;
 while index<1usize {
  values=vec_push<i64>(values,1);
  index=index+1usize;
  0
 }
 if vec_len<i64>(values)==1usize {7}else{0}
}
"#;
    let checked = crate::check(source, "oracle-measured-site.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let main = &program.functions[0];
    let at = crate::hir::vec_loop_renewal::bindings_in(&program, main)
        .into_keys()
        .next()
        .expect("the vec loop publishes a renewal site");
    let expected =
        crate::cleanup_plan::renewal_profile::binding_with_index(&program, main, &at, None)
            .map(|binding| binding.id.as_str().to_owned());
    assert!(expected.is_some());
    assert_measured_controls(
        "single_site",
        &|reserve| {
            crate::cleanup_plan::renewal_profile::binding_with_index_funded(
                &program, main, &at, None, reserve,
            )
        },
        &|found| found.map(|binding| binding.id.as_str()) == expected.as_deref(),
    );
}

#[test]
fn oracle_payer_with_expired_lease_refuses_without_production_fallback() {
    let program = program();
    let function = &program.functions[0];
    let stale = OracleScope::open(1 << 12).unwrap().lease();
    let (result, _, charged, _) = with_limit_usage_refusal(1 << 20, || {
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, stale);
        budget.reserve_bytes(function, 8, "expired")
    });
    assert!(result.is_err());
    assert_eq!(charged, 0);
}

#[test]
fn finite_account_work_refusal_census_carrier_is_observed_through_drop() {
    let checked = crate::check(
        include_str!("../../../../../tests/owned_data/byte_helper_renewal.spx"),
        "oracle-census-refusal.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    // Own the independent finite account before the production ledger starts.
    let scope = OracleScope::open_with_work(1 << 20, 0).unwrap();
    let lease = scope.lease();
    let (((), metrics), refused, active) = crate::bounded_output::with_limit_usage(0, || {
        crate::test_allocation_observer::measure(|| {
            let result = crate::loop_calls::byte_writer::checked_bindings_funded_with_work(
                &program,
                main,
                &mut |bytes| lease.charge(bytes),
                &mut |units| lease.charge_work(units),
            );
            assert!(matches!(
                result,
                Err(FundedError::Refused(OracleRefusal::Exhausted {
                    requested: 1
                }))
            ));
            drop(result);
            assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
        })
    });
    assert!(!refused);
    assert_eq!(active, 0);
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(
        metrics.allocated, 0,
        "refusal constructs only the inline carrier"
    );
    let receipt = scope.finish();
    assert!(receipt.charged > 0 && receipt.charged <= receipt.allowance);
    let first = receipt.first_work_refusal.unwrap();
    assert_eq!((first.requested, first.remaining, first.ordinal), (1, 0, 1));
    assert!(receipt.first_refusal.is_none());
}

#[test]
fn account_type_probe_fast_receivers_match_original_without_production_ledger_touch() {
    use original_fast_type_probe as nested_shape;
    let program = program();
    let function = &program.functions[0];
    let types = [
        crate::hir::ResolvedType::Never,
        crate::hir::ResolvedType::I64,
        crate::hir::ResolvedType::Bytes,
        crate::hir::ResolvedType::String,
    ];
    let original: Vec<_> = types
        .iter()
        .map(|ty| crate::cleanup::type_needs_resource_cleanup(&program, ty).unwrap())
        .collect();
    let scope = OracleScope::open(0).unwrap();
    let lease = scope.lease();
    let (((), metrics), refused, active) = crate::bounded_output::with_limit_usage(0, || {
        crate::test_allocation_observer::measure(|| {
            let invocation = super::enter(&program, function, lease).unwrap();
            for (ty, original) in types.iter().zip(&original) {
                let received = nested_shape::type_needs_drop(&program, function, ty).unwrap();
                assert_eq!(received, *original);
            }
            assert_eq!(invocation.exit(), Ok(()));
        })
    });
    assert!(!refused);
    assert_eq!(active, 0);
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, 0);
    assert_eq!(metrics.live, 0);
    assert_eq!(scope.receipt().charged, 0);
    assert!(scope.receipt().first_refusal.is_none());
    assert!(scope.receipt().physical_failure.is_none());
}

mod original_fast_type_probe {
    use super::*;
    pub(super) fn type_needs_drop(
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        ty: &crate::hir::ResolvedType,
    ) -> Result<bool, crate::diagnostic::Diagnostic> {
        crate::cleanup_plan::replay::paid_type_facts::tests::original_fast_probe(
            program, function, ty,
        )
    }
}

#[test]
fn funded_type_probe_receivers_keep_exact_work_bytes_zero_short_sticky_and_complete_drop() {
    let program = program();
    let function = &program.functions[0];
    let types = [
        crate::hir::ResolvedType::Never,
        crate::hir::ResolvedType::I64,
        crate::hir::ResolvedType::Bytes,
        crate::hir::ResolvedType::String,
    ];
    let original: Vec<_> = types
        .iter()
        .map(|ty| crate::cleanup::type_needs_resource_cleanup(&program, ty).unwrap())
        .collect();
    let run = |bytes, work| {
        let scope = OracleScope::open_with_work(bytes, work).unwrap();
        let lease = scope.lease();
        let ((failed, metrics), refused, active) =
            crate::bounded_output::with_limit_usage(0, || {
                crate::test_allocation_observer::measure(|| {
                    let invocation = super::enter(&program, function, lease).unwrap();
                    let result = (|| {
                        for (ty, expected) in types.iter().zip(&original) {
                            let found = crate::cleanup_plan::replay::nested_shape::type_needs_drop(
                                &program, function, ty,
                            )?;
                            assert_eq!(found, *expected);
                        }
                        Ok::<(), crate::diagnostic::Diagnostic>(())
                    })();
                    let failed = result.is_err();
                    if failed {
                        let first = scope.receipt();
                        let before = crate::test_allocation_observer::current_metrics();
                        let retry = crate::cleanup_plan::replay::nested_shape::type_needs_drop(
                            &program, function, &types[0],
                        );
                        assert!(retry.is_err());
                        drop(retry);
                        assert_eq!(
                            before.allocated,
                            crate::test_allocation_observer::current_metrics().allocated
                        );
                        let after = scope.receipt();
                        assert_eq!(after.first_refusal, first.first_refusal);
                        assert_eq!(after.first_work_refusal, first.first_work_refusal);
                        assert_eq!(after.physical_failure, first.physical_failure);
                        assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
                        assert_eq!(lease.charge_work(0), Err(OracleRefusal::Sticky));
                    }
                    drop(result);
                    assert_eq!(invocation.exit(), Ok(()));
                    failed
                })
            });
        let receipt = scope.finish();
        assert!(!refused && active == 0 && !metrics.invalid);
        assert!(metrics.allocated <= receipt.charged);
        assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
        (failed, receipt)
    };
    let (failed, ample) = run(1 << 20, crate::cleanup_plan::replay::MAX_REPLAY_WORK_UNITS);
    assert!(!failed && ample.charged > 0 && ample.work > 0);
    let (failed, exact) = run(ample.charged, ample.work);
    assert!(!failed);
    assert_eq!((exact.charged, exact.work), (ample.charged, ample.work));
    for (bytes, work) in [
        (0, ample.work),
        (ample.charged - 1, ample.work),
        (ample.charged, 0),
        (ample.charged, ample.work - 1),
    ] {
        let (failed, receipt) = run(bytes, work);
        assert!(failed);
        assert!(receipt.first_refusal.is_some() || receipt.first_work_refusal.is_some());
    }
}
