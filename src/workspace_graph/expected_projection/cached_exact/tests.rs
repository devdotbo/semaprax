use super::{summed, Retention};
use crate::ast::Program;
fn programs() -> Vec<Program> {
    ["one", "two"]
        .into_iter()
        .map(|module| {
            crate::parse(
                &format!("module {module}; @id(\"{module}.main\") fn main()->i64 {{1+2}}"),
                std::path::Path::new(&format!("{module}.spx")),
            )
            .unwrap()
        })
        .collect()
}
struct Restore(usize);
impl Drop for Restore {
    fn drop(&mut self) {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
    }
}
fn limit(bytes: usize) -> Restore {
    Restore(crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(bytes)))
}
#[test]
fn semantic_cache_charges_two_exact_hir_bundles_without_duplicating_arc_carriers() {
    let programs = programs();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let parsed = summed(&programs, &authored, Retention::Parsed).unwrap();
    let semantic = summed(&programs, &authored, Retention::Semantic).unwrap();
    // Independently known AST: two modules, each Block/Binary/Int/Int.
    let extra = 8 * crate::workspace_graph::HIR_EXPR_FIXED_BUNDLE;
    assert_eq!(semantic.0 - parsed.0, extra);
    assert_eq!(semantic.1 - parsed.1, extra);
    let root = crate::hir::DeclarationId::new("one.main");
    let id =
        crate::hir::ExpressionId::new(&crate::hir::FunctionExecutionId::Monomorphic(root), "body");
    let cloned = id.clone();
    assert_eq!(id.as_str().as_ptr(), cloned.as_str().as_ptr());
    let mut malformed = super::super::StructuralCost::new();
    malformed.exact_expressions = 1;
    assert!(super::fixed_upper(
        0,
        &malformed,
        &super::super::StructuralCost::new(),
        Retention::Semantic
    )
    .is_err());
    malformed.exact_expressions = usize::MAX;
    assert!(super::fixed_upper(
        usize::MAX,
        &malformed,
        &super::super::StructuralCost::new(),
        Retention::Semantic
    )
    .is_err());
}
#[test]
fn cached_exact_forecast_uses_all_module_sums_and_exact_one_short_limit() {
    let programs = programs();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    for retention in [Retention::Parsed, Retention::Semantic] {
        let full = summed(&programs, &authored, retention).unwrap();
        let parts: Vec<_> = programs
            .iter()
            .map(|program| summed(std::slice::from_ref(program), &authored, retention).unwrap())
            .collect();
        assert_eq!(full.0, parts.iter().map(|part| part.0).sum::<usize>());
        assert_eq!(full.1, parts.iter().map(|part| part.1).sum::<usize>());
        {
            let _restore = limit(full.1);
            assert_eq!(summed(&programs, &authored, retention).unwrap(), full);
        }
        {
            let _restore = limit(full.1 - 1);
            assert!(summed(&programs, &authored, retention)
                .unwrap_err()
                .iter()
                .any(|error| error.code == "SPX-G171"));
        }
        {
            let _restore = limit(parts.iter().map(|part| part.1).max().unwrap());
            assert!(summed(&programs, &authored, retention).is_err());
        }
    }
}
#[test]
fn cached_exact_preserves_old_fitting_receipts_and_never_selects_peak_or_filter() {
    let programs = programs();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let old = super::super::initial_core_prebound(&programs, &authored, false).unwrap();
    for retention in [Retention::Parsed, Retention::Semantic] {
        assert_eq!(
            super::initial(&programs, &authored, false, retention).unwrap(),
            old
        );
        let mut mode = 4;
        let exact = summed(&programs, &authored, retention).unwrap();
        assert_eq!(
            super::next(&programs, &authored, usize::MAX, &mut mode, retention).unwrap(),
            exact
        );
        assert_eq!(mode, 7);
        assert!(super::next(&programs, &authored, usize::MAX, &mut mode, retention).is_err());
    }
}
