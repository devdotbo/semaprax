//! Frozen iterator default/record precedence with paid borrowed emissions.
use super::{compare, iterator_shape, Admission, Directory, Entry, Family, Refusal, Scratch};
use crate::hir::{
    DeclarationKind, IdentityOrigin, OwnershipMode as M, ResolvedBinding as B, ResolvedExpr as E,
    ResolvedExprKind as K, ResolvedFunction as F, ResolvedProgram as P, ResolvedStatement as S,
    ResolvedType as T,
};
fn children<'a>(
    expression: &'a E,
    pending: &mut Scratch<(&'a E, bool)>,
    conditional: bool,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let start = pending.as_slice().len();
    if let K::Closure { captures, .. } = &expression.kind {
        for capture in captures {
            admission.units(1)?;
            let child = &capture.value;
            pending.push((child, conditional), admission)?;
        }
    } else {
        let mut cursor = crate::hir::lexical_exit::ChildCursor::new(expression);
        loop {
            admission.units(1)?;
            let Some(child) = cursor.next() else {
                break;
            };
            pending.push((child, conditional), admission)?;
        }
    }
    pending.reverse_suffix(start, admission)
}
fn record(program: &P, ty: &T, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    admission.units(1)?;
    let T::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !arguments.is_empty() {
        return Ok(false);
    }
    let index = admission.probes.unwrap_or(&program.declarations);
    let Some(item) = index
        .declaration(declaration, &mut |units| admission.units(units).is_ok())
        .map_err(|_| Refusal::Work)?
    else {
        return Ok(false);
    };
    if item.kind != DeclarationKind::Record || item.identity_origin != IdentityOrigin::Explicit {
        return Ok(false);
    }
    let Some(parameters) = index
        .parameters(declaration, &mut |units| admission.units(units).is_ok())
        .map_err(|_| Refusal::Work)?
    else {
        return Ok(false);
    };
    if !parameters.is_empty() {
        return Ok(false);
    }
    let Some(fields) = index
        .fields(declaration, &mut |units| admission.units(units).is_ok())
        .map_err(|_| Refusal::Work)?
    else {
        return Ok(false);
    };
    if fields.len() != 2 {
        return Ok(false);
    }
    let mut bytes = 0;
    for field in fields {
        admission.units(1)?;
        bytes += usize::from(field.ty == T::Bytes);
    }
    if bytes != 1 {
        return Ok(false);
    }
    let mut sizes = 0;
    for field in fields {
        admission.units(1)?;
        sizes += usize::from(field.ty == T::Usize);
    }
    if sizes != 1 {
        return Ok(false);
    }
    for field in fields {
        let Some(item) = index
            .declaration(&field.id, &mut |units| admission.units(units).is_ok())
            .map_err(|_| Refusal::Work)?
        else {
            return Ok(false);
        };
        if item.identity_origin != IdentityOrigin::Explicit {
            return Ok(false);
        }
    }
    Ok(true)
}
fn place(value: &E, binding: &B, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    admission.units(1)?;
    let K::Place(place) = &value.kind else {
        return Ok(false);
    };
    Ok(compare::values(&place.root, &binding.id, admission)? && place.projections.is_empty())
}
fn record_update(
    program: &P,
    binding: &B,
    value: &E,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    admission.units(1)?;
    if binding.ownership != M::Own
        || value.ownership != M::Own
        || !compare::types(&binding.ty, &value.ty, admission)?
        || !record(program, &binding.ty, admission)?
    {
        return Ok(false);
    }
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    let mut target = None;
    // resolve_call_target(None) searches ordinary functions only, in order.
    for function in &program.functions {
        admission.units(1)?;
        if compare::text(function.id.as_str(), callee.as_str(), admission)? {
            target = Some(function);
            break;
        }
    }
    let Some(target) = target else {
        return Ok(false);
    };
    if !target.effects.is_empty()
        || !type_arguments.is_empty()
        || !compare::types(&target.return_type, &binding.ty, admission)?
        || target.params.len() != args.len()
    {
        return Ok(false);
    }
    let mut owners = 0;
    for (parameter, argument) in target.params.iter().zip(args) {
        admission.units(1)?;
        if parameter.ownership == M::Own
            && compare::types(&parameter.ty, &binding.ty, admission)?
            && place(argument, binding, admission)?
        {
            owners += 1;
        }
    }
    if owners != 1 {
        return Ok(false);
    }
    for (parameter, argument) in target.params.iter().zip(args) {
        admission.units(1)?;
        if !compare::types(&parameter.ty, &argument.ty, admission)? {
            return Ok(false);
        }
        let admitted = match parameter.ownership {
            M::Own => {
                argument.ownership == M::Own
                    && compare::types(&parameter.ty, &binding.ty, admission)?
                    && place(argument, binding, admission)?
            }
            M::Borrow => {
                if !matches!(argument.ownership, M::Own | M::Borrow)
                    || !matches!(&argument.kind, K::Place(place) if place.projections.is_empty())
                {
                    false
                } else {
                    record(program, &parameter.ty, admission)?
                        || (argument.ownership == M::Borrow
                            && crate::loop_calls::resolved_renewal_view(&parameter.ty))
                }
            }
            M::Value => argument.ownership == M::Value,
            M::Shared => false,
        };
        if !admitted {
            return Ok(false);
        }
    }
    Ok(true)
}
fn scalar_update(binding: &B, value: &E, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    admission.units(1)?;
    let T::Nominal {
        declaration,
        arguments,
    } = &binding.ty
    else {
        return Ok(false);
    };
    let [element] = arguments.as_slice() else {
        return Ok(false);
    };
    if !compare::text(declaration.as_str(), crate::prelude::VEC_ID, admission)?
        || binding.ownership != M::Own
        || !compare::types(&value.ty, &binding.ty, admission)?
        || value.ownership != M::Own
        || !crate::hir::generic_collection::scalar(element)
    {
        return Ok(false);
    }
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    let op = match crate::vec_ops::by_id_with_work(callee.as_str(), &mut |units| {
        admission.units(units).is_ok()
    }) {
        Ok(Some(op)) => op,
        Ok(None) => return Ok(false),
        Err(()) => return admission.refuse(Refusal::Work),
    };
    if !op.reopens_same_owner() || args.len() != op.arity() {
        return Ok(false);
    }
    let [argument] = type_arguments.as_slice() else {
        return Ok(false);
    };
    if !compare::types(argument, element, admission)? {
        return Ok(false);
    }
    let Some(first) = args.first() else {
        return Ok(false);
    };
    Ok(place(first, binding, admission)?
        && compare::types(&first.ty, &binding.ty, admission)?
        && first.ownership == M::Own)
}
fn inside<'a>(
    program: &P,
    body: &'a E,
    records: bool,
    output: &mut Scratch<Entry<'a>>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let mut pending = Scratch::new(admission)?;
    pending.push((body, false), admission)?;
    while let Some((expression, conditional)) = pending.pop(admission)? {
        if !records {
            if let K::If {
                condition,
                then_branch,
                else_branch,
            } = &expression.kind
            {
                pending.push((condition, conditional), admission)?;
                pending.push((then_branch, true), admission)?;
                pending.push((else_branch, true), admission)?;
                continue;
            }
        }
        if records || conditional {
            if let K::Block { statements, .. } | K::VecConstructor { statements, .. } =
                &expression.kind
            {
                for statement in statements {
                    admission.units(1)?;
                    if let S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    {
                        let admitted = if records {
                            record_update(program, binding, value, admission)?
                        } else {
                            scalar_update(binding, value, admission)?
                        };
                        if admitted {
                            Directory::emit(
                                output,
                                Family::Iterator,
                                &value.id,
                                binding,
                                admission,
                            )?;
                        }
                    }
                }
            }
        }
        children(expression, &mut pending, conditional, admission)?;
    }
    Ok(())
}
pub(super) fn collect<'a>(
    program: &P,
    function: &'a F,
    output: &mut Scratch<Entry<'a>>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    // Record first, default second: default wins malformed duplicate IDs.
    for records in [true, false] {
        let mut pending = Scratch::new(admission)?;
        pending.push((&function.body, false), admission)?;
        while let Some((expression, _)) = pending.pop(admission)? {
            if let K::Block { statements, .. } | K::VecConstructor { statements, .. } =
                &expression.kind
            {
                for statement in statements {
                    admission.units(1)?;
                    if let S::While {
                        condition, body, ..
                    }
                    | S::LexicalWhile {
                        condition, body, ..
                    } = statement
                    {
                        if records {
                            inside(program, body, true, output, admission)?;
                        } else if let Some(protocol) =
                            iterator_shape::recognize(condition, body, admission)?
                        {
                            inside(program, protocol.authored_body, false, output, admission)?;
                        }
                    }
                }
            }
            children(expression, &mut pending, false, admission)?;
        }
    }
    Ok(())
}
