use super::{DEPTH, LIMIT};
use crate::ast::{
    BinaryOp, Expr, ExprKind as K, Function, MatchMode, MatchPattern, RecordMatchFieldPattern as P,
    Statement as S, Type,
};
pub(crate) struct Proof<'a> {
    pub(crate) iterations: u32,
    body: &'a Expr,
}
fn int(e: &Expr, n: i64) -> bool {
    matches!(e.kind, K::Int(v) if v == n)
}
fn var(e: &Expr, name: &str) -> bool {
    matches!(&e.kind, K::Var(v) if v == name)
}
fn fields(fields: &[crate::ast::RecordMatchPatternField], counter: &str, depth: usize) -> bool {
    depth <= DEPTH
        && fields.iter().all(|field| match &field.pattern {
            P::Binding { name, .. } => name != counter,
            P::Wildcard { .. } => true,
            P::Record { fields: nested, .. } => self::fields(nested, counter, depth + 1),
        })
}
/// This pure structural tail census rejects every additional assignment and
/// nested loop/closure; it never infers a step from a callee or guard.
fn tail(e: &Expr, counter: &str, depth: usize) -> bool {
    if depth > DEPTH {
        return false;
    }
    match &e.kind {
        K::Closure { .. }
        | K::Yield { .. }
        | K::VecConstructor { .. }
        | K::MethodCall { .. }
        | K::SuperMethod { .. } => return false,
        K::Block { statements, .. } if !statements.is_empty() => return false,
        K::Match { arms, .. } => {
            if arms.iter().any(|a| {
                a.guard.is_some()
                    || match &a.pattern {
                        MatchPattern::Binding { name, .. } => name == counter,
                        MatchPattern::Variant { fields, .. } => {
                            fields.iter().any(|f| f.binding == counter)
                        }
                        MatchPattern::Record { fields: nested, .. } => {
                            !fields(nested, counter, depth + 1)
                        }
                        MatchPattern::Or { .. } => true,
                        _ => false,
                    }
            }) {
                return false;
            }
        }
        _ => {}
    }
    let mut index = 0;
    while let Some(child) = e.child(index) {
        if !tail(child, counter, depth + 1) {
            return false;
        }
        index += 1;
    }
    true
}
pub(crate) fn inspect<'a>(function: &'a Function, body: &'a Expr) -> Option<Proof<'a>> {
    if !function.params.is_empty()
        || !function.type_parameters.is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || function.return_type != Type::I64
    {
        return None;
    }
    let K::Block { statements, .. } = &function.body.kind else {
        return None;
    };
    let index = statements.iter().position(
        |s| matches!(s, S::While { body: current, .. } if std::ptr::eq(current.as_ref(), body)),
    )?;
    let S::Let {
        name: counter,
        mutable: true,
        declared,
        value: start,
        ..
    } = statements.get(index.checked_sub(1)?)?
    else {
        return None;
    };
    if !int(start, 0) || declared.as_ref().is_some_and(|ty| *ty != Type::I64) {
        return None;
    }
    let S::While { condition, .. } = &statements[index] else {
        return None;
    };
    let K::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return None;
    };
    if !var(left, counter) {
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
        name: record,
        mutable: false,
        value: constructor,
        ..
    }, discard] = locals.as_slice()
    else {
        return None;
    };
    let K::ConstructRecord {
        type_name,
        type_arguments,
        ..
    } = &constructor.kind
    else {
        return None;
    };
    if record == counter
        || !type_arguments.is_empty()
        || !tail(constructor, counter, 0)
        || !int(end, 0)
    {
        return None;
    }
    let value = crate::ast::expression_statement::value(discard)?;
    if !value.contains_lexical_exit() {
        return None;
    }
    let K::Match {
        mode: MatchMode::Own,
        scrutinee,
        arms,
    } = &value.kind
    else {
        return None;
    };
    if !var(scrutinee, record) {
        return None;
    }
    let [arm] = arms.as_slice() else {
        return None;
    };
    let MatchPattern::Record {
        type_name: pattern_name,
        fields: nested,
        ..
    } = &arm.pattern
    else {
        return None;
    };
    if pattern_name != type_name || arm.guard.is_some() || !fields(nested, counter, 0) {
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
        name,
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
    if name != counter || !var(left, counter) || !int(right, 1) || !tail(remaining, counter, 0) {
        return None;
    }
    Some(Proof { iterations, body })
}
pub(crate) fn permits(function: &Function, expression: &Expr) -> bool {
    let K::Block { statements, .. } = &function.body.kind else {
        return false;
    };
    statements.iter().any(|statement| {
        let S::While { body, .. } = statement else {
            return false;
        };
        inspect(function, body).is_some_and(|proof| {
            proof.iterations > 0 && proof.iterations <= LIMIT && contains(proof.body, expression, 0)
        })
    })
}
fn contains(root: &Expr, needle: &Expr, depth: usize) -> bool {
    if depth > DEPTH {
        return false;
    }
    if std::ptr::eq(root, needle) {
        return true;
    }
    let mut index = 0;
    while let Some(child) = root.child(index) {
        if contains(child, needle, depth + 1) {
            return true;
        }
        index += 1;
    }
    false
}
