//! Ordinary Vec emissions retain the old two-pass traversal and short circuits.
use super::{compare, iterator_shape, leaf, Admission, Directory, Entry, Family, Refusal, Scratch};
use crate::hir::{
    OwnershipMode as M, ResolvedBinding as B, ResolvedExpr as E, ResolvedExprKind as K,
    ResolvedFunction as F, ResolvedProgram as P, ResolvedStatement as S, ResolvedType as T,
};
use crate::vec_ops::VecOp;
pub(super) fn mode(
    program: &P,
    op: VecOp,
    element: &T,
    admission: &mut Admission<'_>,
) -> Result<Option<M>, Refusal> {
    let result = crate::hir::owned_leaf_collection::operation_with_work(
        admission.probes.unwrap_or(&program.declarations),
        op,
        element,
        &mut |units| admission.units(units).is_ok(),
    );
    let allowed = match result {
        Ok(value) => value,
        Err(()) => return admission.refuse(Refusal::Work),
    };
    if !allowed {
        return Ok(None);
    }
    if *element == T::Bytes {
        return Ok(Some(M::Own));
    }
    let result = crate::hir::owned_leaf_collection::runtime_with_work(
        admission.probes.unwrap_or(&program.declarations),
        element,
        &mut |units| admission.units(units).is_ok(),
    );
    match result {
        Ok(value) => Ok(Some(if value { M::Own } else { M::Value })),
        Err(()) => admission.refuse(Refusal::Work),
    }
}
pub(super) fn same_cell(
    program: &P,
    binding: &B,
    value: &E,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
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
        || value.ownership != M::Own
        || !compare::types(&value.ty, &binding.ty, admission)?
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
    let result = crate::vec_ops::by_id_with_work(callee.as_str(), &mut |units| {
        admission.units(units).is_ok()
    });
    let op = match result {
        Ok(Some(op)) if op.reopens_same_owner() => op,
        Ok(_) => return Ok(false),
        Err(()) => return admission.refuse(Refusal::Work),
    };
    admission.units(1)?;
    let mode = if crate::hir::generic_collection::scalar(element) {
        Some(M::Value)
    } else {
        mode(program, op, element, admission)?
    };
    let Some(mode @ (M::Value | M::Own)) = mode else {
        return Ok(false);
    };
    let [argument] = type_arguments.as_slice() else {
        return Ok(false);
    };
    if !compare::types(argument, element, admission)? || args.len() != op.arity() {
        return Ok(false);
    }
    let Some(first) = args.first() else {
        return Ok(false);
    };
    if !compare::types(&first.ty, &binding.ty, admission)? || first.ownership != M::Own {
        return Ok(false);
    }
    let K::Place(place) = &first.kind else {
        return Ok(false);
    };
    if !compare::values(&place.root, &binding.id, admission)? || !place.projections.is_empty() {
        return Ok(false);
    }
    for (index, argument) in args.iter().enumerate() {
        admission.units(1)?;
        // Prepay the unchanged operation-specific type predicate's actual
        // operands. No expected Vec/argument owner is constructed here.
        compare::type_tree(&argument.ty, admission)?;
        compare::type_tree(element, admission)?;
        if !op.accepts_resolved(index, &argument.ty, element)
            || argument.ownership
                != if mode == M::Own {
                    op.param_ownership_for(index, element)
                } else {
                    op.param_ownership(index)
                }
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn children<'a>(
    expression: &'a E,
    pending: &mut Scratch<(&'a E, bool)>,
    in_loop: bool,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let start = pending.as_slice().len();
    if let K::Closure { captures, .. } = &expression.kind {
        for capture in captures {
            admission.units(1)?;
            let child = &capture.value;
            pending.push((child, in_loop), admission)?;
        }
    } else {
        let mut cursor = crate::hir::lexical_exit::ChildCursor::new(expression);
        loop {
            admission.units(1)?;
            let Some(child) = cursor.next() else {
                break;
            };
            pending.push((child, in_loop), admission)?;
        }
    }
    pending.reverse_suffix(start, admission)
}
pub(super) fn collect<'a>(
    program: &P,
    function: &'a F,
    output: &mut Scratch<Entry<'a>>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let mut mutable = Scratch::new(admission)?;
    let mut pending = Scratch::new(admission)?;
    pending.push((&function.body, false), admission)?;
    // The original declaration census includes closures; the candidate pass
    // below prunes them. Do not unify these distinct traversal universes.
    while let Some((expression, _)) = pending.pop(admission)? {
        if let K::Block { statements, .. } | K::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                admission.units(1)?;
                if let S::Let {
                    binding,
                    mutable: true,
                    ..
                } = statement
                {
                    mutable.push(&binding.id, admission)?;
                }
            }
        }
        children(expression, &mut pending, false, admission)?;
    }
    let mut mutable = mutable.into_vec();
    super::sort::heap(&mut mutable, admission, |left, right, admission| {
        leaf::compare(left, right, admission)
    })?;
    pending.push((&function.body, false), admission)?;
    while let Some((expression, in_loop)) = pending.pop(admission)? {
        if matches!(expression.kind, K::Closure { .. }) {
            continue;
        }
        if let K::Block { statements, tail } = &expression.kind {
            pending.push((tail, in_loop), admission)?;
            for statement in statements {
                admission.units(1)?;
                match statement {
                    S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } if in_loop => {
                        if leaf::mutable(&mutable, &binding.id, admission)?
                            && same_cell(program, binding, value, admission)?
                        {
                            Directory::emit(output, Family::Vec, &value.id, binding, admission)?;
                        }
                        pending.push((value, in_loop), admission)?;
                    }
                    S::While {
                        condition, body, ..
                    }
                    | S::LexicalWhile {
                        condition, body, ..
                    } => {
                        let ordinary =
                            iterator_shape::recognize(condition, body, admission)?.is_none();
                        pending.push((body, ordinary), admission)?;
                        pending.push((condition, false), admission)?;
                    }
                    S::Let { value, .. } | S::Assign { value, .. } => {
                        pending.push((value, in_loop), admission)?
                    }
                    _ => {}
                }
            }
        } else {
            children(expression, &mut pending, in_loop, admission)?;
        }
    }
    Ok(())
}
