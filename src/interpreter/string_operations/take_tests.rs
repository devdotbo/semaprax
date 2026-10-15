//! Exact two charge boundaries and actual owner lifetime; no interpreter-wide claim.
use super::*;
#[test]
fn named_take_operation_and_operand_fuel_boundaries_preserve_owner_and_drop_without_copy() {
    let ast=crate::check("module fuel; @id(\"take\") fn take(text:string)->string {string_take(text)} @id(\"main\") fn main()->i64 {0}","take-fuel.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "take")
        .unwrap();
    let mut call = &function.body;
    while let ResolvedExprKind::Block { tail, .. } = &call.kind {
        call = tail;
    }
    let ResolvedExprKind::Call { args, .. } = &call.kind else {
        unreachable!()
    };
    let root = match &args[0].kind {
        ResolvedExprKind::Place(p) => &p.root,
        _ => unreachable!(),
    };
    let admitted = BTreeMap::new();
    for text in ["", "é\0"] {
        // Independent original-owner construction/Drop baseline includes actual
        // Environment Vec/map backing. Checked source, evaluator's fixed Arc
        // context and observer's fixed metadata are explicitly outside scope.
        let (_, baseline) = crate::test_allocation_observer::measure(|| {
            let environment =
                Environment::from(vec![(root.clone(), Value::String(text.to_owned()))]);
            drop(environment);
        });
        assert!(!baseline.invalid);
        assert_eq!(baseline.live, 0);
        assert_eq!(baseline.allocated, baseline.freed);
        for budget in [0, 1, 2] {
            let mut evaluator = Evaluator::new_prepared(
                FunctionLookup::Borrowed(&admitted),
                BTreeMap::new(),
                &program.declarations,
                budget,
                0,
                PreparedCancellation::Never,
            );
            let (_, observed) = crate::test_allocation_observer::measure(|| {
                let owner = text.to_owned();
                let pointer = owner.as_ptr();
                let mut environment = Environment::from(vec![(root.clone(), Value::String(owner))]);
                let outcome = evaluator.evaluate_string_op(
                    crate::string_ops::StringOp::Take,
                    args,
                    &mut environment,
                    0,
                );
                if budget < 2 {
                    assert!(matches!(outcome, Err(Flow::Exhausted)));
                    let Some(Value::String(owner)) = environment.get(root) else {
                        panic!("fuel moved original owner")
                    };
                    assert_eq!(owner.as_ptr(), pointer);
                    assert_eq!(owner, text);
                } else {
                    let Ok(Value::String(owner)) = outcome else {
                        panic!("exact two charges refuse")
                    };
                    assert_eq!(owner.as_ptr(), pointer);
                    assert_eq!(owner, text);
                    assert!(matches!(environment.get(root), Some(Value::Moved)));
                    drop(owner);
                }
                drop(environment);
            });
            assert_eq!(evaluator.steps, budget);
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
            assert_eq!(observed.allocations, baseline.allocations);
            assert_eq!(observed.reallocations, baseline.reallocations);
            assert_eq!(observed.allocated, baseline.allocated);
        }
    }
}
