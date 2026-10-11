//! Summed legacy retention receipts; no execution-phase peak permission.
use super::{
    AuthoredDeclaration, active_builder_limit, checked_usage, cost, dependency_identity_max,
    next_retention_prebound_with_uncached_peak, synthetic_builder_bytes,
    synthetic_builder_bytes_scoped, uncached_peak_prebound,
};
use crate::{ast::Program, diagnostic::Diagnostic};
use std::collections::BTreeMap;

/// Preserve the exact legacy receipt whenever it fits. Only a legacy builder
/// refusal activates the narrower, still conservative identity universe.
pub(in crate::workspace_graph) fn checked_retention_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    checked_retention_prebound_with_uncached_peak(programs, authored, false)
}

/// Modes zero through four preserve existing receipts. The uncached core
/// builds and drops one synthetic AST per source, so only that construction
/// work may use the final peak-only receipt. A semantic frontend stages those
/// synthetic ASTs and must retain the ordinary summed receipts.
pub(in crate::workspace_graph) fn checked_retention_prebound_with_uncached_peak(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    allow_uncached_peak: bool,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    match retention_prebound(programs, authored, false) {
        Ok(costs) => Ok(costs),
        Err(errors) if cost::is_builder_refusal(&errors) => {
            match retention_prebound(programs, authored, true) {
                Ok(costs) => Ok(costs),
                Err(errors) if cost::is_builder_refusal(&errors) => {
                    match retention_prebound_mode(programs, authored, true, 2) {
                        Ok(costs) => Ok(costs),
                        Err(errors) if cost::is_builder_refusal(&errors) => {
                            match retention_prebound_mode(programs, authored, true, 3) {
                                Err(errors) if cost::is_builder_refusal(&errors) => {
                                    match retention_prebound_mode(programs, authored, true, 4) {
                                        Err(errors)
                                            if cost::is_builder_refusal(&errors)
                                                && allow_uncached_peak =>
                                        {
                                            uncached_peak_prebound(programs, authored)
                                        }
                                        result => result,
                                    }
                                }
                                result => result,
                            }
                        }
                        Err(errors) => Err(errors),
                    }
                }
                Err(errors) => Err(errors),
            }
        }
        Err(errors) => Err(errors),
    }
}

pub(in crate::workspace_graph) fn retention_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    dependency_scoped: bool,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    retention_prebound_mode(programs, authored, dependency_scoped, 0)
}
pub(in crate::workspace_graph) fn retention_prebound_mode(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    dependency_scoped: bool,
    layout_mode: u8,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    let _precharge = super::super::diagnostics::precharge_scope();
    super::super::diagnostics::begin_precharge_pass();
    let mut resolve = 0usize;
    let mut runtime = 0usize;
    // build_resolved_core calls synthetic_program once per source, in order.
    // synthetic_program replaces each imported full-function clone's body and
    // contracts before starting the next import; the finished synthetic
    // module retains only its stub. Thus retained modules add, while at most
    // one full imported body is transiently live across the whole loop.
    let mut transient_peak = 0usize;
    for program in programs {
        super::super::diagnostics::begin_precharge_module(&program.path);
        let maximum = if dependency_scoped {
            Some(dependency_identity_max(program, authored, programs)?)
        } else {
            None
        };
        let costs = if maximum.is_some() {
            synthetic_builder_bytes_scoped(program, authored, programs, maximum, layout_mode, true)?
        } else {
            synthetic_builder_bytes(program, authored, programs)?
        };
        let charged = if layout_mode >= 3 {
            costs.retained_clone_and_hir
        } else {
            costs.raw_clone_and_hir
        };
        super::super::diagnostics::record_precharge_module(&program.path, charged);
        resolve = checked_usage(resolve, charged, "builder_bytes", active_builder_limit())?;
        transient_peak = transient_peak.max(costs.transient_import_clone);
        runtime = checked_usage(
            runtime,
            costs.runtime,
            "builder_bytes",
            active_builder_limit(),
        )?;
    }
    if layout_mode == 3 {
        resolve = checked_usage(
            resolve,
            transient_peak,
            "builder_bytes",
            active_builder_limit(),
        )?;
    }
    let total = checked_usage(resolve, runtime, "builder_bytes", active_builder_limit())?;
    Ok((resolve, total))
}

/// Return the next strictly smaller core reservation after a real builder
/// overflow. A refusal in one fallback is not authority to retry the same
/// reservation or to skip an unrelated diagnostic. The caller owns attempt
/// rollback and uses `layout_mode = 1` before the first retry, so modes 2
/// through 4 are considered in order.
pub(in crate::workspace_graph) fn next_retention_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    layout_mode: &mut u8,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    next_retention_prebound_with_uncached_peak(programs, authored, current, layout_mode, false)
}
