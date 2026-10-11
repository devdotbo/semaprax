//! Independent typed constructor authentication. No new runtime operation.
use super::{ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType};
mod capture;

pub(crate) struct Expansion<'a> {
    pub(crate) element: &'a ResolvedType,
    pub(crate) count: u32,
    pub(crate) statements: &'a [ResolvedStatement],
    pub(crate) tail: &'a ResolvedExpr,
}
/// The marker occupies the ordinary Block node, not a second owning temporary.
pub(crate) fn expansion(expression: &ResolvedExpr) -> Option<Expansion<'_>> {
    match &expression.kind {
        ResolvedExprKind::VecConstructor {
            element,
            count,
            statements,
            tail,
        } => Some(Expansion {
            element,
            count: *count,
            statements,
            tail,
        }),
        _ => None,
    }
}

/// Independently checks the full typed capacity/push/owner chain. Normal HIR
/// validation subsequently authenticates every child identity, type and call.
pub(crate) fn authenticate(expression: &ResolvedExpr) -> Result<(), crate::diagnostic::Diagnostic> {
    use super::OwnershipMode;
    let Some(expansion) = expansion(expression) else {
        return Ok(());
    };
    let reject = || super::hir_error("resolved Vec constructor expansion disagrees");
    if u64::from(expansion.count) > crate::vec_ops::MAX_CAPACITY
        || expansion.statements.len() != expansion.count as usize + 1
        || expression.ownership != OwnershipMode::Own
        || !matches!(&expression.ty, ResolvedType::Nominal { declaration, arguments }
            if declaration.as_str() == crate::prelude::VEC_ID
                && arguments.as_slice() == std::slice::from_ref(expansion.element))
    {
        return Err(reject());
    }
    let mut previous = None;
    let mut last_number = 0usize;
    for (index, statement) in expansion.statements.iter().enumerate() {
        let ResolvedStatement::Let {
            binding,
            mutable: false,
            value,
            ..
        } = statement
        else {
            return Err(reject());
        };
        let Some(number) = binding
            .name
            .strip_prefix("_if")
            .filter(|name| !name.starts_with('0') && name.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|name| name.parse::<usize>().ok())
        else {
            return Err(reject());
        };
        if number <= last_number
            || binding.ty != expression.ty
            || value.ty != expression.ty
            || binding.ownership != OwnershipMode::Own
            || value.ownership != OwnershipMode::Own
        {
            return Err(reject());
        }
        last_number = number;
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance: None,
            args,
        } = &value.kind
        else {
            return Err(reject());
        };
        if type_arguments.as_slice() != std::slice::from_ref(expansion.element) {
            return Err(reject());
        }
        if index == 0 {
            if callee.as_str() != crate::vec_ops::WITH_CAPACITY_ID
                || !matches!(args.as_slice(),
                [ResolvedExpr { kind: ResolvedExprKind::Usize(n), ty: ResolvedType::Usize,
                    ownership: OwnershipMode::Value, .. }] if *n == u64::from(expansion.count))
            {
                return Err(reject());
            }
        } else if callee.as_str() != crate::vec_ops::PUSH_ID
            || args.len() != 2
            || args[1].ty != *expansion.element
            || !matches!(&args[0].kind, ResolvedExprKind::Place(place)
                if Some(&place.root) == previous && place.projections.is_empty())
        {
            return Err(reject());
        }
        previous = Some(&binding.id);
    }
    if expansion.tail.ty != expression.ty
        || expansion.tail.ownership != OwnershipMode::Own
        || !matches!(&expansion.tail.kind, ResolvedExprKind::Place(place)
            if Some(&place.root) == previous && place.projections.is_empty())
    {
        return Err(reject());
    }
    capture::reject_item_capture(&expansion)?;
    Ok(())
}

pub(crate) fn function_uses(
    function: &super::ResolvedFunction,
) -> Result<bool, crate::diagnostic::Diagnostic> {
    roots_use(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
    )
}
/// Presence follows the existing ordinary semantic-expression depth contract
/// (512), not the cache codec's distinct 256-node serialization contract.
/// It allocates no heap, path, identity, or proof inventory. Full constructor
/// authentication and charged retained inventories remain independent.
fn roots_use<'a>(
    roots: impl Iterator<Item = &'a ResolvedExpr>,
) -> Result<bool, crate::diagnostic::Diagnostic> {
    fn present(
        expression: &ResolvedExpr,
        depth: usize,
        visits: &mut usize,
    ) -> Result<bool, crate::diagnostic::Diagnostic> {
        *visits = visits
            .checked_add(1)
            .filter(|count| *count <= 16 * 1024 * 1024)
            .ok_or_else(|| {
                super::hir_error("Vec constructor presence exceeds ordinary source-work upper")
            })?;
        if depth > 512 {
            return Err(super::hir_error(
                "Vec constructor presence exceeds ordinary semantic depth",
            ));
        }
        if matches!(expression.kind, ResolvedExprKind::VecConstructor { .. }) {
            return Ok(true);
        }
        let mut children = super::lexical_exit::ChildCursor::new(expression);
        while let Some(child) = children.next() {
            if present(child, depth + 1, visits)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    let mut visits = 0usize;
    for root in roots {
        if present(root, 1, &mut visits)? {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(crate) fn program_uses(
    program: &super::ResolvedProgram,
) -> Result<bool, crate::diagnostic::Diagnostic> {
    roots_use(
        program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function))
            .flat_map(|f| {
                f.requires
                    .iter()
                    .chain(std::iter::once(&f.body))
                    .chain(&f.ensures)
            })
            .chain(program.function_templates.iter().flat_map(|f| {
                f.requires
                    .iter()
                    .chain(std::iter::once(&f.body))
                    .chain(&f.ensures)
            })),
    )
}

#[cfg(test)]
mod tests;
