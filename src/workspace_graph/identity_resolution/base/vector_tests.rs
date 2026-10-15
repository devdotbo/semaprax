//! Actual live core module route, including returned HIR beyond core lifetime.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const SOURCE: &str = r#"module vector.receiving;
@id("app.read") fn read(values:borrow Vec<i64>)->i64 {0}
@id("app.main") fn main()->i64 {0}
"#;
const CAP: usize = 64 * 1024 * 1024;
fn parameters(
    program: &crate::hir::ResolvedProgram,
) -> &crate::hir::NominalPayload<crate::hir::ResolvedTypeParameterDeclaration> {
    &program
        .types
        .iter()
        .find(|ty| ty.id.as_str() == crate::prelude::VEC_ID)
        .unwrap()
        .type_parameters
}
#[test]
fn actual_core_vec_receiver_keeps_full_old_fresh_borrowed_selective_hir_wire_and_last_drop() {
    let program = crate::check(SOURCE, "vector-receiving.spx").unwrap();
    assert_eq!(crate::prelude::declarations_for_program(&program).len(), 3);
    let (((), disposal), refused, _, first) = with_limit_usage_refusal(CAP, || {
        let authored = BTreeMap::new();
        let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
            &authored,
            std::slice::from_ref(&program),
        )
        .unwrap();
        let old = crate::hir::identity_resolution::nominal_original::resolve(&program).unwrap();
        let owned = Input::Owned(program.clone());
        let actual =
            resolve_module_with_core(&mut core, None, &program.path, &owned, None).unwrap();
        assert_eq!(actual.0, old.0);
        assert_eq!(
            crate::cache_codec::encode(&actual.0).unwrap(),
            crate::cache_codec::encode(&old.0).unwrap()
        );
        let base_receipt = core.base_prelude.receipt().unwrap();
        let extension_receipt = core.base_prelude.extension_receipt().unwrap();
        assert_eq!(base_receipt.remaining, 0);
        assert_eq!(extension_receipt.remaining, 0);
        assert_eq!(extension_receipt.used, extension_receipt.limit);
        let view = crate::workspace_graph::expected_projection::borrowed_leaf::construct(&program)
            .unwrap();
        let borrowed = Input::Borrowed(view);
        let other =
            resolve_module_with_core(&mut core, None, &program.path, &borrowed, None).unwrap();
        assert_eq!(other.0, old.0);
        assert!(parameters(&actual.0).same_owner(parameters(&other.0)));
        let cache =
            FrontendPass::paid_receiver_candidate(program.clone(), old.0.clone(), old.1.costs());
        let selective =
            resolve_module_with_core(&mut core, Some(&cache), &program.path, &owned, None).unwrap();
        assert_eq!(selective.0, old.0);
        assert!(parameters(&actual.0).same_owner(parameters(&selective.0)));
        assert_eq!(core.base_prelude.receipt(), Some(base_receipt));
        assert_eq!(
            core.base_prelude.extension_receipt(),
            Some(extension_receipt)
        );
        drop(core);
        assert!(parameters(&actual.0).same_owner(parameters(&other.0)));
        let ((), metrics) = measure(|| {
            drop(actual);
            drop(other);
            drop(selective);
        });
        ((), metrics)
    });
    assert!(!refused && first.is_none());
    assert!(!disposal.invalid);
    assert_eq!(disposal.allocated, 0);
}

#[test]
fn actual_core_extension_work_refusal_publishes_full_distinct_report_and_never_reacquires() {
    let program = crate::check(SOURCE, "vector-work.spx").unwrap();
    let ((report, receipt, retry, disposal), refused, _, first) =
        with_limit_usage_refusal(CAP, || {
            let authored = BTreeMap::new();
            let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
                &authored,
                std::slice::from_ref(&program),
            )
            .unwrap();
            core.base_prelude.restrict_extension_work(0);
            let input = Input::Owned(program.clone());
            let ModuleFailure::Report(report) =
                resolve_module_with_core(&mut core, None, &program.path, &input, None).unwrap_err()
            else {
                panic!("prepaid work report")
            };
            let receipt = core.base_prelude.extension_receipt().unwrap();
            assert_eq!(core.base_prelude.receipt().unwrap().remaining, 0);
            let mut called = false;
            let retry = core
                .base_prelude
                .ensure_selected(
                    crate::prelude::declarations_for_program(&program),
                    &mut |_| {
                        called = true;
                        Ok(())
                    },
                )
                .unwrap_err();
            assert!(!called);
            let ((), disposal) = measure(|| drop(core));
            (report, receipt, retry, disposal)
        });
    assert!(!refused && first.is_none());
    assert_eq!(
        report,
        vec![Diagnostic::io(
            "SPX-H006",
            "prelude extension producer work bound refused"
        )]
    );
    assert_eq!(
        receipt.first,
        Some(crate::hir::prelude_bundle::WorkRefusal {
            requested: 1,
            remaining: 0
        })
    );
    assert_eq!(
        retry,
        InvocationFailure::ExtensionWork(receipt.first.unwrap())
    );
    assert!(!disposal.invalid && disposal.allocated == 0);
}

#[test]
fn all_admitted_core_prefixes_keep_full_old_hir_wire_payload_owners_and_final_drop() {
    let fixtures = [
        (2, "module e; @id(\"app.main\") fn main()->i64 {0}"),
        (3, "module e; @id(\"app.read\") fn read(x:borrow Vec<i64>)->i64 {0} @id(\"app.main\") fn main()->i64 {0}"),
        (4, "module e; @id(\"app.main\") fn main()->i64 {box_into_inner<i64>(box_new<i64>(0))}"),
        (6, "module e; @id(\"app.read\") fn read(x:borrow Iter<i64>)->i64 {0} @id(\"app.main\") fn main()->i64 {0}"),
        (8, "module e; @id(\"app.read\") fn read(x:List<i64>)->i64 {0} @id(\"app.main\") fn main()->i64 {0}"),
        (9, "module e; @id(\"app.read\") fn read(x:borrow StdinReader)->i64 {0} @id(\"app.main\") fn main()->i64 {0}"),
        (11, "module e; @id(\"app.read\") fn read(x:borrow Map<i64,i64>)->i64 {0} @id(\"app.main\") fn main()->i64 {0}"),
    ];
    for (count, source) in fixtures {
        let program = crate::check(source, "extension-prefix.spx").unwrap();
        assert_eq!(
            crate::prelude::declarations_for_program(&program).len(),
            count
        );
        let (((), disposal), refused, _, first) = with_limit_usage_refusal(CAP, || {
            let authored = BTreeMap::new();
            let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
                &authored,
                std::slice::from_ref(&program),
            )
            .unwrap();
            let old = crate::hir::identity_resolution::nominal_original::resolve(&program).unwrap();
            let input = Input::Owned(program.clone());
            let actual =
                resolve_module_with_core(&mut core, None, &program.path, &input, None).unwrap();
            let receipts = *core.base_prelude.extension_receipts();
            let view =
                crate::workspace_graph::expected_projection::borrowed_leaf::construct(&program)
                    .unwrap();
            let borrowed = Input::Borrowed(view);
            let other =
                resolve_module_with_core(&mut core, None, &program.path, &borrowed, None).unwrap();
            let cache = FrontendPass::paid_receiver_candidate(
                program.clone(),
                old.0.clone(),
                old.1.costs(),
            );
            let selective =
                resolve_module_with_core(&mut core, Some(&cache), &program.path, &input, None)
                    .unwrap();
            for current in [&actual.0, &other.0, &selective.0] {
                assert_eq!(current, &old.0);
                assert_eq!(
                    crate::cache_codec::encode(current).unwrap(),
                    crate::cache_codec::encode(&old.0).unwrap()
                );
            }
            assert_eq!(*core.base_prelude.extension_receipts(), receipts);
            drop(core);
            for left in &actual.0.types {
                let Some(right) = other.0.types.iter().find(|right| right.id == left.id) else {
                    panic!("full type view")
                };
                if !left.type_parameters.is_empty() {
                    assert!(left.type_parameters.same_owner(&right.type_parameters));
                }
                if let (
                    crate::hir::ResolvedTypeDeclarationKind::Variant { cases: a },
                    crate::hir::ResolvedTypeDeclarationKind::Variant { cases: b },
                ) = (&left.kind, &right.kind)
                {
                    assert!(a.same_owner(b));
                }
            }
            let ((), disposal) = measure(|| {
                drop(actual);
                drop(other);
                drop(selective);
            });
            ((), disposal)
        });
        assert!(!refused && first.is_none());
        assert!(!disposal.invalid && disposal.allocated == 0);
    }
}
