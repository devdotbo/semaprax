use super::{Length, DEPTH, LIMIT, WORK};
use crate::hir::{
    OwnershipMode, ResolvedExpr, ResolvedExprKind as K, ResolvedFunction, ResolvedProgram,
    ResolvedStatement as S, ResolvedType, ValueId,
};
pub(crate) struct ResolvedScratch<'a> {
    pub(crate) iterations: u32,
    sites: [Option<&'a ResolvedExpr>; LIMIT],
    roots: [Option<&'a ValueId>; LIMIT],
}
impl ResolvedScratch<'_> {
    pub(crate) fn sites(&self) -> impl Iterator<Item = &ResolvedExpr> {
        self.sites.iter().flatten().copied()
    }
    pub(crate) fn borrowed_root(&self, root: &ValueId) -> bool {
        self.roots.iter().flatten().any(|id| **id == *root)
    }
    pub(crate) fn permits(&self, expression: &ResolvedExpr) -> bool {
        self.sites
            .iter()
            .flatten()
            .any(|site| std::ptr::eq(*site, expression))
    }
}
pub(crate) fn resolved<'a>(
    program: &'a ResolvedProgram,
    condition: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
) -> Option<ResolvedScratch<'a>> {
    prove(
        program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function)),
        condition,
        body,
    )
}
pub(crate) fn resolved_function<'a>(
    function: &'a ResolvedFunction,
    condition: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
) -> Option<ResolvedScratch<'a>> {
    prove(std::iter::once(function), condition, body)
}
fn prove<'a>(
    functions: impl Iterator<Item = &'a ResolvedFunction>,
    condition: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
) -> Option<ResolvedScratch<'a>> {
    let protocol = crate::hir::iterator_loop::recognize(condition, body)?;
    if protocol.owned_item?.ty != ResolvedType::Bytes {
        return None;
    }
    let K::Block {
        statements: locals, ..
    } = &protocol.authored_body.kind
    else {
        return None;
    };
    if !locals
        .iter()
        .any(|s| matches!(s, S::Let { value, .. } if zeroed(value)))
    {
        return None;
    }
    if !crate::bounded_output::reserve_active_required(
        std::mem::size_of::<ResolvedScratch<'_>>()
            + std::mem::size_of::<[Option<(crate::hir::lexical_exit::ChildCursor<'_>, bool)>; DEPTH]>(
            ),
    ) {
        return None;
    }
    let mut work = WORK;
    let mut length = None;
    for function in functions {
        work = work.checked_sub(1)?;
        if let Some(bound) = wrapper_bound(function, condition, body, &mut work) {
            if length.replace(bound).is_some() {
                return None;
            }
        }
    }
    let mut proof = ResolvedScratch {
        iterations: length?.length,
        sites: [None; LIMIT],
        roots: [None; LIMIT],
    };
    let mut count = 0;
    for statement in locals {
        if let S::Let {
            binding,
            mutable: false,
            value,
            ..
        } = statement
        {
            if zeroed(value)
                && binding.ty == ResolvedType::Bytes
                && binding.ownership == OwnershipMode::Own
            {
                *proof.sites.get_mut(count)? = Some(value);
                proof.roots[count] = Some(&binding.id);
                count += 1;
            }
        }
    }
    if count == 0 {
        return None;
    }
    let mut frames = [None; DEPTH];
    frames[0] = Some((
        crate::hir::lexical_exit::ChildCursor::new(protocol.authored_body),
        false,
    ));
    let mut depth = 1;
    while depth != 0 {
        work = work.checked_sub(1)?;
        let slot = depth - 1;
        let (mut cursor, entered) = frames[slot]?;
        let expression = cursor.expression();
        if !entered {
            frames[slot] = Some((cursor, true));
            match &expression.kind {
                K::BorrowPlace { operation, place }
                    if operation.as_str() == crate::byte_ops::BYTES_AS_SLICE_ID
                        && place.projections.is_empty()
                        && expression.ty == ResolvedType::SliceU8
                        && expression.ownership == OwnershipMode::Borrow
                        && proof.roots.iter().flatten().any(|id| **id == place.root) =>
                {
                    let parent = depth
                        .checked_sub(2)
                        .and_then(|i| frames[i])
                        .map(|(cursor, _)| cursor.expression())?;
                    if !matches!(&parent.kind, K::Call { callee, args, instance: None, .. } if matches!(crate::byte_ops::by_id(callee.as_str()), Some(crate::byte_ops::ByteOp::Len | crate::byte_ops::ByteOp::Get)) && args.first().is_some_and(|a| std::ptr::eq(a, expression)))
                    {
                        return None;
                    }
                    depth -= 1;
                    continue;
                }
                K::Place(place) | K::BorrowPlace { place, .. }
                    if proof.roots.iter().flatten().any(|id| **id == place.root) =>
                {
                    return None
                }
                K::Closure { .. } | K::Yield { .. } => return None,
                K::Block { statements, .. } | K::VecConstructor { statements, .. } => {
                    work = work.checked_sub(statements.len())?;
                    if statements
                        .iter()
                        .any(|s| matches!(s, S::While { .. } | S::LexicalWhile { .. }))
                    {
                        return None;
                    }
                }
                K::Call {
                    callee,
                    type_arguments,
                    instance: None,
                    args,
                } if callee.as_str() == crate::byte_ops::BYTES_AS_SLICE_ID => {
                    if type_arguments.is_empty()
                        && expression.ty == ResolvedType::SliceU8
                        && expression.ownership == OwnershipMode::Borrow
                        && matches!(args.as_slice(), [arg] if arg.ty == ResolvedType::Bytes && arg.ownership == OwnershipMode::Borrow && matches!(&arg.kind, K::Place(place) if place.projections.is_empty() && proof.roots.iter().flatten().any(|id| **id == place.root)))
                    {
                        depth -= 1;
                        continue;
                    }
                }
                K::Call { callee, .. }
                    if callee.as_str() == crate::byte_ops::ZEROED_ID
                        || callee.as_str() == crate::byte_ops::ZEROED_BOUNDED_ID =>
                {
                    if !proof.permits(expression) {
                        return None;
                    }
                }
                K::Call { callee, .. } if crate::byte_ops::by_id(callee.as_str()).is_none() => {
                    return None
                }
                K::Invoke { .. } | K::NativeRustImportCall(_) | K::HostCommandCall(_) => {
                    return None
                }
                _ => {}
            }
            continue;
        }
        if let Some(child) = cursor.next() {
            frames[slot] = Some((cursor, true));
            *frames.get_mut(depth)? =
                Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Some(proof)
}
fn wrapper_bound(
    function: &ResolvedFunction,
    condition: &ResolvedExpr,
    body: &ResolvedExpr,
    work: &mut usize,
) -> Option<Length> {
    let K::Block { statements, .. } = &function.body.kind else {
        return None;
    };
    for (index, statement) in statements.iter().enumerate() {
        *work = work.checked_sub(1)?;
        let S::Let { value: wrapper, .. } = statement else {
            continue;
        };
        let K::Block {
            statements: inner,
            tail,
        } = &wrapper.kind
        else {
            continue;
        };
        let [S::Let {
            binding: step,
            mutable: true,
            value: seed,
            ..
        }, loop_statement] = inner.as_slice()
        else {
            continue;
        };
        let (actual_condition, actual_body) = match loop_statement {
            S::While {
                condition, body, ..
            } => (condition.as_ref(), body.as_ref()),
            S::LexicalWhile {
                condition,
                body,
                latch,
                ..
            } => {
                if !matches!(latch, crate::hir::LoopLatchKind::OwnedIteratorStep { .. })
                    || crate::hir::lexical_exit::validate_latch(condition, body, latch).is_err()
                {
                    continue;
                }
                (condition.as_ref(), body.as_ref())
            }
            _ => continue,
        };
        if !std::ptr::eq(actual_condition, condition) || !std::ptr::eq(actual_body, body) {
            continue;
        }
        let protocol = crate::hir::iterator_loop::recognize(condition, body)?;
        if step != protocol.step
            || tail.ownership != OwnershipMode::Value
            || !matches!(tail.kind, K::Int(0))
        {
            return None;
        }
        let K::Call {
            callee,
            type_arguments,
            instance: None,
            args,
        } = &seed.kind
        else {
            return None;
        };
        if callee.as_str() != crate::iterator_ops::NEXT_ID
            || type_arguments != &[ResolvedType::Bytes]
            || args.len() != 1
            || seed.ty != step.ty
            || seed.ownership != OwnershipMode::Own
        {
            return None;
        }
        let K::Call {
            callee,
            type_arguments,
            instance: None,
            args,
        } = &args[0].kind
        else {
            return None;
        };
        if callee.as_str() != crate::iterator_ops::INTO_ITER_ID
            || type_arguments != &[ResolvedType::Bytes]
            || args.len() != 1
        {
            return None;
        }
        return provenance(&args[0], &statements[..index], work);
    }
    None
}
fn zeroed(expression: &ResolvedExpr) -> bool {
    expression.ty == ResolvedType::Bytes
        && expression.ownership == OwnershipMode::Own
        && matches!(&expression.kind, K::Call { callee, type_arguments, instance: None, args }
            if callee.as_str() == crate::byte_ops::ZEROED_ID && type_arguments.is_empty()
            && matches!(args.as_slice(), [arg] if arg.ty == ResolvedType::Usize && arg.ownership == OwnershipMode::Value && matches!(arg.kind, K::Usize(n) if n <= crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES)))
}
fn provenance<'a>(
    mut expression: &'a ResolvedExpr,
    mut statements: &'a [S],
    work: &mut usize,
) -> Option<Length> {
    let mut pushes = 0u32;
    loop {
        *work = work.checked_sub(1)?;
        if !matches!(&expression.ty, ResolvedType::Nominal { declaration, arguments } if declaration.as_str() == crate::prelude::VEC_ID && arguments.as_slice() == [ResolvedType::Bytes])
            || expression.ownership != OwnershipMode::Own
        {
            return None;
        }
        match &expression.kind {
            K::Place(place) if place.projections.is_empty() => {
                *work = work.checked_sub(statements.len())?;
                let (i,statement)=statements.iter().enumerate().rev().find(|(_,s)|matches!(s,S::Let {binding,..}|S::Assign {binding,..} if binding.id==place.root))?;
                let S::Let {
                    binding,
                    mutable: false,
                    value,
                    ..
                } = statement
                else {
                    return None;
                };
                if binding.ty != expression.ty || binding.ownership != OwnershipMode::Own {
                    return None;
                }
                expression = value;
                statements = &statements[..i];
            }
            K::Call {
                callee,
                type_arguments,
                instance: None,
                args,
            } if type_arguments == &[ResolvedType::Bytes] => {
                if callee.as_str() == crate::vec_ops::WITH_CAPACITY_ID {
                    return match args.as_slice() {
                        [arg] => match arg.kind {
                            K::Usize(capacity)
                                if arg.ty == ResolvedType::Usize
                                    && arg.ownership == OwnershipMode::Value
                                    && capacity <= LIMIT as u64
                                    && u64::from(pushes) <= capacity =>
                            {
                                Some(Length { length: pushes })
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                }
                if callee.as_str() != crate::vec_ops::PUSH_ID
                    || args.len() != 2
                    || args[1].ty != ResolvedType::Bytes
                    || args[1].ownership != OwnershipMode::Own
                {
                    return None;
                }
                pushes = pushes.checked_add(1)?;
                if pushes > LIMIT as u32 {
                    return None;
                }
                expression = &args[0];
            }
            _ => return None,
        }
    }
}
