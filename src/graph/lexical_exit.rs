//! Typed lexical graph projection. Certificates remain independently replayed authority.
use super::*;
use crate::hir::{ControlTarget, LexicalExitKind, LoopLatchKind, SourceForKind};
fn target_json(target: &ControlTarget) -> String {
    match target {
        ControlTarget::Function(owner) => format!(
            "{{\"kind\":\"function\",\"owner\":{}}}",
            quote_json(&owner.identity_key())
        ),
        ControlTarget::Loop { owner, site } => format!(
            "{{\"kind\":\"loop\",\"owner\":{},\"site\":{}}}",
            quote_json(&owner.identity_key()),
            quote_json(site.as_str())
        ),
    }
}
fn latch_json(latch: &LoopLatchKind) -> String {
    match latch {
        LoopLatchKind::Condition => "{\"kind\":\"condition\"}".to_owned(),
        LoopLatchKind::SourceForStep { kind, step } => format!("{{\"kind\":\"source_for_step\",\"source_kind\":{},\"step\":{}}}",quote_json(match kind { SourceForKind::Copy=>"copy",SourceForKind::BorrowRecord=>"borrow_record",SourceForKind::OwnedIterator=>"owned_iterator",SourceForKind::BorrowBytes=>"borrow_bytes"}),quote_json(step.as_str())),
        LoopLatchKind::OwnedIteratorStep { next, assignment, step, remainder } => format!("{{\"kind\":\"owned_iterator_step\",\"next\":{},\"assignment\":{},\"step\":{},\"remainder\":{}}}",quote_json(next.as_str()),quote_json(assignment.as_str()),quote_json(step.as_str()),quote_json(remainder.as_str())),
    }
}
pub(super) fn expression_json(
    program: &ResolvedProgram,
    header: &str,
    kind: &LexicalExitKind,
    target: &ControlTarget,
    value: Option<&ResolvedExpr>,
) -> Result<String, Diagnostic> {
    let operand = match value {
        Some(value) => expr_json(program, value)?,
        None => "null".to_owned(),
    };
    Ok(format!(
        "{{{header},\"kind\":\"lexical_exit\",\"exit_kind\":{},\"target\":{},\"value\":{operand}}}",
        quote_json(match kind {
            LexicalExitKind::Break => "break",
            LexicalExitKind::Continue => "continue",
            LexicalExitKind::Return => "return",
        }),
        target_json(target)
    ))
}
pub(super) fn statement_json(
    program: &ResolvedProgram,
    condition: &ResolvedExpr,
    body: &ResolvedExpr,
    latch: &LoopLatchKind,
) -> Result<String, Diagnostic> {
    Ok(format!(
        "{{\"kind\":\"lexical_while\",\"condition\":{},\"body\":{},\"latch\":{}}}",
        expr_json(program, condition)?,
        expr_json(program, body)?,
        latch_json(latch)
    ))
}
