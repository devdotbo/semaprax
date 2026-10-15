//! Current immutable array extents; no retained provenance supplies authority.
use super::{
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement,
    ResolvedType, ValueId,
};
use crate::hir::lexical_exit::ChildCursor;

// Exact extents come from the current immutable function tree, never from a
// decoded provenance row or a retained capacity proof. Unknown shapes retain
// the legacy maximum. Independent whole-HIR validation remains mandatory.
pub(super) fn static_array_slice_length(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    argument: &ResolvedExpr,
) -> Option<u64> {
    if !program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|held| std::ptr::eq(held, function))
    {
        return None;
    }
    let mut held = false;
    if !visit_extent_body(function, |node| {
        held |= std::ptr::eq(node, argument);
    }) || !held
    {
        return None;
    }
    let mut current = argument;
    // A cyclic, overly long or unsupported alias chain cannot improve a bound.
    for _ in 0..32 {
        if current.ty != ResolvedType::SliceU8 {
            return None;
        }
        match &current.kind {
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                let (ty, initializer) = immutable_extent_initializer(function, &place.root)?;
                if ty != &current.ty || initializer.ty != current.ty {
                    return None;
                }
                current = initializer;
            }
            ResolvedExprKind::BorrowPlace { operation, place }
                if operation.as_str() == crate::byte_ops::ARRAY_AS_SLICE_ID
                    && place.projections.is_empty() =>
            {
                let (ty, initializer) = immutable_extent_initializer(function, &place.root)?;
                if ty != &initializer.ty {
                    return None;
                }
                return literal_array_length(initializer);
            }
            ResolvedExprKind::Call {
                callee,
                instance: None,
                args,
                ..
            } if callee.as_str() == crate::byte_ops::ARRAY_AS_SLICE_ID && args.len() == 1 => {
                let array = &args[0];
                if let ResolvedExprKind::Place(place) = &array.kind {
                    if !place.projections.is_empty() {
                        return None;
                    }
                    let (ty, initializer) = immutable_extent_initializer(function, &place.root)?;
                    if ty != &array.ty || ty != &initializer.ty {
                        return None;
                    }
                    return literal_array_length(initializer);
                }
                return literal_array_length(array);
            }
            _ => return None,
        }
    }
    None
}
fn literal_array_length(expression: &ResolvedExpr) -> Option<u64> {
    let actual = match &expression.kind {
        ResolvedExprKind::ArrayU8(values) => u64::try_from(values.len()).ok()?,
        ResolvedExprKind::RepeatArrayU8 { count, .. } => u64::from(*count),
        _ => return None,
    };
    match expression.ty {
        ResolvedType::ArrayU8(length) if u64::from(length) == actual => Some(actual),
        _ => None,
    }
}
fn immutable_extent_initializer<'a>(
    function: &'a ResolvedFunction,
    value: &ValueId,
) -> Option<(&'a ResolvedType, &'a ResolvedExpr)> {
    let mut found = None;
    let mut refused = function
        .params
        .iter()
        .any(|parameter| parameter.id == *value);
    if !visit_extent_body(function, |expression| {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable,
                        value: initializer,
                        ..
                    } if binding.id == *value => {
                        if *mutable || found.is_some() {
                            refused = true;
                        }
                        found = Some((&binding.ty, initializer));
                    }
                    ResolvedStatement::Assign { binding, .. } if binding.id == *value => {
                        refused = true
                    }
                    _ => {}
                }
            }
        }
    }) {
        return None;
    }
    if refused {
        None
    } else {
        found
    }
}
pub(super) fn visit_extent_body<'a>(
    function: &'a ResolvedFunction,
    mut visit: impl FnMut(&'a ResolvedExpr),
) -> bool {
    // No extra heap metadata or allowance: a fixed-depth borrowed cursor walk.
    // Closures are separate functions and cannot lend enclosing binding facts.
    let mut frames = [None; 514];
    frames[0] = Some(ChildCursor::new(&function.body));
    let mut length = 1;
    let mut work = 0usize;
    visit(&function.body);
    while length != 0 {
        let frame = frames[length - 1].as_mut().unwrap();
        if matches!(frame.expression().kind, ResolvedExprKind::Closure { .. }) {
            frames[length - 1] = None;
            length -= 1;
            continue;
        }
        if let Some(child) = frame.next() {
            work += 1;
            if work > 65_536 || length == frames.len() {
                return false;
            }
            visit(child);
            frames[length] = Some(ChildCursor::new(child));
            length += 1;
        } else {
            frames[length - 1] = None;
            length -= 1;
        }
    }
    true
}
