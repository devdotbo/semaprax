//! Exact syntax expansion; this marker grants no ordinary Call authority.
mod cursor;
use super::{Expr, ExprKind, LetSyntax, Statement, Type};
pub(crate) use cursor::{items_fit_depth, NodeCursor};

pub struct Expansion<'a> {
    pub element: &'a Type,
    pub count: u32,
    pub value: &'a Expr,
    pub statements: &'a [Statement],
}
impl<'a> Expansion<'a> {
    pub fn items(&self) -> impl ExactSizeIterator<Item = &'a Expr> {
        self.statements[1..].iter().map(|statement| {
            let Statement::Let { value, .. } = statement else {
                unreachable!()
            };
            let ExprKind::Call { args, .. } = &value.kind else {
                unreachable!()
            };
            &args[1]
        })
    }
}

/// Authenticate structure before traversing the generated ordinary block.
pub fn inspect(expression: &Expr) -> Result<Expansion<'_>, &'static str> {
    let ExprKind::VecConstructor {
        element,
        count,
        value,
    } = &expression.kind
    else {
        return Err("expected Vec constructor provenance");
    };
    if u64::from(*count) > crate::vec_ops::MAX_CAPACITY {
        return Err("Vec constructor count exceeds the existing capacity bound");
    }
    let ExprKind::Block { statements, tail } = &value.kind else {
        return Err("Vec constructor expansion is not a block");
    };
    if statements.len() != *count as usize + 1 {
        return Err("Vec constructor expansion count disagrees");
    }
    let mut previous: Option<&str> = None;
    let mut previous_number = 0usize;
    for (index, statement) in statements.iter().enumerate() {
        let Statement::Let {
            syntax: LetSyntax::Authored,
            name,
            mutable: false,
            declared: None,
            value: call,
            ..
        } = statement
        else {
            return Err("Vec constructor binding shape disagrees");
        };
        if !name.starts_with("_if")
            || name[3..].is_empty()
            || name[3..].starts_with('0')
            || !name[3..].bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("Vec constructor binding is not hygienically minted");
        }
        let number = name[3..]
            .parse::<usize>()
            .map_err(|_| "Vec constructor name overflows")?;
        if number <= previous_number {
            return Err("Vec constructor binding order disagrees");
        }
        previous_number = number;
        let ExprKind::Call {
            name: callee,
            type_arguments,
            args,
        } = &call.kind
        else {
            return Err("Vec constructor step is not an ordinary call");
        };
        if type_arguments.as_slice() != std::slice::from_ref(element) {
            return Err("Vec constructor element type disagrees");
        }
        if index == 0 {
            if callee != "vec_with_capacity"
                || !matches!(args.as_slice(),
                [Expr { kind: ExprKind::Usize(n), .. }] if *n == u64::from(*count))
            {
                return Err("Vec constructor capacity step disagrees");
            }
        } else if callee != "vec_push"
            || args.len() != 2
            || !matches!(&args[0].kind, ExprKind::Var(root) if Some(root.as_str()) == previous)
        {
            return Err("Vec constructor previous owner or push step disagrees");
        }
        previous = Some(name);
    }
    if !matches!(&tail.kind, ExprKind::Var(root) if Some(root.as_str()) == previous) {
        return Err("Vec constructor tail owner disagrees");
    }
    Ok(Expansion {
        element,
        count: *count,
        value,
        statements,
    })
}

/// Exact owned backing of a Type clone, excluding its inline outer Type.
/// The cursor is depth-bounded, independent of argument-list width.
pub(crate) fn type_backing_bytes(root: &Type) -> Option<usize> {
    const DEPTH: usize = 129;
    let mut stack: [Option<(&Type, usize)>; DEPTH] = [None; DEPTH];
    let mut depth = 1usize;
    let mut total = 0usize;
    stack[0] = Some((root, 0));
    while depth != 0 {
        let (ty, child) = stack[depth - 1]?;
        if child == 0 {
            let backing = match ty {
                Type::Named { name, arguments } => name
                    .len()
                    .checked_add(arguments.len().checked_mul(std::mem::size_of::<Type>())?)?,
                Type::Function { parameters, .. } => parameters
                    .len()
                    .checked_mul(std::mem::size_of::<Type>())?
                    .checked_add(std::mem::size_of::<Type>())?,
                _ => 0,
            };
            total = total.checked_add(backing)?;
        }
        let next = match ty {
            Type::Named { arguments, .. } => arguments.get(child),
            Type::Function { parameters, result } => parameters
                .get(child)
                .or_else(|| (child == parameters.len()).then_some(result.as_ref())),
            _ => None,
        };
        if let Some(next) = next {
            if depth == DEPTH {
                return None;
            }
            stack[depth - 1] = Some((ty, child + 1));
            stack[depth] = Some((next, 0));
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Some(total)
}

/// Capacity inference only; normal source and HIR authentication still apply.
pub(crate) fn return_type(expression: &Expr) -> Option<Type> {
    let constructor = inspect(expression).ok()?;
    Some(Type::Named {
        name: "Vec".to_owned(),
        arguments: vec![constructor.element.clone()],
    })
}
