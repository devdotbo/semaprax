//! Independently bind the retained source form at each authored field site.
//! No retained origin flag can manufacture an authored call or row projection.
use super::*;
pub(super) struct Site<'a> {
    pub(super) function: &'a Function,
    pub(super) path: String,
    pub(super) bytes: bool,
    pub(super) declaration: &'a TypeDeclaration,
    pub(super) field: &'a crate::ast::FieldDeclaration,
}
pub(super) fn explicit(expression: &Expr) -> Option<(&[Expr], bool)> {
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return None;
    };
    if name == crate::vec_field::NAME && type_arguments.len() == 1 && args.len() == 3 {
        return Some((args, false));
    }
    if name == "str_as_bytes" && type_arguments.is_empty() && args.len() == 1 {
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &args[0].kind
        else {
            return None;
        };
        if name == crate::vec_field::NAME && type_arguments.len() == 1 && args.len() == 3 {
            return Some((args, true));
        }
    }
    None
}
pub(super) fn push<'a>(
    out: &mut Vec<Site<'a>>,
    function: &'a Function,
    path: &str,
    program: &'a Program,
    programs: &'a [Program],
    expression: &'a Expr,
    bytes: bool,
) -> Result<()> {
    let call = if bytes {
        let ExprKind::Call { args, .. } = &expression.kind else {
            return Err(refusal());
        };
        &args[0]
    } else {
        expression
    };
    let ExprKind::Call {
        type_arguments,
        args,
        ..
    } = &call.kind
    else {
        return Err(refusal());
    };
    let [Type::Named { name, arguments }] = type_arguments.as_slice() else {
        return Err(refusal());
    };
    if !arguments.is_empty() {
        return Err(refusal());
    }
    let declaration = declaration(program, programs, name).ok_or_else(refusal)?;
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return Err(refusal());
    };
    let Some(Expr {
        kind: ExprKind::String(selector),
        ..
    }) = args.get(2)
    else {
        return Err(refusal());
    };
    let field = fields
        .iter()
        .find(|field| field.name == *selector)
        .ok_or_else(refusal)?;
    let paid_bytes = 4usize
        .checked_mul(std::mem::size_of::<Site<'_>>())
        .and_then(|b| b.checked_add(path.len()))
        .ok_or_else(refusal)?;
    reserve_builder_structure(paid_bytes)?;
    out.push(Site {
        function,
        path: crate::bounded_output::budgeted_clone(path),
        bytes,
        declaration,
        field,
    });
    Ok(())
}
pub(super) fn validate(
    expected: &Expected<'_>,
    function: &Function,
    expression: &hir::ResolvedExpr,
) -> Result<usize> {
    let hir::ResolvedExprKind::VecFieldRead {
        origin,
        element,
        field,
        bytes,
        args,
    } = &expression.kind
    else {
        return Ok(0);
    };
    let id = |owner: &Function, path: &str| {
        owner.stable_id == function.stable_id
            && expression.id == hir::ExpressionId::new(&execution(function), path)
    };
    match origin {
        hir::VecFieldReadOrigin::Explicit => {
            let site = expected
                .explicit
                .iter()
                .find(|site| id(site.function, &site.path))
                .ok_or_else(refusal)?;
            if *bytes != site.bytes
                || args.len() != 2
                || field.as_str() != site.field.stable_id
                || !element_matches(element, site.declaration)
            {
                return Err(refusal());
            }
            Ok(1)
        }
        hir::VecFieldReadOrigin::Traversal => {
            if expected
                .reads
                .iter()
                .any(|read| !read.copy && id(read.function, &read.path))
            {
                Ok(0)
            } else {
                Err(refusal())
            }
        }
    }
}

// Heap growth is paid before construction; no new depth cap is imposed on
// legacy functions. The existing shared loan-work bound closes this walk.
pub(super) fn refuse_orphan(root: &hir::ResolvedExpr) -> Result<()> {
    fn push<'a>(
        pending: &mut Vec<&'a hir::ResolvedExpr>,
        value: &'a hir::ResolvedExpr,
    ) -> Result<()> {
        if pending.len() == pending.capacity() {
            let additional = pending.capacity().max(1);
            reserve_builder_structure(
                additional
                    .checked_mul(4)
                    .and_then(|n| n.checked_mul(std::mem::size_of::<&hir::ResolvedExpr>()))
                    .ok_or_else(refusal)?,
            )?;
            let old = pending.capacity();
            pending
                .try_reserve_exact(additional)
                .map_err(|_| refusal())?;
            let expected = old.checked_add(additional).ok_or_else(refusal)?;
            if pending.capacity()
                > old
                    .checked_add(additional.checked_mul(4).ok_or_else(refusal)?)
                    .ok_or_else(refusal)?
            {
                return Err(refusal());
            }
            if pending.capacity() < expected {
                return Err(refusal());
            }
        }
        pending.push(value);
        Ok(())
    }
    let mut pending = Vec::new();
    let mut work = 0usize;
    push(&mut pending, root)?;
    while let Some(expression) = pending.pop() {
        work = work.checked_add(1).ok_or_else(refusal)?;
        if work > crate::loan_plan::MAX_LOAN_PLAN_WORK_V1 {
            return Err(refusal());
        }
        if matches!(
            &expression.kind,
            hir::ResolvedExprKind::VecFieldRead {
                origin: hir::VecFieldReadOrigin::Traversal,
                ..
            }
        ) {
            return Err(refusal());
        }
        if let hir::ResolvedExprKind::Closure { body, .. } = &expression.kind {
            push(&mut pending, body)?;
        }
        for child in hir::lexical_exit::ChildCursor::new(expression) {
            push(&mut pending, child)?;
        }
    }
    Ok(())
}
