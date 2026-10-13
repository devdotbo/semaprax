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
    let run = |allowance| {
        let scope = OracleScope::open(allowance).unwrap();
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
    let (same, retry, baseline, allocated) = run(1 << 20);
    assert!(
        same,
        "full nested receiver and all six original error fields"
    );
    assert_eq!(retry, Ok(()));
    assert!(allocated > 0);
    assert!(baseline.first_refusal.is_none());
    let (same, retry, exact, _) = run(baseline.charged);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert_eq!(exact.charged, baseline.charged);
    for allowance in [baseline.charged - 1, 0] {
        let (same, retry, receipt, _) = run(allowance);
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
    let scope = OracleScope::open(1 << 20).unwrap();
    let lease = scope.lease();
    let ((metrics, retry), refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            let ((), metrics) = crate::test_allocation_observer::measure(|| {
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
