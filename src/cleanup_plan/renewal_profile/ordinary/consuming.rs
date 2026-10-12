//! Exact owned-record consuming additions; no ordinary winner is replaced.
use super::{compare, iterator_shape, vector, Admission, Directory, Entry, Refusal, Scratch};
use crate::hir::lexical_exit::ChildCursor;
use crate::hir::{
    OwnershipMode as M, ResolvedBinding as B, ResolvedExpr as E, ResolvedExprKind as K,
    ResolvedFunction as F, ResolvedProgram as P, ResolvedStatement as S, ResolvedType as T,
};
type Frames<'a> = [Option<ChildCursor<'a>>; crate::cache_codec::MAX_DEPTH + 1];
fn visit<'a>(
    root: &'a E,
    admission: &mut Admission<'_>,
    mut visitor: impl FnMut(&'a E, &mut Admission<'_>) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    admission.reserve(std::mem::size_of::<Frames<'_>>())?;
    admission.units(crate::cache_codec::MAX_DEPTH + 1)?;
    let mut stack: Frames<'a> = [None; crate::cache_codec::MAX_DEPTH + 1];
    stack[0] = Some(ChildCursor::new(root));
    let mut len = 1;
    while len != 0 {
        admission.units(1)?;
        let frame = stack[len - 1]
            .as_mut()
            .expect("occupied consuming directory frame");
        let expression = frame.expression();
        if !frame.is_started() {
            visitor(expression, admission)?;
        }
        let child = if matches!(expression.kind, K::Closure { .. }) {
            None
        } else {
            admission.units(1)?;
            frame.next()
        };
        if let Some(child) = child {
            if len == stack.len() {
                return admission.refuse(Refusal::Capacity);
            }
            stack[len] = Some(ChildCursor::new(child));
            len += 1;
        } else {
            stack[len - 1] = None;
            len -= 1;
        }
    }
    Ok(())
}
fn declares(
    root: &E,
    target: &B,
    mutable: bool,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    let mut found = false;
    visit(root, admission, |expression, admission| {
        if let K::Block { statements, .. } | K::VecConstructor { statements, .. } = &expression.kind
        {
            // Preserve any() within one statement list, even once a different
            // list has already found the binding in the old complete walk.
            for statement in statements {
                admission.units(1)?;
                if let S::Let {
                    binding,
                    mutable: actual,
                    ..
                } = statement
                {
                    if compare::values(&binding.id, &target.id, admission)?
                        && compare::types(&binding.ty, &target.ty, admission)?
                        && binding.ownership == target.ownership
                        && (!mutable || *actual)
                    {
                        found = true;
                        break;
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(found)
}
fn seed(step: &B, source: &E, item: &B, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    if !compare::types(&source.ty, &step.ty, admission)? || source.ownership != M::Own {
        return Ok(false);
    }
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &source.kind
    else {
        return Ok(false);
    };
    if !compare::text(callee.as_str(), crate::iterator_ops::NEXT_ID, admission)? {
        return Ok(false);
    }
    let [argument] = type_arguments.as_slice() else {
        return Ok(false);
    };
    if !compare::types(argument, &item.ty, admission)? {
        return Ok(false);
    }
    let [source] = args.as_slice() else {
        return Ok(false);
    };
    if source.ownership != M::Own {
        return Ok(false);
    }
    let T::Nominal {
        declaration,
        arguments,
    } = &source.ty
    else {
        return Ok(false);
    };
    if !compare::text(
        declaration.as_str(),
        crate::iterator_ops::ITER_ID,
        admission,
    )? {
        return Ok(false);
    }
    let [argument] = arguments.as_slice() else {
        return Ok(false);
    };
    compare::types(argument, &item.ty, admission)
}
pub(super) fn collect<'a>(
    program: &P,
    function: &'a F,
    output: &mut Scratch<Entry<'a>>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let mut candidate = false;
    for declaration in program.declarations.declarations() {
        admission.units(1)?;
        if declaration.kind == crate::hir::DeclarationKind::Record
            && declaration.identity_origin == crate::hir::IdentityOrigin::Explicit
        {
            candidate = true;
            break;
        }
    }
    if !candidate {
        return Ok(());
    }
    visit(&function.body, admission, |expression, admission| {
        let K::Block { statements, tail } = &expression.kind else {
            return Ok(());
        };
        for statement in statements {
            admission.units(1)?;
            let (condition, body) = match statement {
                S::While {
                    condition, body, ..
                }
                | S::LexicalWhile {
                    condition, body, ..
                } => (condition.as_ref(), body.as_ref()),
                _ => continue,
            };
            let Some(protocol) = iterator_shape::recognize(condition, body, admission)? else {
                continue;
            };
            let Some(item) = protocol.owned_item else {
                continue;
            };
            let [S::Let {
                binding: step,
                mutable: true,
                value: source,
                ..
            }, S::While { .. } | S::LexicalWhile { .. }] = statements.as_slice()
            else {
                continue;
            };
            // ResolvedBinding equality includes all of id/name/type/mode.
            if !compare::values(&step.id, &protocol.step.id, admission)?
                || !compare::text(&step.name, &protocol.step.name, admission)?
                || !compare::types(&step.ty, &protocol.step.ty, admission)?
                || step.ownership != protocol.step.ownership
                || step.span != protocol.step.span
                || !(tail.ownership == M::Value && matches!(tail.kind, K::Int(0) | K::Usize(0)))
                || !seed(step, source, item, admission)?
                || item.ownership != M::Own
                || !matches!(&item.ty, T::Nominal {arguments, ..} if arguments.is_empty())
            {
                continue;
            }
            visit(protocol.authored_body, admission, |node, admission| {
                let K::Block { statements, .. } = &node.kind else {
                    return Ok(());
                };
                for statement in statements {
                    admission.units(1)?;
                    let S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    else {
                        continue;
                    };
                    let T::Nominal { arguments, .. } = &binding.ty else {
                        continue;
                    };
                    let [element] = arguments.as_slice() else {
                        continue;
                    };
                    let K::Call { callee, .. } = &value.kind else {
                        continue;
                    };
                    let op = match crate::vec_ops::by_id_with_work(callee.as_str(), &mut |units| {
                        admission.units(units).is_ok()
                    }) {
                        Ok(Some(op)) => op,
                        Ok(None) => continue,
                        Err(()) => return admission.refuse(Refusal::Work),
                    };
                    if op != crate::vec_ops::VecOp::Push
                        || vector::mode(program, op, &item.ty, admission)? != Some(M::Own)
                        || vector::mode(program, op, element, admission)? != Some(M::Own)
                        || !vector::same_cell(program, binding, value, admission)?
                        || !declares(&function.body, binding, true, admission)?
                        || declares(protocol.authored_body, binding, false, admission)?
                    {
                        continue;
                    }
                    Directory::emit_consuming(output, &value.id, binding, admission)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    })
}
