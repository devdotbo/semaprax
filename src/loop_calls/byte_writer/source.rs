use super::*;
#[derive(Clone, Copy)]
enum Frame<'a> {
    Function(&'a Function, usize),
    Expression(&'a Expr, usize),
}

pub(crate) fn nonallocating(program: &Program, target: &Function) -> bool {
    prove(program, target).is_some()
}
fn prove(program: &Program, target: &Function) -> Option<()> {
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<
        [Option<Frame<'_>>; DEPTH],
    >()) {
        return None;
    }
    let mut frames = [None; DEPTH];
    frames[0] = Some(Frame::Function(target, 0));
    let mut depth = 1usize;
    let mut work = WORK;
    while depth != 0 {
        work = work.checked_sub(1)?;
        let current = depth - 1;
        let child = match frames[current]? {
            Frame::Function(function, root) => {
                if root == 0
                    && (!function.effects.is_empty()
                        || !function.type_parameters.is_empty()
                        || function.yields.is_some()
                        || function.follows.is_some()
                        || !function
                            .params
                            .iter()
                            .all(|p| ast_param(program, p.mode, &p.ty))
                        || !(function.return_type == Type::Bytes
                            || super::super::ast_result_admitted(program, &function.return_type)
                                && function.return_type != Type::String))
                {
                    return None;
                }
                let expression = if root < function.requires.len() {
                    function.requires.get(root)
                } else if root == function.requires.len() {
                    Some(&function.body)
                } else {
                    function.ensures.get(root - function.requires.len() - 1)
                };
                frames[current] = Some(Frame::Function(function, root + 1));
                expression.map(|e| Frame::Expression(e, usize::MAX))
            }
            Frame::Expression(expression, next) => {
                if next == usize::MAX {
                    frames[current] = Some(Frame::Expression(expression, 0));
                    match &expression.kind {
                        ExprKind::String(_)
                        | ExprKind::Closure { .. }
                        | ExprKind::MethodCall { .. }
                        | ExprKind::SuperMethod { .. }
                        | ExprKind::Yield { .. }
                        | ExprKind::Try { .. }
                        | ExprKind::LexicalExit { .. } => return None,
                        ExprKind::ConstructRecord {
                            type_name,
                            type_arguments,
                            ..
                        } => {
                            if !type_arguments.is_empty()
                                || !program.types.iter().any(|d| {
                                    d.name == *type_name
                                        && crate::source_verify::copy_record_declaration_admitted(d)
                                })
                            {
                                return None;
                            }
                        }
                        ExprKind::ConstructVariant {
                            type_name,
                            type_arguments,
                            ..
                        } => {
                            if !copy_variant(program, type_name, type_arguments) {
                                return None;
                            }
                        }
                        ExprKind::Block { statements, .. } => {
                            work = work.checked_sub(statements.len())?;
                            if statements
                                .iter()
                                .any(|s| matches!(s, crate::ast::Statement::Unsafe { .. }))
                            {
                                return None;
                            }
                        }
                        ExprKind::Call {
                            name,
                            type_arguments,
                            args,
                        } => {
                            if name == crate::vec_field::NAME {
                                if type_arguments.len() != 1
                                    || args.len() != 3
                                    || !matches!(&args[2].kind, ExprKind::String(field) if !field.is_empty())
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::byte_ops::by_name(name) {
                                if !byte_operation(op)
                                    || !type_arguments.is_empty()
                                    || args.len() != op.arity()
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::string_ops::by_name(name) {
                                if !text_operation(op) {
                                    return None;
                                }
                            } else if let Some(op) = crate::vec_ops::by_name(name) {
                                if !vec_operation(op) {
                                    return None;
                                }
                            } else {
                                work =
                                    work.checked_sub(program.functions.len().checked_add(depth)?)?;
                                let callee = program.functions.iter().find(|f| f.name == *name)?;
                                if !type_arguments.is_empty() || args.len() != callee.params.len()
                                    || frames[..depth].iter().any(|f| matches!(f, Some(Frame::Function(active, _)) if active.stable_id == callee.stable_id)) { return None; }
                                frames[current] = Some(Frame::Expression(expression, 0));
                                if depth == DEPTH {
                                    return None;
                                }
                                frames[depth] = Some(Frame::Function(callee, 0));
                                depth += 1;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                // Block and Match child selection traverses an actual prefix.
                work = work.checked_sub(next.checked_add(1)?.checked_mul(2)?)?;
                frames[current] = Some(Frame::Expression(expression, next + 1));
                if next == 2
                    && matches!(&expression.kind, ExprKind::Call { name, .. } if name == crate::vec_field::NAME)
                {
                    continue;
                }
                expression
                    .child(next)
                    .map(|e| Frame::Expression(e, usize::MAX))
            }
        };
        if let Some(child) = child {
            if depth == DEPTH {
                return None;
            }
            frames[depth] = Some(child);
            depth += 1;
        } else {
            frames[current] = None;
            depth -= 1;
        }
    }
    Some(())
}

fn copy_variant(program: &Program, name: &str, arguments: &[Type]) -> bool {
    if matches!(name, "Option" | "Result") {
        return arguments.len() == usize::from(name == "Result") + 1
            && arguments
                .iter()
                .all(crate::source_verify::is_scalar_source_type);
    }
    program.types.iter().any(|d| d.name == name && d.type_parameters.is_empty()
        && arguments.is_empty() && matches!(&d.kind, crate::ast::TypeDeclarationKind::Variant { cases }
            if cases.iter().all(|c| c.fields.iter().all(|f| crate::source_verify::is_scalar_source_type(&f.ty)))))
}
