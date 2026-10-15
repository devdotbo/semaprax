//! Actual receiver owner affinity before construction and after cached success.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::hir::prelude_bundle::ProducerFailure;
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"module nominal.affinity;
@id("main") fn main()->i64 {0}
"#;
const CAP: usize = 64 * 1024 * 1024;

fn assert_detached_report(failure: ModuleFailure) {
    let ModuleFailure::Report(report) = failure else {
        panic!("the actual receiving report owner was funded")
    };
    let pointers = ACQUIRED.with(|cell| cell.get().1.unwrap());
    assert_eq!(report.as_ptr() as usize, pointers.0);
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].message.as_ptr() as usize, pointers.1);
    assert_eq!(report[0].code, "SPX-H006");
    assert_eq!(report[0].severity, crate::diagnostic::Severity::Error);
    assert_eq!(
        report[0].message,
        "base prelude producer acquisition refused"
    );
    assert_eq!(report[0].path, None);
    assert_eq!(report[0].span, None);
    assert_eq!(report[0].help, None);
}

fn owner_affinity(cached: bool) {
    let program = crate::check(SOURCE, "nominal-affinity.spx").unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(CAP, || {
        let authored = BTreeMap::new();
        let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
            &authored,
            std::slice::from_ref(&program),
        )
        .unwrap();
        let input = Input::Owned(program.clone());
        let resolved = if cached {
            Some(resolve_module_with_core(&mut core, None, &program.path, &input, None).unwrap())
        } else {
            None
        };
        let receipt = core.base_prelude.receipt();
        assert_eq!(receipt.is_some(), cached);
        if let Some(receipt) = receipt {
            assert_eq!(receipt.used, receipt.limit);
            assert_eq!(receipt.remaining, 0);
            assert_eq!(receipt.first, None);
        }
        // A different active Rc<Budget> cannot acquire or reuse this core's
        // payload, even though its own outward report allocation succeeds.
        let ((), child_refused, _, child_first) = with_limit_usage_refusal(CAP, || {
            assert_detached_report(
                resolve_module_with_core(&mut core, None, &program.path, &input, None).unwrap_err(),
            );
            assert_eq!(core.base_prelude.receipt(), receipt);
        });
        assert!(!child_refused);
        assert_eq!(child_first, None);
        // Restoring the original ledger cannot reset the selected detachment
        // or complete an unbuilt bundle, nor reuse a previously built bundle.
        assert_detached_report(
            resolve_module_with_core(&mut core, None, &program.path, &input, None).unwrap_err(),
        );
        assert_eq!(core.base_prelude.receipt(), receipt);
        let (selected, retry) = measure(|| {
            core.base_prelude
                .ensure(&mut |_| -> Result<(), Refused> {
                    panic!("detachment must not retry byte acquisition")
                })
                .err()
                .unwrap()
        });
        assert_eq!(
            selected,
            InvocationFailure::Producer(ProducerFailure::Detached)
        );
        assert_eq!(retry.allocated, 0);
        assert!(!retry.invalid);
        let ((), disposal) = measure(|| {
            drop(core);
            drop(resolved);
        });
        assert_eq!(disposal.allocated, 0);
        assert!(!disposal.invalid);
    });
    assert!(
        !refused,
        "owner detachment does not fabricate a byte refusal"
    );
    assert_eq!(first, None);
}

#[test]
fn actual_base_foreign_then_restored_owner_cannot_construct_or_reset_first_detachment() {
    owner_affinity(false);
}

#[test]
fn actual_base_foreign_then_restored_owner_cannot_reuse_cached_payload_or_reset_detachment() {
    owner_affinity(true);
}
