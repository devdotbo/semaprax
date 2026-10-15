//! The legacy SIMD report cannot claim control-flow semantics from lexical proofs.
use super::*;
pub(super) fn reject(program: &hir::ResolvedProgram) -> Result<(), Vec<Diagnostic>> {
    for function in &program.functions {
        if hir::lexical_exit::requires(function).map_err(|d| vec![d])? {
            return Err(vec![consistency_error(
                "lexical control flow is outside simd-report v1".to_owned(),
            )]);
        }
    }
    Ok(())
}
pub(super) fn scan(
    walker: &mut Walker<'_>,
    expression: &ResolvedExpr,
    value: Option<&ResolvedExpr>,
) {
    walker.push_ineligible(expression, REASON_CONTROL_FLOW);
    if let Some(value) = value {
        walker.scan_expr(value);
    }
}
