//! Negative funding boundaries for scoped recovery. Authored, not executed.
//! Nothing here funds recovery: each test shows that today an active zero or
//! sticky-refused budget leaves directory backing and work unpaid. The bounded
//! recovery fixtures retain all six diagnostic fields under active budgets.
//! Budget setup is outside every observed region; observation is inside.
//! These boundaries document residual storage, not a funded production route.
use super::{codes, receive, source};
use crate::ast::{ExprKind, Span, Type};
use crate::bounded_output::{active_remaining, reserve_active_required, with_limit_usage_refusal};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::recovery::Recovery;
use crate::source_verify::type_table::TypeTable;
use crate::test_allocation_observer::measure;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

fn under<T>(limit: usize, operation: impl FnOnce() -> T) -> (T, usize, bool) {
    let (value, overflowed, used, refusal) = with_limit_usage_refusal(limit, operation);
    assert_eq!(overflowed, refusal.is_some());
    (value, used, overflowed)
}

fn exercise_directory(diagnostics: &[Diagnostic]) -> usize {
    let mut recovery = Recovery::default();
    let checkpoint = recovery.checkpoint(&[]);
    recovery.refused_let(0, "bad", checkpoint, diagnostics);
    for index in 1..65 {
        recovery.fork(Some(index - 1), index);
    }
    for _ in 0..16 {
        assert!(recovery.reads_refused(64, "bad"));
    }
    let visits = recovery.visited;
    drop(recovery);
    visits
}

fn root() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "SPX-T202",
        "missing root",
        Span::default(),
    )]
}

#[test]
fn zero_budget_neither_funds_nor_changes_directory_allocation_and_work() {
    let diagnostics = root();
    let (baseline_visits, baseline) = measure(|| exercise_directory(&diagnostics));
    assert_eq!(baseline_visits, 16 * 65);
    let ((snapshot, before, after), used, refused) = under(0, || {
        let before = active_remaining();
        let (visits, snapshot) = measure(|| exercise_directory(&diagnostics));
        assert_eq!(visits, 16 * 65);
        (snapshot, before, active_remaining())
    });
    assert_eq!(before, Some(0));
    assert_eq!(after, before, "recovery must not touch the active budget");
    assert_eq!(used, 0);
    assert!(
        !refused,
        "recovery issued no reservation, so none can be refused"
    );
    assert_eq!(
        snapshot, baseline,
        "zero budget retains actual directory layouts"
    );
    assert!(!snapshot.invalid);
    assert!(
        snapshot.allocated > 0,
        "directory storage is real and unpaid: {snapshot:?}"
    );
    assert_eq!(snapshot.live, 0);
    assert_eq!(snapshot.allocated, snapshot.freed);
}

#[test]
fn sticky_refusal_preserves_first_receipt_and_actual_recovery_storage() {
    let diagnostics = root();
    let (baseline_visits, baseline) = measure(|| exercise_directory(&diagnostics));
    assert_eq!(baseline_visits, 16 * 65);
    let (_, _, _, original) = with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(1));
    });
    let (snapshot, refused, used, receipt) = with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(1));
        let before = active_remaining();
        let (visits, snapshot) = measure(|| exercise_directory(&diagnostics));
        assert_eq!(visits, 16 * 65);
        assert_eq!(active_remaining(), before);
        assert!(!reserve_active_required(0));
        assert!(!reserve_active_required(7));
        snapshot
    });
    assert!(refused);
    assert_eq!(used, 0);
    assert_eq!(snapshot, baseline, "sticky refusal retains actual layouts");
    assert_eq!(
        receipt, original,
        "recovery and retries preserve the first refusal"
    );
    assert!(!snapshot.invalid);
    assert!(
        snapshot.allocated > 0,
        "recovery still allocates after a sticky refusal: {snapshot:?}"
    );
    assert_eq!(snapshot.live, 0);
    assert_eq!(snapshot.allocated, snapshot.freed);
}

#[test]
fn observed_layout_exact_and_one_short_are_not_a_paid_recovery_frontier() {
    let diagnostics = root();
    let (visits, observed) = measure(|| exercise_directory(&diagnostics));
    assert_eq!(visits, 16 * 65);
    assert!(!observed.invalid && observed.allocated > 0);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    // Actual layouts are a storage witness, never reservation authority. Today
    // both exact and one-short limits admit these unpaid acquisitions.
    assert!(observed.highwater > 0);
    for limit in [
        0,
        observed.allocated - 1,
        observed.allocated,
        observed.highwater - 1,
        observed.highwater,
    ] {
        let ((visits, storage), used, refused) =
            under(limit, || measure(|| exercise_directory(&diagnostics)));
        assert_eq!(visits, 16 * 65);
        assert_eq!(used, 0);
        assert!(!refused);
        assert_eq!(
            storage, observed,
            "actual layouts retained at limit={limit}"
        );
        assert!(!storage.invalid && storage.allocated > 0);
        assert_eq!(storage.live, 0);
        assert_eq!(storage.allocated, storage.freed);
    }
}

#[test]
fn zero_budget_does_not_fund_expression_identity_entries() {
    let program = crate::parse(
        &source("let bad: bool = 1; bad"),
        Path::new("funding-annotation.spx"),
    )
    .unwrap();
    let ExprKind::Block { tail, .. } = &program.functions[0].body.kind else {
        panic!("block");
    };
    let declared = Type::Bool;
    let ((viewed, snapshot), used, refused) = under(0, || {
        measure(|| {
            let mut recovery = Recovery::default();
            recovery.refused_annotation(0, "bad", &declared, 0);
            assert!(!recovery.reads_value(0, "bad", tail));
            let viewed = recovery.view(tail).is_some();
            drop(recovery);
            viewed
        })
    });
    assert!(viewed);
    assert_eq!(used, 0);
    assert!(!refused);
    assert!(snapshot.allocated > 0, "{snapshot:?}");
    assert!(!snapshot.invalid);
    assert_eq!(snapshot.live, 0);
    assert_eq!(snapshot.allocated, snapshot.freed);
}

const SIGNATURE: &str = r#"module test.funding_signature; @id("app.main") fn probe(p: Missing) -> i64 { 0 } @id("main") fn main() -> i64 { 0 }"#;

fn signature_probe() -> (crate::test_allocation_observer::Metrics, bool) {
    let program = crate::parse(SIGNATURE, Path::new("funding-signature.spx")).unwrap();
    let current = program
        .functions
        .iter()
        .find(|f| f.name == "probe")
        .unwrap();
    let parameter = &current.params[0];
    let diagnostics = crate::source_verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "SPX-T001" && d.span == Some(parameter.span)),
        "precondition: {diagnostics:?}"
    );
    let types = TypeTable::new(&program);
    let mut variables = HashMap::new();
    variables.insert(
        parameter.name.clone(),
        Binding {
            contract_local: false,
            traversal_row: None,
            ty: parameter.ty.clone(),
            mode: parameter.mode,
            availability: Availability::Available,
            moved_places: HashMap::new(),
            definitely_partial: HashSet::new(),
            native_unit_discard: false,
            mutable: false,
            active_loans: BTreeSet::new(),
            borrow_origin: None,
        },
    );
    let held = crate::prelude::declarations();
    let identity = (held.as_ptr(), held.len());
    let ((hit, snapshot), used, refused) = under(0, || {
        let before = active_remaining();
        assert_eq!(before, Some(0));
        let (hit, snapshot) = measure(|| {
            let mut recovery =
                Recovery::signature(&program, current, &variables, &types, &diagnostics);
            let hit = recovery.reads_known_refused(0, "p");
            drop(recovery);
            hit
        });
        assert_eq!(active_remaining(), before);
        (hit, snapshot)
    });
    assert_eq!(used, 0);
    assert!(!refused);
    let after = crate::prelude::declarations();
    assert_eq!(identity, (after.as_ptr(), after.len()));
    (snapshot, hit)
}

#[test]
fn signature_recheck_allocates_uncharged_under_zero_budget() {
    let (snapshot, hit) = signature_probe();
    assert!(
        hit,
        "the actual signature recheck still publishes the refused parameter"
    );
    assert!(!snapshot.invalid);
    assert!(
        snapshot.allocated > 0,
        "signature pending vector, diagnostic payload/scratch and directory are unpaid: {snapshot:?}"
    );
    assert_eq!(snapshot.live, 0);
    assert_eq!(snapshot.allocated, snapshot.freed);
}

fn fixtures() -> Vec<String> {
    vec![
        source("let bad = missing; bad + unrelated"),
        source("let bad = missing; let alias = bad; alias + bad"),
        r#"module test.funding_dependency; @id("bool.consume") fn f_bool(value: bool) -> i64 { 0 } @id("app.main") fn main() -> i64 { let bad: bool = 1; let alias = bad; let used = f_bool(alias); 0 }"#.to_owned(),
        SIGNATURE.to_owned(),
    ]
}

fn assert_six_fields(expected: &[Diagnostic], actual: &[Diagnostic], label: &str) {
    assert_eq!(expected.len(), actual.len(), "{label}");
    for (want, got) in expected.iter().zip(actual) {
        assert_eq!(want.code, got.code, "{label}");
        assert_eq!(want.severity, got.severity, "{label}");
        assert_eq!(want.message, got.message, "{label}");
        assert_eq!(want.path, got.path, "{label}");
        assert_eq!(want.span, got.span, "{label}");
        assert_eq!(want.help, got.help, "{label}");
        assert!(want == got, "severity or any field differs: {label}");
    }
}

#[test]
fn all_six_diagnostic_fields_survive_active_budgets_for_recovery_fixtures() {
    let held = crate::prelude::declarations();
    for text in fixtures() {
        let program = crate::parse(&text, Path::new("funding-fields.spx")).unwrap();
        let baseline = crate::source_verify::verify(&program);
        assert!(!baseline.is_empty(), "{text}");
        let (zero, _, _) = under(0, || crate::source_verify::verify(&program));
        assert_six_fields(&baseline, &zero, "zero budget");
        let (sticky, _, _) = under(0, || {
            assert!(!reserve_active_required(1));
            crate::source_verify::verify(&program)
        });
        assert_six_fields(&baseline, &sticky, "sticky refused budget");
        let (large, _, _) = under(usize::MAX, || crate::source_verify::verify(&program));
        assert_six_fields(&baseline, &large, "large budget");
    }
    assert!(std::ptr::eq(held, crate::prelude::declarations()));
}

#[test]
fn full_production_verification_allocates_uncharged_under_zero_budget() {
    let held = crate::prelude::declarations();
    let program = crate::parse(
        &source("let bad = missing; let alias = bad; alias + bad"),
        Path::new("funding-full.spx"),
    )
    .unwrap();
    let expected = crate::source_verify::verify(&program);
    assert_eq!(codes(&expected), ["SPX-T202"]);
    let ((snapshot, before, after), used, refused) = under(0, || {
        let before = active_remaining();
        let (_, snapshot) = measure(|| {
            let diagnostics = crate::source_verify::verify(&program);
            assert_six_fields(&expected, &diagnostics, "full receiving Drop");
            drop(diagnostics);
        });
        (snapshot, before, active_remaining())
    });
    assert_eq!(before, Some(0));
    assert_eq!(after, before);
    assert_eq!(used, 0);
    assert!(!refused);
    assert!(!snapshot.invalid);
    assert!(snapshot.allocated > 0, "{snapshot:?}");
    assert_eq!(snapshot.live, 0);
    assert_eq!(snapshot.allocated, snapshot.freed);
    assert!(std::ptr::eq(held, crate::prelude::declarations()));
}

#[test]
fn complete_old_and_new_receivers_independently_keep_the_intentional_delta_under_active_budget() {
    let held = crate::prelude::declarations();
    let source = source("let bad = missing; bad + unrelated");
    let plain_old = receive(&source, true);
    let plain_new = receive(&source, false);
    // The helper parses internally, so it runs only under a generous budget.
    let ((old, new), _, refused) = under(usize::MAX, || {
        (receive(&source, true), receive(&source, false))
    });
    assert!(!refused);
    assert_eq!(old, plain_old);
    assert_eq!(new, plain_new);
    assert_eq!(codes(&old.1), ["SPX-T202", "SPX-T202", "SPX-T202"]);
    assert_eq!(codes(&new.1), ["SPX-T202", "SPX-T202"]);
    assert_eq!(new.1, [old.1[0].clone(), old.1[2].clone()]);
    assert_eq!(
        old.1.len() - new.1.len(),
        1,
        "the dropped read is intentional"
    );
    for use_old in [true, false] {
        let ((_, snapshot), _, refused) = under(usize::MAX, || {
            measure(|| {
                drop(receive(&source, use_old));
            })
        });
        assert!(!refused);
        assert!(!snapshot.invalid, "old={use_old}: {snapshot:?}");
        assert_eq!(snapshot.live, 0, "old={use_old}: {snapshot:?}");
        assert_eq!(snapshot.allocated, snapshot.freed, "old={use_old}");
    }
    assert!(std::ptr::eq(held, crate::prelude::declarations()));
}
