//! Saved original predicate oracle and literal short-circuit debit controls.
use super::*;
#[test]
fn original_branch_and_stage_predicates_preserve_verdict_order_and_exact_work() {
    let program = crate::hir::resolve(&crate::check(
        r#"module branches.borrowed; @id("choose") fn choose(flag:bool)->i64 {if flag {1}else{2}} @id("main") fn main()->i64 {0}"#,
        "borrowed-branch-receivers.spx",
    ).unwrap()).unwrap();
    let function = &program.functions[0];
    let a = ExpressionId::from_owned("a".into());
    let b = ExpressionId::from_owned("b".into());
    let case = DeclarationId::new("case");
    let status = StatusSourceId {
        expression: a.clone(),
        lane: StatusLane::OperationFailure,
    };
    let conditions = [
        EdgeCondition::Always,
        EdgeCondition::BooleanResult(a.clone(), true),
        EdgeCondition::BooleanResult(a.clone(), false),
        EdgeCondition::BooleanResult(b.clone(), false),
        EdgeCondition::VariantCase {
            scrutinee: a.clone(),
            case: case.clone(),
            matches: true,
        },
        EdgeCondition::VariantCase {
            scrutinee: a.clone(),
            case: case.clone(),
            matches: false,
        },
        EdgeCondition::ArmSelected {
            scrutinee: a.clone(),
            arm: 0,
            selected: true,
        },
        EdgeCondition::ArmSelected {
            scrutinee: a.clone(),
            arm: 0,
            selected: false,
        },
        EdgeCondition::ArmSelected {
            scrutinee: a.clone(),
            arm: 1,
            selected: false,
        },
        EdgeCondition::StatusZero(status.clone()),
        EdgeCondition::StatusNonzero(status.clone()),
    ];
    for left in &conditions {
        for right in &conditions {
            let original =
                validate_branch_pair(function, left, right).map_err(|e| (e.code, e.message));
            let mut budget = ReplayBudget::new();
            let current =
                validate(function, left, right, &mut budget).map_err(|e| (e.code, e.message));
            assert_eq!(current, original);
        }
    }
    // Shape probe plus length/header and actually visited bytes. Boolean
    // polarity is evaluated only after expression equality succeeds.
    for (left, right, units, success) in [
        (
            EdgeCondition::BooleanResult(a.clone(), true),
            EdgeCondition::BooleanResult(b.clone(), false),
            3,
            false,
        ),
        (
            EdgeCondition::BooleanResult(a.clone(), true),
            EdgeCondition::BooleanResult(a.clone(), false),
            4,
            true,
        ),
        (
            EdgeCondition::StatusZero(status.clone()),
            EdgeCondition::StatusNonzero(status),
            4,
            true,
        ),
    ] {
        for allowance in [units, units - 1] {
            let mut budget = ReplayBudget::new();
            budget.remaining = allowance;
            let current = validate(function, &left, &right, &mut budget);
            if allowance == units {
                assert_eq!(current.is_ok(), success);
            } else {
                assert!(current
                    .unwrap_err()
                    .message
                    .contains("work budget exhausted"));
            }
            assert_eq!(budget.remaining, 0);
        }
    }
    let stages = [
        StagedCopyResultSource::Body {
            expression: function.body.id.clone(),
            instance: function.return_type.clone(),
        },
        StagedCopyResultSource::Body {
            expression: b,
            instance: ResolvedType::Bool,
        },
        StagedCopyResultSource::Body {
            expression: function.body.id.clone(),
            instance: ResolvedType::Bool,
        },
    ];
    for source in stages {
        let original = validate_staged_target(function, &source).map_err(|e| (e.code, e.message));
        let mut budget = ReplayBudget::new();
        let current = staged(function, &source, &mut budget).map_err(|e| (e.code, e.message));
        assert_eq!(current, original);
    }
}
