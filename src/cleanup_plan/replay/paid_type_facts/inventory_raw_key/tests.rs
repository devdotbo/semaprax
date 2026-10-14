use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
fn run(
    bytes: usize,
    work: usize,
    function: &ResolvedFunction,
    root: &ResolvedType,
    expected: &str,
) -> (
    (bool, bool, usize),
    crate::test_allocation_observer::Metrics,
    bool,
    usize,
) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "full structural key setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        identity(root, &mut super::super::tests::payer(function, &mut budget))
                    });
                let matched = result.as_ref().is_ok_and(|key| key == expected);
                let failed = result.is_err();
                let used = work.saturating_sub(budget.remaining);
                if failed {
                    let before = current_metrics();
                    let retry =
                        identity(root, &mut super::super::tests::payer(function, &mut budget));
                    assert!(retry.is_err());
                    assert_eq!(before.allocated, current_metrics().allocated);
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed, used)
            })
        });
    (outcome, observed, refused, paid)
}
#[test]
fn full_raw_structural_identity_keeps_nested_arguments_utf8_null_and_no_capped_membership_collision(
) {
    let program = super::super::tests::fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let mut roots = Vec::new();
    for width in [0, 1, 17, 65] {
        roots.push(ResolvedType::Nominal {
            declaration: DeclarationId::new("é\0prefix:0:"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::ArrayU8(usize::MAX), ResolvedType::I64],
                    result: Box::new(ResolvedType::TypeParameter {
                        owner: function.id.clone(),
                        index: 17
                    })
                };
                width
            ],
        });
    }
    roots.push(ResolvedType::Nominal {
        declaration: DeclarationId::new("é".repeat(40_000)),
        arguments: vec![ResolvedType::Bytes],
    });
    for root in roots {
        let expected = root.identity_key(); // complete original independently compiled receiver
        let (ample, observed, refused, paid) = run(
            usize::MAX,
            MAX_REPLAY_WORK_UNITS,
            function,
            &root,
            &expected,
        );
        assert!(ample.0 && !ample.1 && !refused && !observed.invalid && observed.allocated <= paid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        let (exact, observed, refused, used) = run(paid, ample.2, function, &root, &expected);
        assert!(exact.0 && !exact.1 && !refused && !observed.invalid && used == paid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        for (bytes, work) in [
            (0, MAX_REPLAY_WORK_UNITS),
            (paid - 1, MAX_REPLAY_WORK_UNITS),
            (paid, 0),
            (paid, ample.2 - 1),
        ] {
            let (short, observed, _, used) = run(bytes, work, function, &root, &expected);
            assert!(short.1 && !observed.invalid && observed.allocated <= used);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
