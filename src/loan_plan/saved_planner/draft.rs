//! Saved original Cfg-coupled traversal drafts, adapted to the saved `Cfg`.
//!
//! `traversal::draft` and `traversal::draft_bytes` take the production `Cfg`;
//! these are the same bodies over the saved nested-table `Cfg`. The unchanged
//! inspect logic they call (`traversal::inspect_counted`) is shared by note.

use super::*;

pub(in crate::loan_plan) fn draft(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &ResolvedExpr,
    cfg: &Cfg<'_>,
    work: &mut WorkCounter,
) -> Result<Option<CfgDraft>, Diagnostic> {
    let Some(proof) = traversal::inspect_counted(program, expression, work)? else {
        return Ok(None);
    };
    let origin = expression_place(proof.source)
        .ok_or_else(|| error("record traversal source lacks its exact place"))?;
    Ok(Some(CfgDraft {
        site: proof.region.id.clone(),
        origin,
        origin_span: proof.source.span,
        parent_root: None,
        binding: None,
        start: cfg.node(proof.region, LoanPointPhase::Before)?,
        seeds: [cfg.node(proof.region, LoanPointPhase::After)?]
            .into_iter()
            .collect(),
        cause: LoanCause::TraversalFreeze,
    }))
}

/// Borrowed `Slice<u8>` traversal: the view's resolved owner is frozen from
/// before the complete lowered region until after it. The loan is independent
/// of the view's own liveness, which may end at its last read inside the body.
pub(in crate::loan_plan) fn draft_bytes(
    expression: &ResolvedExpr,
    cfg: &Cfg<'_>,
    aliases: &BTreeMap<ValueId, Place>,
    work: &mut WorkCounter,
) -> Result<Option<CfgDraft>, Diagnostic> {
    let Some(region) = crate::hir::byte_traversal_region(expression)? else {
        return Ok(None);
    };
    charge(work)?;
    let place = expression_place(region.view)
        .ok_or_else(|| error("borrowed byte traversal view lacks its exact place"))?;
    Ok(Some(CfgDraft {
        site: expression.id.clone(),
        origin: resolve_origin(aliases, place, work)?,
        origin_span: region.view.span,
        parent_root: None,
        binding: None,
        start: cfg.node(expression, LoanPointPhase::Before)?,
        seeds: [cfg.node(expression, LoanPointPhase::After)?]
            .into_iter()
            .collect(),
        cause: LoanCause::TraversalFreeze,
    }))
}
