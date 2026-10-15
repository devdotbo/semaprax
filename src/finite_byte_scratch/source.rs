use super::{Length, DEPTH, LIMIT, WORK};
use crate::ast::{Expr, ExprKind, Function, Statement, Type};

pub(crate) struct SourceScratch<'a> {
    pub(crate) iterations: u32,
    sites: [Option<&'a Expr>; LIMIT],
    names: [Option<&'a str>; LIMIT],
}
impl SourceScratch<'_> {
    pub(crate) fn permits(&self, expression: &Expr) -> bool {
        self.sites
            .iter()
            .flatten()
            .any(|site| std::ptr::eq(*site, expression))
    }
}
/// The initial profile deliberately admits only a direct function-block ForOwn.
/// Unknown providers, branches, mutation and reserve lose the bound, rather than
/// inheriting a type's much larger physical carrier ceiling.
pub(crate) fn source<'a>(function: &'a Function, body: &'a Expr) -> Option<SourceScratch<'a>> {
    let ExprKind::Block { statements, .. } = &function.body.kind else {
        return None;
    };
    let (index, values) =
        statements
            .iter()
            .enumerate()
            .find_map(|(i, statement)| match statement {
                Statement::ForOwn {
                    values,
                    body: candidate,
                    ..
                } if std::ptr::eq(candidate.as_ref(), body) => Some((i, values.as_ref())),
                _ => None,
            })?;
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &values.kind
    else {
        return None;
    };
    if name != "vec_into_iter" || type_arguments != &[Type::Bytes] || args.len() != 1 {
        return None;
    }
    let ExprKind::Block {
        statements: locals, ..
    } = &body.kind
    else {
        return None;
    };
    if !locals
        .iter()
        .any(|s| matches!(s, Statement::Let { value, .. } if zeroed(value)))
    {
        return None;
    }
    if !crate::bounded_output::reserve_active_required(
        std::mem::size_of::<SourceScratch<'_>>()
            + std::mem::size_of::<[Option<(&Expr, usize)>; DEPTH]>(),
    ) {
        return None;
    }
    let mut work = WORK;
    let length = provenance(&args[0], &statements[..index], &mut work)?;
    let mut proof = SourceScratch {
        iterations: length.length,
        sites: [None; LIMIT],
        names: [None; LIMIT],
    };
    let mut count = 0;
    for statement in locals {
        if let Statement::Let {
            name,
            mutable: false,
            value,
            ..
        } = statement
        {
            if zeroed(value) {
                *proof.sites.get_mut(count)? = Some(value);
                proof.names[count] = Some(name.as_str());
                count += 1;
            }
        }
    }
    if count == 0 {
        return None;
    }
    let mut frames = [None; DEPTH];
    frames[0] = Some((body, usize::MAX));
    let mut depth = 1;
    while depth != 0 {
        work = work.checked_sub(1)?;
        let slot = depth - 1;
        let (expression, next) = frames[slot]?;
        if next == usize::MAX {
            frames[slot] = Some((expression, 0));
            match &expression.kind {
                ExprKind::Var(name)
                    if proof.names.iter().flatten().any(|n| *n == name.as_str()) =>
                {
                    return None
                }
                ExprKind::Closure { .. } | ExprKind::Yield { .. } => return None,
                ExprKind::Block { statements, .. } => {
                    work = work.checked_sub(statements.len())?;
                    // A nested loop could repeat an otherwise permitted site.
                    if statements.iter().any(|s| {
                        matches!(
                            s,
                            Statement::While { .. }
                                | Statement::For { .. }
                                | Statement::ForBorrow { .. }
                                | Statement::ForOwn { .. }
                        )
                    }) {
                        return None;
                    }
                }
                ExprKind::Call {
                    name,
                    type_arguments,
                    args,
                } if name == crate::byte_ops::BYTES_AS_SLICE_NAME => {
                    if type_arguments.is_empty()
                        && matches!(args.as_slice(), [arg] if matches!(&arg.kind, ExprKind::Var(name) if proof.names.iter().flatten().any(|n| *n == name.as_str())))
                    {
                        let parent = depth
                            .checked_sub(2)
                            .and_then(|i| frames[i])
                            .map(|(e, _)| e)?;
                        if !matches!(&parent.kind, ExprKind::Call { name, args, .. } if matches!(crate::byte_ops::by_name(name), Some(crate::byte_ops::ByteOp::Len | crate::byte_ops::ByteOp::Get)) && args.first().is_some_and(|a| std::ptr::eq(a, expression)))
                        {
                            return None;
                        }
                        depth -= 1;
                        continue;
                    }
                }
                ExprKind::Call { name, .. }
                    if name == crate::byte_ops::ZEROED_NAME
                        || name == crate::byte_ops::ZEROED_BOUNDED_NAME =>
                {
                    if !proof.permits(expression) {
                        return None;
                    }
                }
                ExprKind::Call { name, .. } if crate::byte_ops::by_name(name).is_none() => {
                    return None
                }
                ExprKind::MethodCall { .. } | ExprKind::SuperMethod { .. } => return None,
                _ => {}
            }
            continue;
        }
        if let Some(child) = expression.child(next) {
            frames[slot] = Some((expression, next + 1));
            *frames.get_mut(depth)? = Some((child, usize::MAX));
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Some(proof)
}
fn zeroed(expression: &Expr) -> bool {
    matches!(&expression.kind, ExprKind::Call { name, type_arguments, args }
        if name == crate::byte_ops::ZEROED_NAME && type_arguments.is_empty()
        && matches!(args.as_slice(), [arg] if matches!(arg.kind, ExprKind::Usize(n) if n <= crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES)))
}
fn provenance<'a>(
    mut expression: &'a Expr,
    mut statements: &'a [Statement],
    work: &mut usize,
) -> Option<Length> {
    let mut pushes = 0u32;
    loop {
        *work = work.checked_sub(1)?;
        match &expression.kind {
            ExprKind::Var(name) => {
                *work = work.checked_sub(statements.len())?;
                let (i, statement)=statements.iter().enumerate().rev().find(|(_,s)|matches!(s, Statement::Let {name:candidate,..}|Statement::Assign {name:candidate,..} if candidate==name))?;
                let Statement::Let {
                    mutable: false,
                    value,
                    ..
                } = statement
                else {
                    return None;
                };
                expression = value;
                statements = &statements[..i];
            }
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } if type_arguments == &[Type::Bytes] => {
                if name == "vec_with_capacity" {
                    return match args.as_slice() {
                        [arg] => match arg.kind {
                            ExprKind::Usize(capacity)
                                if capacity <= LIMIT as u64 && u64::from(pushes) <= capacity =>
                            {
                                Some(Length { length: pushes })
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                }
                if name != "vec_push" || args.len() != 2 {
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
