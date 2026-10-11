//! Exact syntax expansion; this marker grants no ordinary Call authority.
use super::{Expr, ExprKind, LetSyntax, Statement, Type};

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
            || !name[3..].bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("Vec constructor binding is not hygienically minted");
        }
        if statements[..index].iter().any(|earlier| {
            matches!(earlier,
            Statement::Let { name: other, .. } if other == name)
        }) {
            return Err("Vec constructor binding is duplicated");
        }
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
