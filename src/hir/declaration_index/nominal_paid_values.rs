//! Actual byte-owned construction operations for the nominal producer.
//! No ambient account, clone credit, fixed depth limit, or replay work is used.
use super::identity_payer::IdentityIndexError;
use super::nominal_rollback::Workspace;
use super::*;
use std::{fmt, mem::size_of};
pub(super) type Result<T, E> = std::result::Result<T, IdentityIndexError<E>>;
pub(super) type Reserve<'a, E> = dyn FnMut(usize) -> std::result::Result<(), E> + 'a;

pub(super) fn overflow<E>() -> IdentityIndexError<E> {
    IdentityIndexError::Semantic(Diagnostic::io_static(
        "SPX-H006",
        "nominal constructor layout overflowed",
    ))
}
pub(super) fn bytes<E>(reserve: &mut Reserve<'_, E>, n: usize) -> Result<(), E> {
    reserve(n).map_err(IdentityIndexError::Identity)
}
pub(super) fn vector_fee<T, E>(capacity: usize, reserve: &mut Reserve<'_, E>) -> Result<(), E> {
    let backing = capacity.checked_mul(size_of::<T>()).ok_or_else(overflow)?;
    bytes(
        reserve,
        backing
            .checked_add(size_of::<Vec<T>>())
            .and_then(|n| {
                n.checked_add(size_of::<std::result::Result<Vec<T>, IdentityIndexError<E>>>())
            })
            .ok_or_else(overflow)?,
    )
}
pub(super) fn vector<T, E>(capacity: usize, reserve: &mut Reserve<'_, E>) -> Result<Vec<T>, E> {
    vector_fee::<T, E>(capacity, reserve)?;
    let result = Vec::with_capacity(capacity);
    assert!(size_of::<T>() == 0 || result.capacity() == capacity);
    Ok(result)
}
pub(super) fn grow<T, E>(
    values: &mut Vec<T>,
    additional: usize,
    reserve: &mut Reserve<'_, E>,
) -> Result<(), E> {
    let needed = values.len().checked_add(additional).ok_or_else(overflow)?;
    if needed <= values.capacity() {
        return bytes(reserve, 0);
    }
    let cap = needed
        .max(values.capacity().checked_mul(2).ok_or_else(overflow)?)
        .max(4);
    let mut next = vector(cap, reserve)?;
    next.append(values);
    *values = next;
    Ok(())
}
pub(super) fn node<K, V, E>(reserve: &mut Reserve<'_, E>) -> Result<(), E> {
    bytes(
        reserve,
        crate::cleanup_plan::replay_btree_node_upper::<K, V>().ok_or_else(overflow)?,
    )
}
pub(super) fn text<E>(value: &str, reserve: &mut Reserve<'_, E>) -> Result<String, E> {
    let n = value
        .len()
        .checked_add(size_of::<String>())
        .and_then(|n| {
            n.checked_add(size_of::<std::result::Result<String, IdentityIndexError<E>>>())
        })
        .ok_or_else(overflow)?;
    bytes(reserve, n)?;
    let mut out = String::with_capacity(value.len());
    assert_eq!(out.capacity(), value.len());
    out.push_str(value);
    Ok(out)
}
struct Count(usize);
impl fmt::Write for Count {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}
pub(super) fn formatted<E>(
    args: fmt::Arguments<'_>,
    reserve: &mut Reserve<'_, E>,
) -> Result<String, E> {
    let mut count = Count(0);
    fmt::write(&mut count, args).map_err(|_| overflow())?;
    bytes(
        reserve,
        count
            .0
            .checked_add(size_of::<String>())
            .and_then(|n| {
                n.checked_add(size_of::<std::result::Result<String, IdentityIndexError<E>>>())
            })
            .ok_or_else(overflow)?,
    )?;
    let mut out = String::with_capacity(count.0);
    assert_eq!(out.capacity(), count.0);
    fmt::write(&mut out, args).expect("exact paid String formatter");
    assert_eq!(out.len(), count.0);
    Ok(out)
}
pub(super) fn diagnostic<E>(
    code: &'static str,
    args: fmt::Arguments<'_>,
    span: crate::ast::Span,
    path: &str,
    reserve: &mut Reserve<'_, E>,
) -> Result<Diagnostic, E> {
    bytes(
        reserve,
        size_of::<Diagnostic>()
            + size_of::<std::result::Result<Diagnostic, IdentityIndexError<E>>>(),
    )?;
    let message = formatted(args, reserve)?;
    let path = text(path, reserve)?;
    Ok(Diagnostic {
        code,
        severity: crate::diagnostic::Severity::Error,
        message: std::borrow::Cow::Owned(message),
        path: Some(path),
        span: Some(span),
        help: None,
    })
}
impl Workspace {
    pub(super) fn ensure_paid_frontier<E>(
        &self,
        capacity: usize,
        reserve: &mut Reserve<'_, E>,
    ) -> Result<(), E> {
        bytes(reserve, 0)?;
        let scratch = self.scratch.as_ref().expect("paid construction workspace");
        let mut held = scratch.lock().expect("primed construction disposal lock");
        assert!(
            held.is_empty(),
            "only grow the disposal frontier outside Drop"
        );
        if capacity > held.capacity() {
            let replacement = vector::<ResolvedType, E>(capacity, reserve)?;
            *held = replacement;
        }
        Ok(())
    }
}
/// Census follows the exact final-owner disposal order. Each cloned Vec/Box
/// allocation is counted at its source node; the traversal itself is paid.
/// The source is borrowed, so a refused census has no recursive owned rollback.
pub(super) fn clone_type<E>(
    root: &ResolvedType,
    workspace: &Workspace,
    reserve: &mut Reserve<'_, E>,
) -> Result<ResolvedType, E> {
    if !matches!(root, ResolvedType::Nominal { arguments, .. } if !arguments.is_empty())
        && !matches!(root, ResolvedType::Function { .. })
    {
        bytes(
            reserve,
            size_of::<ResolvedType>()
                + size_of::<std::result::Result<ResolvedType, IdentityIndexError<E>>>(),
        )?;
        return Ok(root.clone());
    }
    let mut pending = vector::<&ResolvedType, E>(1, reserve)?;
    pending.push(root);
    let mut payload = 0usize;
    let mut frontier = 0;
    while let Some(ty) = pending.pop() {
        let (children, boxed) = match ty {
            ResolvedType::Nominal { arguments, .. } => (arguments.as_slice(), None),
            ResolvedType::Function { parameters, result } => {
                (parameters.as_slice(), Some(result.as_ref()))
            }
            _ => (&[][..], None),
        };
        payload = payload
            .checked_add(
                children
                    .len()
                    .checked_mul(size_of::<ResolvedType>())
                    .ok_or_else(overflow)?,
            )
            .ok_or_else(overflow)?;
        if boxed.is_some() {
            payload = payload
                .checked_add(size_of::<ResolvedType>())
                .ok_or_else(overflow)?;
        }
        grow(
            &mut pending,
            children.len() + usize::from(boxed.is_some()),
            reserve,
        )?;
        pending.extend(children);
        if let Some(result) = boxed {
            pending.push(result);
        }
        frontier = frontier.max(pending.len());
    }
    workspace.ensure_paid_frontier(frontier, reserve)?;
    bytes(
        reserve,
        payload
            .checked_add(size_of::<ResolvedType>())
            .and_then(|n| {
                n.checked_add(size_of::<
                    std::result::Result<ResolvedType, IdentityIndexError<E>>,
                >())
            })
            .ok_or_else(overflow)?,
    )?;
    Ok(root.clone())
}
pub(super) fn field<E>(
    field: &ResolvedFieldDeclaration,
    workspace: &Workspace,
    reserve: &mut Reserve<'_, E>,
) -> Result<ResolvedFieldDeclaration, E> {
    bytes(
        reserve,
        size_of::<ResolvedFieldDeclaration>()
            + size_of::<std::result::Result<ResolvedFieldDeclaration, IdentityIndexError<E>>>(),
    )?;
    let name = text(&field.name, reserve)?;
    let ty = clone_type(&field.ty, workspace, reserve)?;
    // No subsequent reservation occurs while the complete root is unguarded.
    Ok(ResolvedFieldDeclaration {
        id: field.id.clone(),
        name,
        ty,
        index: field.index,
        span: field.span,
    })
}
