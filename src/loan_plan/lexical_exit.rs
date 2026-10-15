//! Exit successors are reconstructed from typed HIR, never cleanup labels.
use super::*;
use crate::hir::{ControlTarget, LoopLatchKind};
pub(super) fn destination<'a>(
    expression: &ResolvedExpr,
    expressions: &[&'a ResolvedExpr],
    work: &mut WorkCounter,
) -> Result<&'a ResolvedExpr, Diagnostic> {
    let ResolvedExprKind::LexicalExit {
        kind,
        target: ControlTarget::Loop { site, .. },
        ..
    } = &expression.kind
    else {
        return Err(error("loop exit has no typed target"));
    };
    for region in expressions {
        charge(work)?;
        let ResolvedExprKind::Block { statements, tail } = &region.kind else {
            continue;
        };
        for (index, statement) in statements.iter().enumerate() {
            charge(work)?;
            let ResolvedStatement::LexicalWhile {
                condition, latch, ..
            } = statement
            else {
                continue;
            };
            if &condition.id != site {
                continue;
            }
            if *kind == crate::ast::LexicalExitKind::Break {
                return Ok(statements
                    .get(index + 1)
                    .map(statement_entry)
                    .unwrap_or(tail));
            }
            let id = match latch {
                LoopLatchKind::Condition => return Ok(condition),
                LoopLatchKind::SourceForStep { step, .. } => step,
                LoopLatchKind::OwnedIteratorStep { next, .. } => next,
            };
            for candidate in expressions {
                charge(work)?;
                if &candidate.id == id {
                    return Ok(candidate);
                }
            }
            return Err(error(
                "loop latch expression is absent from the authenticated function",
            ));
        }
    }
    Err(error(
        "lexical loop target has no independently reconstructed successor",
    ))
}
