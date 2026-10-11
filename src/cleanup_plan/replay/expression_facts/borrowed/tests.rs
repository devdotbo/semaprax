//! Independent ordinary fact-map verdict and whole-receiver resource witness.
use super::*;
#[test]
fn ordinary_fact_oracle_and_two_receivers_share_exact_work_and_backing_through_drop() {
    let source = r#"module facts.borrowed;
@id("read") fn read(view:borrow Slice<u8>,at:usize)->i64 {
 let value=match byte_get(view,at) { Option::Some {value}=>i64_from_u8(value),Option::None {}=>0, };
 value+1
}
@id("main") fn main()->i64 {0}
"#;
    let program =
        crate::hir::resolve(&crate::check(source, "borrowed-facts.spx").unwrap()).unwrap();
    let function = &program.functions[0];
    // Literal byte-prefix census is independent of the receiver callback.
    for (left, right, units) in [
        ("", "", 1),
        ("a", "b", 1),
        ("ab", "ac", 2),
        ("ab", "abz", 3),
    ] {
        let left = ExpressionId::from_owned(left.into());
        let right = ExpressionId::from_owned(right.into());
        for allowance in [units, units - 1] {
            let mut budget = ReplayBudget::new();
            budget.remaining = allowance;
            let actual = compare(function, &left, &right, &mut budget);
            if allowance == units {
                assert_eq!(actual.unwrap(), left.cmp(&right));
            } else {
                assert_eq!(actual.unwrap_err().code, "SPX-H006");
            }
            assert_eq!(budget.remaining, 0);
        }
    }
    let expected = expression_facts(function).unwrap();
    let (empty, refused) =
        crate::bounded_output::with_limit(0, || ExpressionId::from_owned(String::new()));
    assert!(refused);
    assert_eq!(empty.as_str(), "");
    let absent = [empty, ExpressionId::from_owned("hostile.nearby".into())];
    fn run(
        function: &ResolvedFunction,
        expected: &BTreeMap<ExpressionId, Option<CallFact>>,
        absent: &[ExpressionId; 2],
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), ()>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let mut budget = ReplayBudget::with_physical(function, bytes).map_err(|_| ())?;
            budget.remaining = work;
            // Two independent constructions and their complete receiving probes
            // consume the same budget and backing owner, with no restart/refund.
            for _ in 0..2 {
                let facts = BorrowedFacts::build(function, &mut budget).map_err(|_| ())?;
                assert_eq!(facts.entries.len(), expected.len());
                for (key, expected) in expected {
                    let actual = facts
                        .fact(function, key, &mut budget)
                        .map_err(|_| ())?
                        .unwrap();
                    match (expected, actual) {
                        (None, None) => {}
                        (Some(expected), Some(actual)) => {
                            assert_eq!(expected.callee, actual.callee);
                            assert_eq!(expected.instance, actual.instance);
                            assert_eq!(expected.arguments, actual.arguments);
                            assert_eq!(expected.type_arguments, actual.type_arguments);
                        }
                        _ => panic!("independent fact kind disagrees"),
                    }
                }
                for absent in absent {
                    assert!(!expected.contains_key(absent));
                    assert!(facts
                        .fact(function, absent, &mut budget)
                        .map_err(|_| ())?
                        .is_none());
                }
                drop(facts);
            }
            let used = (
                work - budget.remaining,
                budget.physical.as_ref().unwrap().charged(),
            );
            drop(budget);
            Ok(used)
        })
    }
    let (wide, allocation) = run(
        function,
        &expected,
        &absent,
        MAX_REPLAY_WORK_UNITS,
        16 << 20,
    );
    let (work, bytes) = wide.unwrap();
    assert!(work <= MAX_REPLAY_WORK_UNITS);
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
    for (limit_work, limit_bytes, success) in [
        (work, bytes, true),
        (work - 1, bytes, false),
        (work, bytes - 1, false),
    ] {
        let (result, allocation) = run(function, &expected, &absent, limit_work, limit_bytes);
        assert_eq!(result.is_ok(), success);
        assert!(!allocation.invalid && allocation.live == 0);
        if success {
            assert_eq!(result.unwrap(), (work, bytes));
            assert!(allocation.allocated <= bytes);
        }
    }
    // The original duplicate-identity error remains independent of the table's
    // storage algorithm and is selected before any candidate fact is published.
    let mut forged = function.clone();
    let root = forged.body.id.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut forged.body.kind else {
        panic!("source block");
    };
    tail.id = root;
    let original = expression_facts(&forged).err().unwrap();
    let mut budget = ReplayBudget::with_physical(&forged, 16 << 20).unwrap();
    let guarded = BorrowedFacts::build(&forged, &mut budget).err().unwrap();
    assert_eq!(guarded.code, original.code);
    assert_eq!(guarded.message, original.message);
}
