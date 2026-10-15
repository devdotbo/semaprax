use super::super::{Diagnostic, Expr, ExprKind};

pub(super) fn visit_ast_expression_sites(
    expression: &Expr,
    path: &str,
    visit: &mut impl FnMut(&Expr, &str) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    visit(expression, path)?;
    match &expression.kind {
        ExprKind::VecConstructor { value, .. } => {
            visit_ast_expression_sites(value, path, visit)?;
        }
        ExprKind::Closure { body, .. } => {
            visit_ast_expression_sites(body, &format!("{path}.closure.body"), visit)?
        }
        ExprKind::Call { args, .. } => {
            for (index, argument) in args.iter().enumerate() {
                visit_ast_expression_sites(
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}")),
                    visit,
                )?;
            }
        }
        ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::LexicalExit { value: None, .. } => {}
        ExprKind::LexicalExit {
            value: Some(value), ..
        }
        | ExprKind::Unary { value, .. } => {
            visit_ast_expression_sites(
                value,
                &crate::bounded_output::budgeted_format(format_args!("{path}.value")),
                visit,
            )?;
        }
        ExprKind::Binary { left, right, .. } => {
            visit_ast_expression_sites(
                left,
                &crate::bounded_output::budgeted_format(format_args!("{path}.left")),
                visit,
            )?;
            visit_ast_expression_sites(
                right,
                &crate::bounded_output::budgeted_format(format_args!("{path}.right")),
                visit,
            )?;
        }
        ExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    crate::ast::Statement::Let { value, .. }
                    | crate::ast::Statement::Assign { value, .. } => visit_ast_expression_sites(
                        value,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.value"
                        )),
                        visit,
                    )?,
                    crate::ast::Statement::Unsafe { body, .. } => visit_ast_expression_sites(
                        body,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.value"
                        )),
                        visit,
                    )?,
                    crate::ast::Statement::While {
                        condition, body, ..
                    } => {
                        visit_ast_expression_sites(
                            condition,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.condition"
                            )),
                            visit,
                        )?;
                        visit_ast_expression_sites(
                            body,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.body"
                            )),
                            visit,
                        )?;
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
                        visit_ast_expression_sites(
                            values,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s0.value.arg.0"
                            )),
                            visit,
                        )?;
                        visit_ast_expression_sites(
                            body,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s2.body.s1.value"
                            )),
                            visit,
                        )?;
                    }
                    crate::ast::Statement::ForOwn { values, body, .. } => {
                        // Consuming iterator traversal has a distinct HIR
                        // lowering. Its iterator source is staged at `.s0`,
                        // and the yielded-item binding is at
                        // `.s1.body.s0.value.arm.1.binding.0`; its authored
                        // body follows that binding at `.arm.1.value.s0.value`.
                        visit_ast_expression_sites(
                            values,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s0.value.arg.0"
                            )),
                            visit,
                        )?;
                        visit_ast_expression_sites(
                            body,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s1.body.s0.value.arm.1.value.s0.value"
                            )),
                            visit,
                        )?;
                    }
                }
            }
            visit_ast_expression_sites(
                tail,
                &crate::bounded_output::budgeted_format(format_args!("{path}.tail")),
                visit,
            )?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_ast_expression_sites(
                condition,
                &crate::bounded_output::budgeted_format(format_args!("{path}.condition")),
                visit,
            )?;
            visit_ast_expression_sites(
                then_branch,
                &crate::bounded_output::budgeted_format(format_args!("{path}.then")),
                visit,
            )?;
            visit_ast_expression_sites(
                else_branch,
                &crate::bounded_output::budgeted_format(format_args!("{path}.else")),
                visit,
            )?;
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                visit_ast_expression_sites(
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit_ast_expression_sites(
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                visit,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                if let Some(guard) = &arm.guard {
                    visit_ast_expression_sites(
                        guard.as_ref(),
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.arm.{index}.guard"
                        )),
                        visit,
                    )?;
                }
                visit_ast_expression_sites(
                    &arm.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Try { operand } | ExprKind::Yield { request: operand } => {
            visit_ast_expression_sites(
                operand,
                &crate::bounded_output::budgeted_format(format_args!("{path}.operand")),
                visit,
            )?;
        }
        ExprKind::UpdateRecord { base, fields } => {
            visit_ast_expression_sites(
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                visit,
            )?;
            for (index, field) in fields.iter().enumerate() {
                visit_ast_expression_sites(
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Project { base, .. } => {
            visit_ast_expression_sites(
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                visit,
            )?;
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
