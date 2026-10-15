use super::{DEPTH, LIMIT};
use crate::ast::BinaryOp;
use crate::hir::{
    OwnershipMode as O, ResolvedExpr as E, ResolvedExprKind as K, ResolvedMatchMode,
    ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern as P,
    ResolvedStatement as S, ResolvedType as T, ValueId,
};
fn int(e: &E, n: i64) -> bool {
    e.ty == T::I64 && e.ownership == O::Value && matches!(e.kind, K::Int(v) if v == n)
}
fn place(e: &E, id: &ValueId) -> bool {
    e.ty == T::I64
        && e.ownership == O::Value
        && matches!(&e.kind, K::Place(p) if p.root == *id && p.projections.is_empty())
}
fn fields(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
    counter: &crate::hir::ResolvedBinding,
    depth: usize,
) -> bool {
    depth <= DEPTH
        && fields.iter().all(|f| match &f.pattern {
            P::Binding(b) => b.id != counter.id && b.name != counter.name,
            P::Wildcard => true,
            P::Record { fields: nested, .. } => self::fields(nested, counter, depth + 1),
        })
}
fn tail(e: &E, counter: &crate::hir::ResolvedBinding, depth: usize) -> bool {
    if depth > DEPTH {
        return false;
    }
    match &e.kind {
        K::Closure { .. } | K::Yield { .. } | K::VecConstructor { .. } => return false,
        K::Block { statements, .. } if !statements.is_empty() => return false,
        K::Match { arms, .. }
            if arms.iter().any(|a| {
                a.guard.is_some()
                    || match &a.pattern {
                        ResolvedMatchPattern::Binding(b) => {
                            b.id == counter.id || b.name == counter.name
                        }
                        ResolvedMatchPattern::Variant { fields, .. } => fields
                            .iter()
                            .any(|f| f.binding.id == counter.id || f.binding.name == counter.name),
                        ResolvedMatchPattern::Record { fields: nested, .. } => {
                            !fields(nested, counter, depth + 1)
                        }
                        ResolvedMatchPattern::Or(_) => true,
                        _ => false,
                    }
            }) =>
        {
            return false;
        }
        _ => {}
    }
    for child in crate::hir::lexical_exit::ChildCursor::new(e) {
        if !tail(child, counter, depth + 1) {
            return false;
        }
    }
    true
}
pub(crate) fn inspect(program: &ResolvedProgram, condition: &E, body: &E) -> Option<u32> {
    let function = program.functions.iter().find(|f| {
        matches!(&f.body.kind, K::Block { statements, .. } if statements.iter().any(|s| matches!(s, S::While { condition: c, body: b, .. } | S::LexicalWhile { condition: c, body: b, .. } if std::ptr::eq(c.as_ref(), condition) && std::ptr::eq(b.as_ref(), body))))
    })?;
    if !function.params.is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || function.return_type != T::I64
    {
        return None;
    }
    let K::Block { statements, .. } = &function.body.kind else {
        return None;
    };
    let index = statements.iter().position(|s| matches!(s, S::While { condition: c, body: b, .. } | S::LexicalWhile { condition: c, body: b, .. } if std::ptr::eq(c.as_ref(), condition) && std::ptr::eq(b.as_ref(), body)))?;
    let S::Let {
        binding: counter,
        mutable: true,
        value: start,
        ..
    } = statements.get(index.checked_sub(1)?)?
    else {
        return None;
    };
    if counter.ty != T::I64
        || counter.ownership != O::Value
        || !int(start, 0)
        || condition.ty != T::Bool
        || condition.ownership != O::Value
    {
        return None;
    }
    let K::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return None;
    };
    if !place(left, &counter.id) || right.ty != T::I64 || right.ownership != O::Value {
        return None;
    }
    let K::Int(bound) = right.kind else {
        return None;
    };
    let iterations = u32::try_from(bound)
        .ok()
        .filter(|n| *n > 0 && *n <= LIMIT)?;
    let K::Block {
        statements: locals,
        tail: end,
    } = &body.kind
    else {
        return None;
    };
    let [S::Let {
        binding: record,
        mutable: false,
        value: constructor,
        ..
    }, S::CopyDiscard { value, .. }] = locals.as_slice()
    else {
        return None;
    };
    if record.id == counter.id
        || record.ownership != O::Own
        || constructor.ownership != O::Own
        || record.ty != constructor.ty
        || !matches!(constructor.kind, K::ConstructRecord { .. })
        || !tail(constructor, counter, 0)
        || !int(end, 0)
    {
        return None;
    }
    let T::Nominal {
        declaration,
        arguments,
    } = &record.ty
    else {
        return None;
    };
    if !matches!(&constructor.kind, K::ConstructRecord { record, .. } if record == declaration) {
        return None;
    }
    if !arguments.is_empty()
        || !program.types.iter().any(|d| {
            d.id == *declaration
                && d.type_parameters.is_empty()
                && matches!(
                    d.kind,
                    crate::hir::ResolvedTypeDeclarationKind::Record { .. }
                )
        })
    {
        return None;
    }
    let K::Match {
        mode: ResolvedMatchMode::Own,
        scrutinee,
        arms,
    } = &value.kind
    else {
        return None;
    };
    if scrutinee.ty != record.ty
        || scrutinee.ownership != O::Own
        || !matches!(&scrutinee.kind, K::Place(p) if p.root == record.id && p.projections.is_empty())
    {
        return None;
    }
    let [arm] = arms.as_slice() else {
        return None;
    };
    let ResolvedMatchPattern::Record {
        record: pattern_record,
        instance,
        fields: nested,
    } = &arm.pattern
    else {
        return None;
    };
    if pattern_record != declaration
        || instance != &record.ty
        || arm.guard.is_some()
        || !fields(nested, counter, 0)
    {
        return None;
    }
    let K::Block {
        statements: arm_statements,
        tail: remaining,
    } = &arm.value.kind
    else {
        return None;
    };
    let [S::Assign {
        binding,
        field: None,
        value: step,
        ..
    }] = arm_statements.as_slice()
    else {
        return None;
    };
    let K::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &step.kind
    else {
        return None;
    };
    // The assignment target carries its own use-site span, so whole-binding
    // equality can never hold; compare the identity facts the let established.
    if binding.id != counter.id
        || binding.name != counter.name
        || binding.ty != counter.ty
        || binding.ownership != counter.ownership
        || !place(left, &counter.id)
        || !int(right, 1)
        || step.ty != T::I64
        || step.ownership != O::Value
        || !tail(remaining, counter, 0)
    {
        return None;
    }
    Some(iterations)
}
