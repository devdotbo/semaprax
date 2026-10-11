//! Exact cached construction forecast: sum every module, never peak/filter.
use super::{
    active_builder_limit, checked_usage, cost, dependency_identity_max, limit_error,
    AuthoredDeclaration, StructuralCost,
};
use crate::{ast::Program, diagnostic::Diagnostic};
use std::collections::BTreeMap;
pub(in crate::workspace_graph) const MODE: u8 = 7;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workspace_graph) enum Retention {
    Parsed,
    Semantic,
}
impl Retention {
    fn copies(self) -> usize {
        match self {
            Self::Parsed => 1,
            Self::Semantic => 2,
        }
    }
}
fn overflow() -> Vec<Diagnostic> {
    vec![limit_error("builder_bytes", active_builder_limit())]
}
// Replace only the independently asserted per-expression bundle. Every other
// declaration/container, string-copy, generic/default/import and identity bound
// stays in the original conservative ceiling. ExpressionId clone shares Arc.
pub(super) fn fixed_upper(
    upper: usize,
    raw: &StructuralCost,
    runtime: &StructuralCost,
    retention: Retention,
) -> Result<usize, Vec<Diagnostic>> {
    let count = raw
        .exact_expressions
        .checked_add(runtime.exact_expressions)
        .ok_or_else(overflow)?;
    let old = count
        .checked_mul(super::super::HIR_FIXED_EXPANSION_FACTOR)
        .and_then(|n| n.checked_mul(std::mem::size_of::<crate::ast::Expr>()))
        .ok_or_else(overflow)?;
    let exact = count
        .checked_mul(retention.copies())
        .and_then(|n| n.checked_mul(super::super::HIR_EXPR_FIXED_BUNDLE))
        .ok_or_else(overflow)?;
    upper
        .checked_sub(old)
        .and_then(|n| n.checked_add(exact))
        .ok_or_else(overflow)
}
pub(super) fn summed(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    retention: Retention,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    let _precharge = super::super::diagnostics::precharge_scope();
    super::super::diagnostics::begin_precharge_pass();
    let (mut resolve, mut runtime) = (0, 0);
    for program in programs {
        super::super::diagnostics::begin_precharge_module(&program.path);
        let maximum = Some(dependency_identity_max(program, authored, programs)?);
        let costs = super::synthetic_builder_bytes_scoped_with_retention(
            program,
            authored,
            programs,
            maximum,
            true,
            (MODE, retention),
        )?;
        let charged = costs.retained_clone_and_hir;
        super::super::diagnostics::record_precharge_module(&program.path, charged);
        resolve = checked_usage(resolve, charged, "builder_bytes", active_builder_limit())?;
        runtime = checked_usage(
            runtime,
            costs.runtime,
            "builder_bytes",
            active_builder_limit(),
        )?;
    }
    Ok((
        resolve,
        checked_usage(resolve, runtime, "builder_bytes", active_builder_limit())?,
    ))
}
pub(in crate::workspace_graph) fn initial(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    absent: bool,
    retention: Retention,
) -> Result<(usize, usize, bool, u8), Vec<Diagnostic>> {
    match super::initial_core_prebound(programs, authored, absent) {
        Err(errors) if !absent && cost::is_builder_refusal(&errors) => {
            let (resolve, total) = summed(programs, authored, retention)?;
            Ok((resolve, total, false, MODE))
        }
        result => result,
    }
}
pub(in crate::workspace_graph) fn next(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    mode: &mut u8,
    retention: Retention,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    if *mode < 4 {
        match super::next_retention_prebound_with_uncached_peak(
            programs, authored, current, mode, false,
        ) {
            Err(errors) if cost::is_builder_refusal(&errors) => {}
            result => return result,
        }
    }
    if *mode != 4 {
        return Err(overflow());
    }
    // Deliberately skip modes five and six: a present frontend retains ASTs.
    *mode = MODE;
    let result = summed(programs, authored, retention)?;
    if result.0 >= current {
        return Err(overflow());
    }
    Ok(result)
}

#[cfg(test)]
#[path = "cached_exact/tests.rs"]
mod tests;
