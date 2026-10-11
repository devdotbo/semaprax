//! Interpreter admission for bounded generic owned-record expressions.

use crate::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedMatchArm,
    ResolvedMatchPattern, ResolvedProgram, ResolvedType,
};

pub(super) fn match_result_is_admitted(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
) -> bool {
    if let ResolvedExprKind::Match { mode, .. } = &expression.kind {
        if *mode == hir::ResolvedMatchMode::Value
            && expression.ownership == OwnershipMode::Value
            && scrutinee.ownership == OwnershipMode::Value
            && crate::variant_guards::copy_variant(&program.declarations, &expression.ty)
            && super::variant_admission::is_admitted_copy_scalar_variant(
                &program.declarations,
                &scrutinee.ty,
            )
            && super::variant_pattern_is_admitted(&program.declarations, *mode, &scrutinee.ty, arms)
        {
            return true;
        }
        if hir::generic_variant::match_result(
            program,
            function,
            *mode,
            &expression.ty,
            expression.ownership,
        ) && super::variant_pattern_is_admitted(
            &program.declarations,
            *mode,
            &scrutinee.ty,
            arms,
        ) {
            return true;
        }
    }
    let generic = hir::bounded_owned_record_template_for_function(program, function).is_some();
    let ordinary = program.functions.iter().any(|item| item.id == function.id);
    if !generic && !ordinary {
        return false;
    }
    let ResolvedExprKind::Match { mode, .. } = &expression.kind else {
        return false;
    };
    let [arm] = arms else { return false };
    let ResolvedMatchPattern::Record {
        record, instance, ..
    } = &arm.pattern
    else {
        return false;
    };
    // Completion is not an owning result carrier. Reconstruct the current
    // function's lexical contour before admitting its terminal arm; the
    // ordinary whole-program validator and child scan still run afterwards.
    let terminal = expression.ty == ResolvedType::Never
        && expression.ownership == OwnershipMode::Value
        && arm.value.ty == ResolvedType::Never
        && arm.value.ownership == OwnershipMode::Value
        && hir::lexical_exit::owner(program, function)
            .and_then(|owner| hir::lexical_exit::validate_function(program, function, &owner))
            .is_ok();
    let result_type = if terminal {
        &function.return_type
    } else {
        &expression.ty
    };
    let ordinary_shape = ordinary
        && (hir::is_admitted_nested_owned_byte_record(&program.declarations, &scrutinee.ty)
            || hir::owned_text_record::runtime_admitted(&scrutinee.ty, &program.declarations)
            || hir::owned_collection_record::admitted(&scrutinee.ty, &program.declarations))
        && (matches!(result_type, ResolvedType::Bytes | ResolvedType::String)
            || crate::map_ops::is_collection(result_type)
            || crate::hir::owned_collection_record::vector(&program.declarations, result_type)
            || (hir::is_admitted_nested_owned_byte_record(&program.declarations, result_type)
                || hir::owned_text_record::runtime_admitted(result_type, &program.declarations)
                || hir::owned_collection_record::admitted(result_type, &program.declarations)))
        && arm.guard.is_none()
        && super::record_pattern_is_admitted(
            &program.declarations,
            *mode,
            &scrutinee.ty,
            &arm.pattern,
        );
    *mode == hir::ResolvedMatchMode::Own
        && expression.ty == arm.value.ty
        && (expression.ownership == OwnershipMode::Own || terminal)
        && scrutinee.ownership == OwnershipMode::Own
        && (arm.value.ownership == OwnershipMode::Own || terminal)
        && instance == &scrutinee.ty
        && matches!(&scrutinee.ty, ResolvedType::Nominal { declaration, .. }
            if declaration == record)
        && (ordinary_shape
            || (generic
                && hir::is_admitted_nested_owned_byte_record(
                    &program.declarations,
                    &expression.ty,
                )))
}

#[cfg(test)]
mod tests;
