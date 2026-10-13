//! Funded borrowed traversal for declaration-only collection refusal.
//! The complete owned `contains` and `validate_function` stay in the parent module.
use super::*;
pub(crate) mod early;

fn fund(bytes: Option<usize>) -> Result<(), Diagnostic> {
    match bytes {
        Some(bytes) if crate::bounded_output::reserve_active_required(bytes) => Ok(()),
        Some(_) => Err(budget()),
        None => {
            crate::bounded_output::refuse_active_required(usize::MAX);
            Err(budget())
        }
    }
}
fn budget() -> Diagnostic {
    hir_error_static("declaration-only collection scan exceeds the active validation budget")
}
// Vec growth is paid at its full new size before that allocation exists.
fn reserve<T>(items: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
    fund(Some(0))?;
    let Some(needed) = items.len().checked_add(additional) else {
        return fund(None);
    };
    if needed > items.capacity() {
        let Some(capacity) = items
            .capacity()
            .checked_mul(2)
            .map(|capacity| capacity.max(needed).max(4))
        else {
            return fund(None);
        };
        fund(capacity.checked_mul(std::mem::size_of::<T>()))?;
        if items.try_reserve_exact(capacity - items.len()).is_err() || items.capacity() != capacity
        {
            crate::bounded_output::refuse_active_required(usize::MAX);
            return Err(budget());
        }
    }
    Ok(())
}
fn push<'a>(items: &mut Vec<&'a ResolvedType>, value: &'a ResolvedType) -> Result<(), Diagnostic> {
    reserve(items, 1)?;
    items.push(value);
    Ok(())
}
// A node is paid at the tree upper bound before the insertion that may allocate it.
fn insert<'a>(
    seen: &mut BTreeSet<&'a ResolvedType>,
    ty: &'a ResolvedType,
) -> Result<bool, Diagnostic> {
    if seen.contains(&ty) {
        return Ok(false);
    }
    fund(crate::cleanup_plan::replay_btree_node_upper::<
        &'a ResolvedType,
        (),
    >())?;
    Ok(seen.insert(ty))
}
// Expanding a type with no children adds nothing, so it needs no `seen` entry.
fn has_children(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Function { .. } => true,
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            !arguments.is_empty()
                || index
                    .record_fields(declaration)
                    .is_some_and(|fields| !fields.is_empty())
                || index
                    .variant_cases(declaration)
                    .is_some_and(|cases| cases.iter().any(|case| !case.fields.is_empty()))
        }
        _ => false,
    }
}
pub(crate) fn contains_funded<'a>(
    index: &'a DeclarationIndex,
    root: &'a ResolvedType,
) -> Result<bool, Diagnostic> {
    fund(Some(0))?;
    // The early gates are those of the owned receiver, in the same order.
    if early::runtime_admitted(index, root)?
        || early::record_admitted(root, index)?
        || crate::hir::owned_leaf_collection::is_vec(index, root)
        || matches!(root, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == crate::prelude::VEC_ID)
        || !matches!(
            root,
            ResolvedType::Nominal { .. } | ResolvedType::Function { .. }
        )
    {
        return Ok(false);
    }
    let record_root = matches!(root, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty() && index.declaration(declaration).is_some_and(|d|
            d.kind == DeclarationKind::Record && d.identity_origin != IdentityOrigin::CompilerOwned));
    let mut pending: Vec<&'a ResolvedType> = Vec::new();
    let mut seen: BTreeSet<&'a ResolvedType> = BTreeSet::new();
    let mut inline = Some(root);
    while let Some(ty) = inline.take().or_else(|| pending.pop()) {
        if super::vector(index, ty)
            || (record_root
                && matches!(ty, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == crate::prelude::VEC_ID))
        {
            return Ok(true);
        }
        if !matches!(
            ty,
            ResolvedType::Nominal { .. } | ResolvedType::Function { .. }
        ) || !has_children(index, ty)
            || !insert(&mut seen, ty)?
        {
            continue;
        }
        match ty {
            ResolvedType::Function { parameters, result } => {
                for parameter in parameters {
                    push(&mut pending, parameter)?;
                }
                push(&mut pending, result)?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if let Some(fields) = index.record_fields(declaration) {
                    for field in fields {
                        push(&mut pending, &field.ty)?;
                    }
                }
                if let Some(cases) = index.variant_cases(declaration) {
                    for field in cases.iter().flat_map(|case| &case.fields) {
                        push(&mut pending, &field.ty)?;
                    }
                }
                for argument in arguments {
                    push(&mut pending, argument)?;
                }
            }
            _ => {}
        }
    }
    Ok(false)
}
// Direct sub-expressions only; mirrors the complete preorder of `try_visit_resolved_calls`.
fn child_upper(expression: &ResolvedExpr) -> usize {
    match &expression.kind {
        ResolvedExprKind::ByteRange { .. } => 3,
        ResolvedExprKind::Closure { captures, .. } => captures.len(),
        ResolvedExprKind::Invoke { args, .. } => 1 + args.len(),
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => args.len(),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.len(),
        ResolvedExprKind::HostCommandCall(call) => call.args.len(),
        ResolvedExprKind::LexicalExit { value, .. } => usize::from(value.is_some()),
        ResolvedExprKind::Unary { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::Yield { .. } => 1,
        ResolvedExprKind::Binary { .. } => 2,
        ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } => {
            1 + statements
                .iter()
                .map(|statement| statement.child_count())
                .sum::<usize>()
        }
        ResolvedExprKind::If { .. } => 3,
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields.len(),
        ResolvedExprKind::Match { arms, .. } => {
            1 + arms
                .iter()
                .map(|arm| 1 + usize::from(arm.guard.is_some()))
                .sum::<usize>()
        }
        ResolvedExprKind::UpdateRecord { fields, .. } => 1 + fields.len(),
        ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::BorrowedUtf8Literal(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => 0,
    }
}
pub(crate) fn validate_function(
    index: &DeclarationIndex,
    f: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    fund(Some(0))?;
    let refuses = |ty: &ResolvedType| contains_funded(index, ty);
    let refusal = || {
        hir_error_static(
            "declaration-only text collection cannot occur in an executable signature or body",
        )
    };
    if refuses(&f.return_type)? {
        return Err(refusal());
    }
    for parameter in &f.params {
        if refuses(&parameter.ty)? {
            return Err(refusal());
        }
    }
    if let Some(yields) = &f.yields {
        if refuses(&yields.request_type)? || refuses(&yields.response_type)? {
            return Err(refusal());
        }
    }
    let mut roots = f
        .ensures
        .iter()
        .rev()
        .chain(f.requires.iter().rev())
        .chain(std::iter::once(&f.body));
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    while let Some(expression) = pending.pop().or_else(|| roots.next()) {
        if refuses(&expression.ty)? {
            return Err(refusal());
        }
        // Funding precedes the helper's pushes; it adds at most the direct children.
        reserve(&mut pending, child_upper(expression))?;
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(())
}
