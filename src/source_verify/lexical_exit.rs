//! Independent authored contour validation before type and ownership inference.
use super::diagnostics::error;
use crate::ast::{
    Expr, ExprKind, Function, LetSyntax, LexicalExitKind, Program, Statement, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;

pub(super) fn validate(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    for function in program
        .functions
        .iter()
        .chain(program.types.iter().flat_map(|ty| match &ty.kind {
            TypeDeclarationKind::Class { methods, .. } => methods.as_slice(),
            _ => &[],
        }))
    {
        check_function(program, function, diagnostics);
    }
}
fn check_function(program: &Program, function: &Function, diagnostics: &mut Vec<Diagnostic>) {
    for contract in function.requires.iter().chain(&function.ensures) {
        walk(program, contract, false, 0, 0, diagnostics);
    }
    walk(
        program,
        &function.body,
        function.yields.is_none(),
        0,
        0,
        diagnostics,
    );
}
fn walk(
    program: &Program,
    expr: &Expr,
    statement_route: bool,
    loops: usize,
    depth: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if depth > crate::cache_codec::MAX_DEPTH {
        diagnostics.push(error(
            program,
            "SPX-P207",
            "lexical exit contour exceeds source nesting bound",
            expr.span,
        ));
        return;
    }
    match &expr.kind {
        ExprKind::LexicalExit { kind, value } => {
            if !statement_route {
                diagnostics.push(error(program, "SPX-T312", "lexical exit crosses a value-evaluation or callable boundary", expr.span)
                    .with_help("place the exit on a direct function, loop or conditional statement-tail route; initializer, argument, aggregate, return-operand, yield and closure crossings are not admitted"));
            } else if matches!(kind, LexicalExitKind::Break | LexicalExitKind::Continue)
                && loops == 0
            {
                diagnostics.push(
                    error(
                        program,
                        "SPX-T313",
                        "lexical loop exit has no enclosing loop target",
                        expr.span,
                    )
                    .with_help("use break or continue only inside the nearest source loop"),
                );
            }
            if (kind == &LexicalExitKind::Return) != value.is_some() {
                diagnostics.push(error(
                    program,
                    "SPX-T312",
                    "lexical exit value does not match its control kind",
                    expr.span,
                ));
            }
            if let Some(value) = value {
                walk(program, value, false, loops, depth + 1, diagnostics);
            }
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    Statement::Let { syntax, value, .. } => walk(
                        program,
                        value,
                        statement_route && matches!(syntax, LetSyntax::StatementIf(_)),
                        loops,
                        depth + 1,
                        diagnostics,
                    ),
                    Statement::Assign { value, .. } => {
                        walk(program, value, false, loops, depth + 1, diagnostics)
                    }
                    Statement::Unsafe { body, .. } => {
                        walk(program, body, false, loops, depth + 1, diagnostics)
                    }
                    Statement::While {
                        condition, body, ..
                    } => {
                        walk(program, condition, false, loops, depth + 1, diagnostics);
                        walk(
                            program,
                            body,
                            statement_route,
                            loops + 1,
                            depth + 1,
                            diagnostics,
                        );
                    }
                    Statement::For { values, body, .. }
                    | Statement::ForBorrow { values, body, .. } => {
                        walk(program, values, false, loops, depth + 1, diagnostics);
                        walk(
                            program,
                            body,
                            statement_route,
                            loops + 1,
                            depth + 1,
                            diagnostics,
                        );
                    }
                    Statement::ForOwn { values, body, .. } => {
                        walk(program, values, false, loops, depth + 1, diagnostics);
                        walk(
                            program,
                            body,
                            statement_route,
                            loops + 1,
                            depth + 1,
                            diagnostics,
                        );
                    }
                }
            }
            walk(
                program,
                tail,
                statement_route,
                loops,
                depth + 1,
                diagnostics,
            );
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            walk(program, condition, false, loops, depth + 1, diagnostics);
            walk(
                program,
                then_branch,
                statement_route,
                loops,
                depth + 1,
                diagnostics,
            );
            walk(
                program,
                else_branch,
                statement_route,
                loops,
                depth + 1,
                diagnostics,
            );
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            walk(program, scrutinee, false, loops, depth + 1, diagnostics);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    walk(program, guard, false, loops, depth + 1, diagnostics);
                }
                walk(
                    program,
                    &arm.value,
                    statement_route,
                    loops,
                    depth + 1,
                    diagnostics,
                );
            }
        }
        ExprKind::Closure { body, .. } => walk(program, body, false, 0, depth + 1, diagnostics),
        _ => {
            let mut index = 0;
            while let Some(child) = expr.child(index) {
                walk(program, child, false, loops, depth + 1, diagnostics);
                index += 1;
            }
        }
    }
}
