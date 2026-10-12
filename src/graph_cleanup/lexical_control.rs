//! Render certificates verbatim; validation owns all semantic authentication.
use super::{array_json, quote_json};
use crate::cleanup_plan::{
    ExecutionBoundaryCertificate, ExecutionBoundaryKind, LexicalControlPlan,
    LexicalExitCertificate, LoopCertificate, ResumeCertificate,
};
use crate::hir::{ControlTarget, LoopLatchKind, SourceForKind};

pub(super) fn target_json(target: &ControlTarget) -> String {
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
        LoopLatchKind::SourceForStep { kind, step } => {
            let kind = match kind {
                SourceForKind::Copy => "copy",
                SourceForKind::BorrowRecord => "borrow_record",
                SourceForKind::OwnedIterator => "owned_iterator",
                SourceForKind::BorrowBytes => "borrow_bytes",
            };
            format!("{{\"kind\":\"source_for_step\",\"source_kind\":{},\"step\":{}}}",
                quote_json(kind), quote_json(step.as_str()))
        }
        LoopLatchKind::OwnedIteratorStep { next, assignment, step, remainder } => format!(
            "{{\"kind\":\"owned_iterator_step\",\"next\":{},\"assignment\":{},\"step\":{},\"remainder\":{}}}",
            quote_json(next.as_str()), quote_json(assignment.as_str()),
            quote_json(step.as_str()), quote_json(remainder.as_str())
        ),
    }
}

fn loop_json(item: &LoopCertificate) -> String {
    format!(
        "{{\"target\":{},\"condition\":{},\"body\":{},\"latch\":{},\"region\":{},\"condition_entry\":{},\"latch_entry\":{},\"iteration_end\":{},\"after\":{},\"entry_order\":{}}}",
        target_json(&item.target), quote_json(item.condition.as_str()),
        quote_json(item.body.as_str()), latch_json(&item.latch), item.region.0,
        item.condition_entry.0,
        item.latch_entry.map(|id| format!("{}", id.0)).unwrap_or_else(|| "null".to_owned()),
        item.iteration_end.0, item.after.0,
        array_json(&item.entry_order, |flag| format!("{}", flag.0))
    )
}

fn exit_json(item: &LexicalExitCertificate) -> String {
    let kind = match item.kind {
        crate::ast::LexicalExitKind::Break => "break",
        crate::ast::LexicalExitKind::Continue => "continue",
        crate::ast::LexicalExitKind::Return => "return",
    };
    format!(
        "{{\"site\":{},\"target\":{},\"kind\":{},\"exit\":{},\"resume\":{}}}",
        quote_json(item.site.as_str()),
        target_json(&item.target),
        quote_json(kind),
        item.exit.0,
        item.resume
            .map(|id| format!("{}", id.0))
            .unwrap_or_else(|| "null".to_owned())
    )
}

fn resume_json(item: &ResumeCertificate) -> String {
    format!(
        "{{\"entry\":{},\"blocks\":{}}}",
        item.entry.0,
        array_json(&item.blocks, |id| format!("{}", id.0))
    )
}

pub(super) fn member_json(control: &LexicalControlPlan) -> String {
    format!(",\"lexical_control\":{{\"schema\":{},\"loops\":{},\"exits\":{},\"resumes\":{},\"abrupt_only_regions\":{},\"function_success_exits\":{},\"execution_boundaries\":{}}}",
        quote_json(control.schema), array_json(&control.loops, loop_json),
        array_json(&control.exits, exit_json), array_json(&control.resumes, resume_json),
        array_json(&control.abrupt_only_regions, |id| format!("{}", id.0)),
        array_json(&control.function_success_exits, |id| format!("{}", id.0)),
        array_json(&control.execution_boundaries, boundary_json))
}

fn boundary_json(item: &ExecutionBoundaryCertificate) -> String {
    let kind = match item.kind {
        ExecutionBoundaryKind::FunctionEntry => "function_entry",
        ExecutionBoundaryKind::LoopIteration => "loop_iteration",
    };
    format!("{{\"kind\":{},\"site\":{},\"block\":{},\"region\":{},\"leaves_regions\":{},\"finalize_in_order\":{}}}",
        quote_json(kind), quote_json(item.site.as_str()), item.block.0, item.region.0,
        array_json(&item.leaves_regions, |id| format!("{}", id.0)),
        array_json(&item.finalize_in_order, super::finalize_action_json))
}
