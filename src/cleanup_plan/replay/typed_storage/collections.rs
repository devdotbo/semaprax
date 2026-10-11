//! Prepaid exact call signatures reconstructed from authenticated operands.
use super::*;
fn params(
    args: &[ResolvedExpr],
    operation: &str,
    names: &[&str],
    ownership: impl Fn(usize) -> OwnershipMode,
    field: bool,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    work.budget.reserve_array::<crate::hir::ResolvedParam>(
        work.function,
        args.len(),
        "selected collection parameter slots",
    )?;
    let mut result = Vec::with_capacity(args.len());
    for (index, (arg, name)) in args.iter().zip(names).enumerate() {
        work.charge(name.len() + 1, "selected collection parameter header/name")?;
        let id = parameter_id(operation, index, work, false)?;
        work.budget.reserve_bytes(
            work.function,
            name.len(),
            "selected collection parameter name",
        )?;
        let ty = work.clone_payload(&arg.ty, "selected collection parameter type")?;
        result.push(crate::hir::ResolvedParam {
            id,
            name: (*name).to_owned(),
            ty,
            ownership: ownership(index),
            span: if field {
                arg.span
            } else {
                crate::ast::Span::default()
            },
        });
    }
    Ok(result)
}
pub(in crate::cleanup_plan::replay) fn vector_params(
    program: &dyn ReplayContext,
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    if !selected_profile::collections::call(program, work.function, expression, work.budget)? {
        return Err(replay_error(
            work.function,
            "selected collection signature lacks exact Vec operation",
        ));
    }
    let ResolvedExprKind::Call {
        callee,
        args,
        type_arguments,
        ..
    } = &expression.kind
    else {
        unreachable!()
    };
    let op = crate::vec_ops::by_id(callee.as_str()).expect("authenticated Vec call");
    let element = &type_arguments[0];
    let owned = *element == ResolvedType::Bytes
        || *element == ResolvedType::String
        || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element);
    params(
        args,
        op.id(),
        &["arg0", "arg1", "arg2"],
        |i| {
            if owned {
                op.param_ownership_for(i, element)
            } else {
                op.param_ownership(i)
            }
        },
        false,
        work,
    )
}
pub(in crate::cleanup_plan::replay) fn field_params(
    program: &dyn ReplayContext,
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    selected_profile::collections::field(program, work.function, expression, work.budget)?;
    let ResolvedExprKind::VecFieldRead { args, .. } = &expression.kind else {
        return Err(replay_error(
            work.function,
            "selected field signature lacks exact field operation",
        ));
    };
    params(
        args,
        crate::vec_field::ID,
        &["value0", "value1"],
        |i| {
            if i == 0 {
                OwnershipMode::Borrow
            } else {
                OwnershipMode::Value
            }
        },
        true,
        work,
    )
}
