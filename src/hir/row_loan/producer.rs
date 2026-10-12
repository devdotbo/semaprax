//! Actual scoped descriptor storage, reserved before the first allocation.
use super::*;
use std::alloc::Layout;

pub(super) fn cloned_type_bytes(ty: &ResolvedType, depth: usize) -> Option<usize> {
    // Current row producers are a flat nominal, its Vec and a usize index.
    // Refuse an unexpected expanded type before recursively cloning it.
    if depth > 2 {
        return None;
    }
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            let mut bytes = Layout::array::<ResolvedType>(arguments.len())
                .ok()?
                .size()
                .checked_add(declaration.as_str().len())?;
            for argument in arguments {
                bytes = bytes.checked_add(cloned_type_bytes(argument, depth + 1)?)?;
            }
            Some(bytes)
        }
        ResolvedType::Usize => Some(0),
        _ => None,
    }
}
fn identity_length(function: &FunctionExecutionId, path: &str, child: bool) -> Option<usize> {
    let FunctionExecutionId::Monomorphic(owner) = function else {
        return None;
    };
    let path_len = path.len().checked_add(if child { 6 } else { 0 })?;
    let digits = |n: usize| if n == 0 { 1 } else { n.ilog10() as usize + 1 };
    // declaration:<owner length>:<owner>:expression:<path length>:<path>
    12usize
        .checked_add(digits(owner.as_str().len()))?
        .checked_add(1)?
        .checked_add(owner.as_str().len())?
        .checked_add(12)?
        .checked_add(digits(path_len))?
        .checked_add(1)?
        .checked_add(path_len)
}
fn required(
    function: &FunctionExecutionId,
    row: &resolve_for::record::Row,
    path: &str,
) -> Option<usize> {
    Layout::new::<RowLoan>()
        .size()
        .checked_add(Layout::array::<ResolvedExpr>(2).ok()?.size())?
        .checked_add(cloned_type_bytes(&row.element, 0)?.checked_mul(2)?)?
        .checked_add(cloned_type_bytes(&row.source.ty, 0)?)?
        .checked_add(cloned_type_bytes(&row.index.ty, 0)?)?
        .checked_add(identity_length(function, path, false)?)?
        .checked_add(identity_length(function, path, true)?.checked_mul(2)?)
}
fn identity(function: &FunctionExecutionId, path: &str, child: Option<usize>) -> ExpressionId {
    use std::fmt::Write;
    let FunctionExecutionId::Monomorphic(owner) = function else {
        unreachable!()
    };
    let child_len = if child.is_some() { 6 } else { 0 };
    let mut text = String::with_capacity(identity_length(function, path, child.is_some()).unwrap());
    write!(
        &mut text,
        "declaration:{}:{}:expression:{}:{path}",
        owner.as_str().len(),
        owner,
        path.len() + child_len
    )
    .expect("writing into exact identity backing");
    if let Some(child) = child {
        text.push_str(".arg.");
        text.push(if child == 0 { '0' } else { '1' });
    }
    ExpressionId::from_owned(text)
}
pub(super) fn materialize(
    function: &FunctionExecutionId,
    span: Span,
    path: &str,
    row: &resolve_for::record::Row,
) -> Option<ResolvedExpr> {
    let Some(bytes) = required(function, row, path) else {
        let _ = crate::bounded_output::reserve_active_required(usize::MAX);
        return None;
    };
    if !crate::bounded_output::reserve_active_required(bytes) {
        return None;
    }
    let place = |segment: usize, binding: &ResolvedBinding| ResolvedExpr {
        id: identity(function, path, Some(segment)),
        ty: binding.ty.clone(),
        ownership: binding.ownership,
        kind: ResolvedExprKind::Place(Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        }),
        span,
    };
    let id = identity(function, path, None);
    id.owned_allocation_bytes()?;
    let first = place(0, &row.source);
    first.id.owned_allocation_bytes()?;
    let second = place(1, &row.index);
    second.id.owned_allocation_bytes()?;
    // The sentinel is a fixed compiler identity: no backing, nothing debited.
    let callee = DeclarationId::compiler(crate::hir::CompilerDeclaration::RowLoanUnbound);
    Some(ResolvedExpr {
        id,
        ty: row.element.clone(),
        ownership: OwnershipMode::Borrow,
        kind: ResolvedExprKind::RowLoan {
            loan: Box::new(RowLoan {
                version: RowLoanVersion::V1,
                element: row.element.clone(),
                traversal: row.region.clone(),
                bound: row.bound.clone(),
                callee,
                // The unbound callee refuses publication until the exact parameter is installed.
                parameter: row.bound.clone(),
            }),
            args: vec![first, second],
        },
        span,
    })
}
/// Binding uses the existing canonical identity carrier policy, independently
/// of the unbound descriptor debit. No formatted decimal or temporary callee
/// owner is needed; the exact spelling is materialized once.
pub(super) fn bind(callee: &DeclarationId, index: usize) -> Option<(DeclarationId, ValueId)> {
    use std::fmt::Write;
    let digits = |n: usize| if n == 0 { 1 } else { n.ilog10() as usize + 1 };
    let owner = callee.as_str();
    let length = "declaration:"
        .len()
        .checked_add(digits(owner.len()))?
        .checked_add(1)?
        .checked_add(owner.len())?
        .checked_add(":value:param:".len())?
        .checked_add(digits(digits(index)))?
        .checked_add(1)?
        .checked_add(digits(index))?;
    // One exact temporary String and one canonical shared identity payload;
    // use the existing conservative identity-carrier policy, not Arc layout.
    let bytes = length
        .checked_mul(2)?
        .checked_add(owner.len())?
        .checked_add(ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES)?;
    if !crate::bounded_output::reserve_active_required(bytes) {
        return None;
    }
    let mut text = String::with_capacity(length);
    if text.capacity() != length {
        let _ = crate::bounded_output::reserve_active_required(usize::MAX);
        return None;
    }
    write!(
        &mut text,
        "declaration:{}:{}:value:param:{}:{}",
        owner.len(),
        owner,
        digits(index),
        index
    )
    .expect("writing into exact parameter identity");
    Some((callee.clone(), ValueId::new(text)))
}

#[cfg(test)]
mod tests;
