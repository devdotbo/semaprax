//! Complete ordinary predicates remain independent of the paid twins.
use super::*;
use crate::cleanup::ReplayContext as _;
use crate::cleanup_plan::replay::allocation::{enter, OracleRefusal, OracleScope};

#[test]
fn complete_byte_text_collection_and_subtree_receivers_match_with_finite_funding() {
    let program = crate::hir::resolve(
        &crate::check(
            r#"
module admission.account;
@id("bytes") record BytesRow { @id("bytes.field") field: Bytes, }
@id("text") record TextRow { @id("text.field") field: string, }
@id("collection") record CollectionRow { @id("collection.field") field: Vec<i64>, }
@id("box") record Box<T> { @id("box.field") field: T, }
@id("main") fn main()->i64 {7}
"#,
            "admission-account.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let nominal = |id: &str, arguments| ResolvedType::Nominal {
        declaration: program
            .types()
            .iter()
            .find(|t| t.id.as_str() == id)
            .unwrap()
            .id
            .clone(),
        arguments,
    };
    let cases = [
        nominal("bytes", vec![]),
        nominal("text", vec![]),
        nominal("collection", vec![]),
        nominal("box", vec![ResolvedType::Bytes]),
        nominal("box", vec![]),
        nominal("bytes", vec![ResolvedType::Bytes]),
        nominal(
            "box",
            vec![ResolvedType::Function {
                parameters: vec![ResolvedType::Bytes],
                result: Box::new(ResolvedType::I64),
            }],
        ),
        ResolvedType::Nominal {
            declaration: DeclarationId::new("missing"),
            arguments: vec![],
        },
        ResolvedType::Bytes,
        ResolvedType::String,
        ResolvedType::I64,
    ];
    let index = program.declarations();
    // Full original predicates run before any account or production ledger.
    let originals: Vec<_> = cases
        .iter()
        .map(|ty| {
            (
                crate::hir::is_admitted_nested_owned_byte_record(index, ty),
                crate::hir::owned_text_record::admitted(ty, index),
                crate::hir::owned_collection_record::admitted(ty, index),
                crate::hir::owned_record_subtree::admitted(index, ty, ty),
                crate::hir::owned_collection_record::vector(index, ty),
            )
        })
        .collect();
    assert!(originals[0].0 && originals[1].1 && originals[2].2 && originals[3].0);
    let run = |bytes, work| {
        let scope = OracleScope::open_with_work(bytes, work).unwrap();
        let lease = scope.lease();
        let ((same, metrics), refused, used, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                let (same, metrics) = crate::test_allocation_observer::measure(|| {
                    let invocation = enter(&program, function, lease).unwrap();
                    let mut budget =
                        ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                    let mut same = true;
                    let result = (|| {
                        budget.prepare_builder(function)?;
                        budget.prepare_clone_cursor(function)?;
                        for (ty, expected) in cases.iter().zip(&originals) {
                            let current = (
                                byte(index, ty, function, &mut budget)?,
                                text::text(ty, index, function, &mut budget)?,
                                collection::collection(ty, index, function, &mut budget)?,
                                subtree(index, ty, ty, function, &mut budget)?,
                                vector(index, ty),
                            );
                            same &= &current == expected;
                        }
                        Ok::<(), Diagnostic>(())
                    })();
                    same &= result.is_ok();
                    drop(result);
                    drop(budget);
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
    assert!(allocated > 0 && baseline.work > 0);
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
        (0, baseline.work),
        (baseline.charged, 0),
    ] {
        let (same, retry, receipt, _) = run(bytes, work);
        assert!(!same);
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(receipt.charged <= bytes && receipt.work <= work);
        assert!(receipt.first_refusal.is_some() || receipt.first_work_refusal.is_some());
        assert!(receipt.physical_failure.is_none());
    }
}
