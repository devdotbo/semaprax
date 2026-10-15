use super::super::{Diagnostic, Expr, ExprKind};
use std::fmt::{self, Write};

struct Path {
    value: String,
}
impl Path {
    fn new(root: &str) -> Result<Self, Vec<Diagnostic>> {
        reserve(std::mem::size_of::<Self>())?;
        let mut path = Self {
            value: String::new(),
        };
        path.write_str(root).map_err(|_| refusal())?;
        Ok(path)
    }
    fn ensure(&mut self, additional: usize) -> fmt::Result {
        let required = self.value.len().checked_add(additional).ok_or(fmt::Error)?;
        if required <= self.value.capacity() {
            return Ok(());
        }
        let capacity = self
            .value
            .capacity()
            .checked_mul(2)
            .ok_or(fmt::Error)?
            .max(32)
            .max(required);
        // The old backing remains live until replacement: debit the full new backing.
        if !crate::bounded_output::reserve_active_required(capacity) {
            return Err(fmt::Error);
        }
        self.value
            .try_reserve_exact(capacity - self.value.len())
            .map_err(|_| fmt::Error)?;
        if self.value.capacity() != capacity {
            return Err(fmt::Error);
        }
        Ok(())
    }
}
impl Write for Path {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.ensure(text.len())?;
        self.value.push_str(text);
        Ok(())
    }
}
fn reserve(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(refusal())
    }
}
fn refusal() -> Vec<Diagnostic> {
    vec![super::super::limit_error(
        "builder_bytes",
        super::super::active_builder_limit(),
    )]
}
pub(in crate::workspace_graph) fn visit_ast_expression_sites(
    expression: &Expr,
    root: &str,
    visit: &mut impl FnMut(&Expr, &str) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    walk(expression, &mut Path::new(root)?, visit)
}
fn child(
    expression: &Expr,
    suffix: fmt::Arguments<'_>,
    path: &mut Path,
    visit: &mut impl FnMut(&Expr, &str) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    let length = path.value.len();
    let result = match path.write_fmt(suffix) {
        Ok(()) => walk(expression, path, visit),
        Err(_) => Err(refusal()),
    };
    path.value.truncate(length);
    result
}
fn walk(
    expression: &Expr,
    path: &mut Path,
    visit: &mut impl FnMut(&Expr, &str) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    visit(expression, path.value.as_str())?;
    match &expression.kind {
        ExprKind::VecConstructor { value, .. } => {
            walk(value, path, visit)?;
        }
        ExprKind::Closure { body, .. } => child(body, format_args!(".closure.body"), path, visit)?,
        ExprKind::Call { args, .. } => {
            for (index, argument) in args.iter().enumerate() {
                child(argument, format_args!(".arg.{index}"), path, visit)?;
            }
        }
        ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::LexicalExit { value: None, .. } => {}
        ExprKind::LexicalExit {
            value: Some(value), ..
        }
        | ExprKind::Unary { value, .. } => {
            child(value, format_args!(".value"), path, visit)?;
        }
        ExprKind::Binary { left, right, .. } => {
            child(left, format_args!(".left"), path, visit)?;
            child(right, format_args!(".right"), path, visit)?;
        }
        ExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    crate::ast::Statement::Let { value, .. }
                    | crate::ast::Statement::Assign { value, .. } => {
                        child(value, format_args!(".s{index}.value"), path, visit)?
                    }
                    crate::ast::Statement::Unsafe { body, .. } => {
                        child(body, format_args!(".s{index}.value"), path, visit)?
                    }
                    crate::ast::Statement::While {
                        condition, body, ..
                    } => {
                        child(condition, format_args!(".s{index}.condition"), path, visit)?;
                        child(body, format_args!(".s{index}.body"), path, visit)?;
                    }
                    crate::ast::Statement::For { values, body, .. }
                    | crate::ast::Statement::ForBorrow { values, body, .. } => {
                        // Bounded `for` traversal is lowered, so its authored
                        // children do not sit at `.values` and `.body`.
                        // `hir::resolve_for::lower` desugars the statement at
                        // `s{index}` into `s{index}.value`: `.s0` binds the
                        // length, `.s1` the index, `.s2` is the `while`, and
                        // the authored body is resolved at
                        // `.value.s2.body.s1.value` behind the item binding at
                        // `.value.s2.body.s0`. The source is admitted only as
                        // an immutable binding (`SPX-T284`), so it reaches the
                        // lowering as the place argument of the `vec_len`
                        // call. Naming the authored paths is what keeps this
                        // reconstruction independent of the HIR while still
                        // describing the same program.
                        child(
                            values,
                            format_args!(".s{index}.value.s0.value.arg.0"),
                            path,
                            visit,
                        )?;
                        child(
                            body,
                            format_args!(".s{index}.value.s2.body.s1.value"),
                            path,
                            visit,
                        )?;
                    }
                    crate::ast::Statement::ForOwn { values, body, .. } => {
                        // Consuming iterator traversal has a distinct HIR
                        // lowering. Its iterator source is staged at `.s0`,
                        // and the yielded-item binding is at
                        // `.s1.body.s0.value.arm.1.binding.0`; its authored
                        // body follows that binding at `.arm.1.value.s0.value`.
                        child(
                            values,
                            format_args!(".s{index}.value.s0.value.arg.0"),
                            path,
                            visit,
                        )?;
                        child(
                            body,
                            format_args!(".s{index}.value.s1.body.s0.value.arm.1.value.s0.value"),
                            path,
                            visit,
                        )?;
                    }
                }
            }
            child(tail, format_args!(".tail"), path, visit)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            child(condition, format_args!(".condition"), path, visit)?;
            child(then_branch, format_args!(".then"), path, visit)?;
            child(else_branch, format_args!(".else"), path, visit)?;
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                child(
                    &field.value,
                    format_args!(".field.{index}.value"),
                    path,
                    visit,
                )?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            child(scrutinee, format_args!(".scrutinee"), path, visit)?;
            for (index, arm) in arms.iter().enumerate() {
                if let Some(guard) = &arm.guard {
                    child(
                        guard.as_ref(),
                        format_args!(".arm.{index}.guard"),
                        path,
                        visit,
                    )?;
                }
                child(&arm.value, format_args!(".arm.{index}.value"), path, visit)?;
            }
        }
        ExprKind::Try { operand } | ExprKind::Yield { request: operand } => {
            child(operand, format_args!(".operand"), path, visit)?;
        }
        ExprKind::UpdateRecord { base, fields } => {
            child(base, format_args!(".base"), path, visit)?;
            for (index, field) in fields.iter().enumerate() {
                child(
                    &field.value,
                    format_args!(".field.{index}.value"),
                    path,
                    visit,
                )?;
            }
        }
        ExprKind::Project { base, .. } => {
            child(base, format_args!(".base"), path, visit)?;
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::BorrowedUtf8Literal(_)
        | ExprKind::Var(_) => {}
        // Method calls resolve to hoisted functions in HIR; the AST-level
        // call-site walk sees them through the resolved Call edge instead.
        ExprKind::MethodCall { .. } => {}
        // `super.method(...)` also resolves to a hoisted parent method in HIR.
        ExprKind::SuperMethod { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "path_cursor_tests.rs"]
mod tests;
