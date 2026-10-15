//! The legacy bool target boundary with one independently owned physical stack.
use super::*;
use crate::hir::ResolvedProgram;
pub(in crate::loop_calls::byte_writer) fn target<'a>(
    program: &'a ResolvedProgram,
    target: &'a ResolvedFunction,
    cursor: &mut Vec<ProofCursor<'a>>,
) -> bool {
    if target.return_type != ResolvedType::Bytes
        || target
            .params
            .iter()
            .filter(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
            .count()
            != 1
        || !target
            .params
            .iter()
            .all(|p| resolved_param(program, p.ownership, &p.ty))
    {
        return false;
    }
    if cursor.is_empty() {
        // Pay both the held heap stack and its constructor's transient stack.
        // A refused proof remains false, exactly as resolved_target; no new
        // allocated Diagnostic is manufactured or retried as authority.
        let bytes = std::mem::size_of::<ProofCursor<'a>>();
        if !crate::bounded_output::reserve_active_required(bytes)
            || !crate::bounded_output::reserve_active_required(bytes)
        {
            return false;
        }
        if cursor.try_reserve_exact(1).is_err() {
            crate::bounded_output::refuse_active_required(bytes);
            return false;
        }
        if cursor.capacity() != 1 {
            if cursor.capacity() > 1 {
                let _ = crate::bounded_output::reserve_active_required(
                    cursor.capacity().saturating_sub(1).saturating_mul(bytes),
                );
            }
            crate::bounded_output::refuse_active_required(cursor.capacity().saturating_mul(bytes));
            return false;
        }
        cursor.push(ProofCursor {
            frames: [None; DEPTH],
        });
    }
    nonallocating_with_cursor(
        target,
        |id, instance| {
            <ResolvedProgram as crate::cleanup::ReplayContext>::resolve_call_target(
                program, id, instance,
            )
        },
        |_, mode, ty| resolved_param(program, mode, ty),
        |_, ty| {
            crate::hir::is_scalar_resolved_type(ty)
                || crate::hir::copy_record_collection::admitted(&program.declarations, ty)
                || super::super::super::resolved_match_scrutinee_admitted(&program.declarations, ty)
        },
        program
            .functions
            .len()
            .saturating_add(program.function_instances.len()),
        &mut cursor[0],
    )
}
