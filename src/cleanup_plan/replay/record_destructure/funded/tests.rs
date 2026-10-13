//! Independent complete ordinary receivers, finite accounts and receiving Drop.
use super::*;
use crate::cleanup::ReplayContext as _;
use crate::cleanup_plan::replay::allocation::{enter, OracleRefusal, OracleScope};

fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"
module destructure.account;
@id("leaf") record Leaf { @id("leaf.bytes") bytes: Bytes, @id("leaf.scalar") scalar: i64, }
@id("root") record Root { @id("root.leaf") leaf: Leaf, }
@id("main") fn main()->i64 {7}
"#,
            "destructure-account.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn nested_destructure_and_field_substitution_match_full_old_receivers_through_drop() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let root = program
        .types()
        .iter()
        .find(|t| t.id.as_str() == "root")
        .unwrap()
        .id
        .clone();
    let leaf = program
        .types()
        .iter()
        .find(|t| t.id.as_str() == "leaf")
        .unwrap()
        .id
        .clone();
    let instance = ResolvedType::Nominal {
        declaration: root.clone(),
        arguments: vec![],
    };
    let leaf_instance = ResolvedType::Nominal {
        declaration: leaf.clone(),
        arguments: vec![],
    };
    let leaf_fields = program
        .declarations()
        .record_fields(&leaf)
        .unwrap()
        .iter()
        .map(|f| ResolvedRecordMatchPatternField {
            field: f.id.clone(),
            pattern: ResolvedRecordMatchFieldPattern::Wildcard,
        })
        .collect();
    let fields = vec![ResolvedRecordMatchPatternField {
        field: program.declarations().record_fields(&root).unwrap()[0]
            .id
            .clone(),
        pattern: ResolvedRecordMatchFieldPattern::Record {
            record: leaf,
            instance: leaf_instance,
            fields: leaf_fields,
        },
    }];
    let original = super::super::replay(
        &program,
        function,
        &root,
        &instance,
        &fields,
        ResolvedMatchMode::Borrow,
    )
    .unwrap();
    let original_error = super::super::replay(
        &program,
        function,
        &root,
        &instance,
        &fields,
        ResolvedMatchMode::Own,
    )
    .err()
    .expect("owned wildcard refuses");
    let template = ResolvedType::Function {
        parameters: vec![ResolvedType::TypeParameter {
            owner: root.clone(),
            index: 0,
        }],
        result: Box::new(ResolvedType::TypeParameter {
            owner: root.clone(),
            index: 0,
        }),
    };
    let arguments = [ResolvedType::Function {
        parameters: vec![ResolvedType::I64],
        result: Box::new(ResolvedType::ArrayU8(16)),
    }];
    let substituted = crate::hir::substitute_type(&template, &root, &arguments).unwrap();
    let run = |allowance, work| {
        let scope = if work == 0 {
            OracleScope::open(allowance).unwrap()
        } else {
            OracleScope::open_with_work(allowance, work).unwrap()
        };
        let lease = scope.lease();
        let ((same, retry, metrics), refused, used, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                let ((same, retry), metrics) = crate::test_allocation_observer::measure(|| {
                    let invocation = enter(&program, function, lease).unwrap();
                    let current = replay(
                        &program,
                        function,
                        &root,
                        &instance,
                        &fields,
                        ResolvedMatchMode::Borrow,
                    )
                    .unwrap();
                    let same = matches!(&current, Ok(value) if value.nested == original.nested && value.bindings == original.bindings);
                    drop(current);
                    let error = replay(
                        &program,
                        function,
                        &root,
                        &instance,
                        &fields,
                        ResolvedMatchMode::Own,
                    )
                    .unwrap();
                    let same = same && matches!(&error, Err(value) if value == &original_error);
                    drop(error);
                    let current = super::super::super::nested_shape::substitute_type_current(
                        &program, function, &template, &root, &arguments,
                    );
                    let same = same && matches!(&current, Ok(value) if value == &substituted);
                    drop(current);
                    let retry = lease.charge(0);
                    assert_eq!(invocation.exit(), Ok(()));
                    (same, retry)
                });
                (same, retry, metrics)
            });
        assert!(!refused);
        assert_eq!((used, first), (0, None));
        let receipt = scope.finish();
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert!(
            metrics.allocated <= receipt.charged,
            "{metrics:?} {receipt:?}"
        );
        (same, retry, receipt, metrics.allocated)
    };
    let (same, retry, baseline, allocated) = run(1 << 20, 1 << 25);
    assert!(
        same,
        "full nested receiver and all six original error fields"
    );
    assert_eq!(retry, Ok(()));
    assert!(allocated > 0);
    assert!(baseline.first_refusal.is_none());
    let (same, retry, exact, _) = run(baseline.charged, baseline.work);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert_eq!(
        (exact.charged, exact.work),
        (baseline.charged, baseline.work)
    );
    assert!(baseline.work > 0);
    for work in [baseline.work - 1, 0] {
        let (same, retry, receipt, _) = run(baseline.charged, work);
        assert!(!same, "work refusal cannot publish a semantic comparison");
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(receipt.first_work_refusal.is_some());
        assert_eq!(receipt.work_allowance, work);
        assert!(receipt.work <= work);
    }
    for allowance in [baseline.charged - 1, 0] {
        let (same, retry, receipt, _) = run(allowance, baseline.work);
        assert!(!same);
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(receipt.first_refusal.is_some());
    }
}

#[test]
fn unfunded_nominal_admission_refuses_before_its_original_owning_predicate() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let root = program
        .types()
        .iter()
        .find(|t| t.id.as_str() == "root")
        .unwrap()
        .id
        .clone();
    let leaf = program
        .types()
        .iter()
        .find(|t| t.id.as_str() == "leaf")
        .unwrap()
        .id
        .clone();
    let instance = ResolvedType::Nominal {
        declaration: root.clone(),
        arguments: vec![],
    };
    let fields = [ResolvedRecordMatchPatternField {
        field: program.declarations().record_fields(&root).unwrap()[0]
            .id
            .clone(),
        pattern: ResolvedRecordMatchFieldPattern::Binding(crate::hir::ResolvedBinding {
            id: function.result_id.clone(),
            name: "held".to_owned(),
            ownership: OwnershipMode::Own,
            ty: ResolvedType::Nominal {
                declaration: leaf,
                arguments: vec![],
            },
            span: function.body.span,
        }),
    }];
    let original = super::super::replay(
        &program,
        function,
        &root,
        &instance,
        &fields,
        ResolvedMatchMode::Own,
    )
    .unwrap();
    assert!(original.nested);
    assert_eq!(original.bindings.len(), 1);
    let scope = OracleScope::open_with_work(1 << 20, 1 << 25).unwrap();
    let lease = scope.lease();
    let ((metrics, retry), refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            let ((), metrics) = crate::test_allocation_observer::measure(|| {
                let invocation = enter(&program, function, lease).unwrap();
                let result = replay_with_admission(
                    &program,
                    function,
                    &root,
                    &instance,
                    &fields,
                    (ResolvedMatchMode::Own, false),
                )
                .unwrap();
                assert!(
                    result.is_err(),
                    "an unfunded original predicate cannot publish a verdict"
                );
                drop(result);
                assert_eq!(invocation.exit(), Ok(()));
            });
            (metrics, lease.charge(0))
        });
    assert!(!refused);
    assert_eq!((used, first), (0, None));
    let receipt = scope.finish();
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert!(metrics.allocated <= receipt.charged);
    assert_eq!(retry, Err(OracleRefusal::Sticky));
    assert_eq!(
        receipt.physical_failure,
        Some("destructure nominal admission acquisition is not funded")
    );
}

#[test]
fn funded_nominal_admission_matches_complete_original_through_drop() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let root = program
        .types()
        .iter()
        .find(|t| t.id.as_str() == "root")
        .unwrap()
        .id
        .clone();
    let leaf = program
        .types()
        .iter()
        .find(|t| t.id.as_str() == "leaf")
        .unwrap()
        .id
        .clone();
    let instance = ResolvedType::Nominal {
        declaration: root.clone(),
        arguments: vec![],
    };
    let fields = [ResolvedRecordMatchPatternField {
        field: program.declarations().record_fields(&root).unwrap()[0]
            .id
            .clone(),
        pattern: ResolvedRecordMatchFieldPattern::Binding(crate::hir::ResolvedBinding {
            id: function.result_id.clone(),
            name: "held".to_owned(),
            ownership: OwnershipMode::Own,
            ty: ResolvedType::Nominal {
                declaration: leaf,
                arguments: vec![],
            },
            span: function.body.span,
        }),
    }];
    let original = super::super::replay(
        &program,
        function,
        &root,
        &instance,
        &fields,
        ResolvedMatchMode::Own,
    )
    .unwrap();
    assert!(original.nested);
    assert_eq!(original.bindings.len(), 1);
    let run = |bytes, work| {
        let scope = OracleScope::open_with_work(bytes, work).unwrap();
        let lease = scope.lease();
        let ((same, metrics), refused, used, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                let (same, metrics) = crate::test_allocation_observer::measure(|| {
                    let invocation = enter(&program, function, lease).unwrap();
                    let result = replay(
                        &program,
                        function,
                        &root,
                        &instance,
                        &fields,
                        ResolvedMatchMode::Own,
                    )
                    .unwrap();
                    let same = matches!(&result, Ok(value) if value.nested == original.nested && value.bindings == original.bindings);
                    drop(result);
                    assert_eq!(invocation.exit(), Ok(()));
                    same
                });
                (same, metrics)
            });
        assert!(!refused);
        assert_eq!((used, first), (0, None));
        let retry = lease.charge(0);
        let receipt = scope.finish();
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert!(metrics.allocated <= receipt.charged);
        (same, retry, receipt, metrics.allocated)
    };
    let (same, retry, baseline, allocated) = run(1 << 20, 1 << 25);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert!(allocated > 0 && baseline.work > 0 && baseline.charged > 0);
    assert!(baseline.physical_failure.is_none());
    let (same, retry, exact, _) = run(baseline.charged, baseline.work);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert_eq!(
        (exact.charged, exact.work),
        (baseline.charged, baseline.work)
    );
    for (bytes, work) in [
        (baseline.charged - 1, baseline.work),
        (baseline.charged, baseline.work - 1),
        (baseline.charged, 0),
        (0, baseline.work),
    ] {
        let (same, retry, receipt, _) = run(bytes, work);
        assert!(!same, "refusal is never an admission verdict");
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(receipt.first_refusal.is_some() || receipt.first_work_refusal.is_some());
        assert!(receipt.charged <= bytes && receipt.work <= work);
        assert!(
            receipt.physical_failure.is_none(),
            "the original byte/work failure remains selected"
        );
    }
}

#[test]
fn hard_physical_refusal_keeps_full_original_diagnostic_and_zero_sticky() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    // The owning diagnostic is obtained independently before account authority.
    // A dynamically held reason must never be stored as an account reference.
    let reason = String::from("cleanup replay physical vector allocation failed");
    let original = crate::cleanup_plan::replay::replay_error_old(function, reason.clone());
    let scope = OracleScope::open_with_work(1 << 20, 1 << 25).unwrap();
    let lease = scope.lease();
    let ((same, metrics), refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            let (same, metrics) = crate::test_allocation_observer::measure(|| {
                let invocation = enter(&program, function, lease).unwrap();
                let mut budget =
                    ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                budget
                    .reserve_bytes(function, 0, "zero before setup")
                    .unwrap();
                budget.prepare_builder(function).unwrap();
                let current = budget.budget_refusal(function, &reason, "");
                let same = current == original;
                drop(current);
                let retry = budget.reserve_bytes(function, 0, "zero after physical refusal");
                assert!(retry.is_err());
                drop(retry);
                drop(budget);
                assert_eq!(invocation.exit(), Ok(()));
                same
            });
            (same, metrics)
        });
    assert!(same);
    assert!(!refused);
    assert_eq!((used, first), (0, None));
    assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
    let receipt = scope.finish();
    assert_eq!(
        receipt.physical_failure,
        Some("oracle replay budget refused physical construction")
    );
    assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none());
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert!(metrics.allocated <= receipt.charged);
}

#[test]
fn authentic_zero_physical_request_acquires_no_backing_or_charge() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let scope = OracleScope::open(0).unwrap();
    let lease = scope.lease();
    let ((), metrics) = crate::test_allocation_observer::measure(|| {
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        budget
            .reserve_bytes(function, 0, "zero physical probe")
            .unwrap();
        drop(budget);
        assert_eq!(invocation.exit(), Ok(()));
    });
    let receipt = scope.finish();
    assert!(!metrics.invalid);
    assert_eq!((metrics.allocated, metrics.live), (0, 0));
    assert_eq!((receipt.charged, receipt.work), (0, 0));
    assert!(receipt.calls > 0);
    assert!(
        receipt.first_refusal.is_none()
            && receipt.first_work_refusal.is_none()
            && receipt.physical_failure.is_none()
    );
}
