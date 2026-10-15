// Saved original iterative validator arms, preserving literal content.
macro_rules! owned_dispatch_1 {
    ($frame:expr, $this:ident, $function:ident, $allow_moves:ident, $allowed_effects:ident, $program:ident, $publication:ident, $frames:ident, $scopes:ident, $Frame:ident) => {
        match $frame {
                $Frame::CallNext {
                    expression,
                    args,
                    params,
                    return_type,
                    return_ownership,
                    index,
                    scope,
                    path,
                } => {
                    if index == args.len() {
                        $this.finish_expr(expression, &return_type, return_ownership)?;
                        $scopes.push(scope);
                    } else {
                        $frames.push($Frame::CallAfterArg {
                            expression,
                            args,
                            params,
                            return_type,
                            return_ownership,
                            index,
                            path: path.clone(),
                        });
                        $frames.push($Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: crate::vec_ops::string_slot::argument_path(
                                expression,
                                path.as_ref(),
                                index,
                            ),
                        });
                    }
                }
                $Frame::CallAfterArg {
                    expression,
                    args,
                    params,
                    return_type,
                    return_ownership,
                    index,
                    path,
                } => {
                    let mut scope = $scopes.pop().expect("call argument scope retained");
                    $publication.publish(&scope);
                    let argument = &args[index];
                    let param = params.parameter(index);
                    $this.require_type(&argument.ty, param.ty, "call argument")?;
                    $this.validate_argument_ownership_view(argument, param)?;
                    let param = params.parameter(index);
                    $this.validate_borrowed_bytes_call_argument_fields(
                        expression,
                        argument,
                        (param.ty, param.ownership),
                        index,
                        &scope,
                    )?;
                    if *param.ty == ResolvedType::SliceU8 {
                        $this.validate_byte_slice_call_argument(argument, &scope)?;
                    }
                    if $this.is_owned_resource(param.ty, param.ownership)? {
                        if !$allow_moves && !contract_owners::local(argument, &scope) {
                            let callee = match &expression.kind {
                                ResolvedExprKind::Call { callee, .. } => callee.as_str(),
                                ResolvedExprKind::LiteralFormat { .. } => crate::literal_format::ID,
                                ResolvedExprKind::VecFieldRead { .. } => crate::vec_field::ID,
                                _ => unreachable!(),
                            };
                            return Err(hir_error(format!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        $this.mark_value_sources_moved(argument, &mut scope)?;
                        $publication.publish(&scope);
                    }
                    $frames.push($Frame::CallNext {
                        expression,
                        args,
                        params,
                        return_type,
                        return_ownership,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                $Frame::NativeNext {
                    expression,
                    args,
                    params,
                    result,
                    index,
                    scope,
                    path,
                } => {
                    if index == args.len() {
                        let ownership = if let ResolvedExprKind::NativeRustImportCall(call) =
                            &expression.kind
                        {
                            call.result.ownership()
                        } else {
                            OwnershipMode::Value
                        };
                        $this.finish_expr(expression, &result, ownership)?;
                        $scopes.push(scope);
                    } else {
                        $frames.push($Frame::NativeAfterArg {
                            expression,
                            args,
                            params,
                            result,
                            index,
                            path,
                        });
                        $frames.push($Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: format!("{path}.native-rust-arg.{index}"),
                        });
                    }
                }
                $Frame::NativeAfterArg {
                    expression,
                    args,
                    params,
                    result,
                    index,
                    path,
                } => {
                    let mut scope = $scopes.pop().expect("native argument scope retained");
                    $publication.publish(&scope);
                    let argument = &args[index];
                    let parameter = &params[index];
                    $this.require_type(&argument.ty, &parameter.ty, "native Rust import argument")?;
                    if argument.ownership != parameter.ownership
                        && !(parameter.ownership == OwnershipMode::Borrow
                            && argument.ownership == OwnershipMode::Own
                            && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
                    {
                        return Err(hir_error(
                            "native Rust import argument ownership disagrees with its declaration",
                        ));
                    }
                    if parameter.ownership == OwnershipMode::Own {
                        if !$allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership to native Rust",
                            ));
                        }
                        $this.mark_value_sources_moved(argument, &mut scope)?;
                        $publication.publish(&scope);
                    }
                    $frames.push($Frame::NativeNext {
                        expression,
                        args,
                        params,
                        result,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                $Frame::BinaryLeft {
                    expression,
                    op,
                    right,
                    path,
                } => {
                    let left_scope = $scopes.pop().expect("binary left scope retained");
                    $publication.publish(&left_scope);
                    let baseline = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        Some((left_scope.keys().cloned().collect(), left_scope.clone()))
                    } else {
                        None
                    };
                    $frames.push($Frame::BinaryRight {
                        expression,
                        op,
                        left: match &expression.kind {
                            ResolvedExprKind::Binary { left, .. } => left,
                            _ => unreachable!(),
                        },
                        baseline,
                    });
                    if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        let enabled = $publication.enabled;
                        $publication.enabled = false;
                        $frames.push($Frame::RestorePublication(enabled));
                    }
                    $frames.push($Frame::Enter {
                        expression: right,
                        scope: left_scope,
                        path: format!("{path}.right"),
                    });
                }
                $Frame::BinaryRight {
                    expression,
                    op,
                    left,
                    baseline,
                } => {
                    let mut scope = $scopes.pop().expect("binary right scope retained");
                    let direct = baseline.is_none();
                    if let Some((ids, mut parent)) = baseline {
                        Self::join_conditional(&mut parent, &scope, &ids);
                        scope = parent;
                    }
                    if direct || matches!(op, BinaryOp::And | BinaryOp::Or) {
                        $publication.publish(&scope);
                    }
                    let ResolvedExprKind::Binary { right, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let output = match op {
                        BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Rem => {
                            // Arithmetic keeps the operand type: i64 and u8
                            // arithmetic is checked, IEEE-754 float
                            // arithmetic is total and never selects a status.
                            if !matches!(
                                &left.ty,
                                ResolvedType::I64
                                    | ResolvedType::I32
                                    | ResolvedType::U8
                                    | ResolvedType::Usize
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                            ) || (matches!(op, BinaryOp::Rem)
                                && !matches!(
                                    left.ty,
                                    ResolvedType::I64
                                        | ResolvedType::I32
                                        | ResolvedType::U8
                                        | ResolvedType::Usize
                                ))
                            {
                                return Err(hir_error(
                                    "binary operand has inconsistent resolved types",
                                ));
                            }
                            $this.require_type(&left.ty, &right.ty, "binary operand")?;
                            left.ty.clone()
                        }
                        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                            // Ordered comparison compares scalar values: i64,
                            // IEEE-754 numerics, and Unicode scalar chars.
                            if !matches!(
                                &left.ty,
                                ResolvedType::I64
                                    | ResolvedType::I32
                                    | ResolvedType::Char
                                    | ResolvedType::U8
                                    | ResolvedType::Usize
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                                    | ResolvedType::String
                            ) {
                                return Err(hir_error(
                                    "comparison operand has inconsistent resolved types",
                                ));
                            }
                            $this.require_type(&left.ty, &right.ty, "comparison operand")?;
                            ResolvedType::Bool
                        }
                        BinaryOp::And | BinaryOp::Or => {
                            $this.require_type(&left.ty, &ResolvedType::Bool, "boolean operand")?;
                            $this.require_type(&right.ty, &ResolvedType::Bool, "boolean operand")?;
                            ResolvedType::Bool
                        }
                        BinaryOp::Eq | BinaryOp::Ne => {
                            if left.ty.is_callable() || right.ty.is_callable() {
                                return Err(hir_error(
                                    "function value equality is outside the admitted profile",
                                ));
                            }
                            $this.require_type(&left.ty, &right.ty, "equality operands")?;
                            ResolvedType::Bool
                        }
                    };
                    $this.finish_expr(expression, &output, OwnershipMode::Value)?;
                    $scopes.push(scope);
                }
                $Frame::LexicalReturn {
                    expression,
                    operand,
                } => {
                    let mut scope = $scopes.pop().expect("return operand scope retained");
                    $this.validate_lexical_return_operand(operand, &mut scope)?;
                    $this.finish_expr(expression, &ResolvedType::Never, OwnershipMode::Value)?;
                    $scopes.push(scope);
                }
                $Frame::IfCondition {
                    expression,
                    then_branch,
                    else_branch,
                    path,
                } => {
                    let outer = $scopes.pop().expect("if condition scope retained");
                    $publication.publish(&outer);
                    let ResolvedExprKind::If { condition, .. } = &expression.kind else {
                        unreachable!()
                    };
                    $this.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                    let outer_ids = outer.keys().cloned().collect();
                    $frames.push($Frame::IfThen {
                        expression,
                        else_branch,
                        path,
                        outer: outer.clone(),
                        outer_ids,
                    });
                    let enabled = $publication.enabled;
                    $publication.enabled = false;
                    $frames.push($Frame::RestorePublication(enabled));
                    $frames.push($Frame::Enter {
                        expression: then_branch,
                        scope: outer,
                        path: format!("{path}.then"),
                    });
                }
                $Frame::IfThen {
                    expression,
                    else_branch,
                    path,
                    outer,
                    outer_ids,
                } => {
                    let then_scope = $scopes.pop().expect("if then scope retained");
                    $frames.push($Frame::IfElse {
                        expression,
                        then_scope,
                        outer: outer.clone(),
                        outer_ids,
                    });
                    let enabled = $publication.enabled;
                    $publication.enabled = false;
                    $frames.push($Frame::RestorePublication(enabled));
                    $frames.push($Frame::Enter {
                        expression: else_branch,
                        scope: outer,
                        path: format!("{path}.else"),
                    });
                }
                $Frame::IfElse {
                    expression,
                    then_scope,
                    mut outer,
                    outer_ids,
                } => {
                    let else_scope = $scopes.pop().expect("if else scope retained");
                    let ResolvedExprKind::If {
                        then_branch,
                        else_branch,
                        ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    $this.finish_lexical_if(
                        expression,
                        then_branch,
                        else_branch,
                        &mut outer,
                        &then_scope,
                        &else_scope,
                        &outer_ids,
                    )?;
                    $publication.publish(&outer);
                    $scopes.push(outer);
                }
                $Frame::BlockNext {
                    expression,
                    statements,
                    tail,
                    index,
                    scope,
                    outer,
                    outer_ids,
                    path,
                } => {
                    if index == statements.len() {
                        $frames.push($Frame::BlockTail {
                            expression,
                            outer_ids,
                            outer,
                        });
                        let enabled = $publication.enabled;
                        $publication.enabled = false;
                        $frames.push($Frame::RestorePublication(enabled));
                        $frames.push($Frame::Enter {
                            expression: tail,
                            scope,
                            path: format!("{path}.tail"),
                        });
                    } else {
                        match &statements[index] {
                            ResolvedStatement::Let { value, .. } => {
                                $frames.push($Frame::BlockAfterLet {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                });
                                let enabled = $publication.enabled;
                                $publication.enabled = false;
                                $frames.push($Frame::RestorePublication(enabled));
                                $frames.push($Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            ResolvedStatement::CopyDiscard { value, .. } => {
                                $frames.push($Frame::BlockAfterUnsafe {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                });
                                let enabled = $publication.enabled;
                                $publication.enabled = false;
                                $frames.push($Frame::RestorePublication(enabled));
                                $frames.push($Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            ResolvedStatement::Assign { binding, value, .. } => {
                                if crate::byte_ops::is_same_owner_set_hir(value, &binding.id) {
                                    $this.buffer_reopen_sites.insert(value.id.clone());
                                }
                                // The target must be a previously declared
                                // mutable scalar binding in this block's scope.
                                if !scope.contains_key(&binding.id) {
                                    return Err(hir_error(format!(
                                        "assignment target `{}` has no enclosing binding",
                                        binding.id
                                    )));
                                }
                                let target_scope = scope.clone();
                                $frames.push($Frame::BlockAfterAssign {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    scope: target_scope,
                                    outer,
                                    outer_ids,
                                    path,
                                });
                                let enabled = $publication.enabled;
                                $publication.enabled = false;
                                $frames.push($Frame::RestorePublication(enabled));
                                $frames.push($Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            ResolvedStatement::Unsafe { body, .. } => {
                                // Contract expressions stay pure; ordinary
                                // blocks resolve the body like any nested
                                // block and bind nothing outside it.
                                if !$allow_moves {
                                    return Err(hir_error(
                                        "contract expressions cannot contain unsafe boundary statements",
                                    ));
                                }
                                $frames.push($Frame::BlockAfterUnsafe {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                });
                                let enabled = $publication.enabled;
                                $publication.enabled = false;
                                $frames.push($Frame::RestorePublication(enabled));
                                $frames.push($Frame::Enter {
                                    expression: body,
                                    scope,
                                    path: format!("{path}.s{index}.body"),
                                });
                            }
                            ResolvedStatement::While {
                                condition, body, ..
                            }
                            | ResolvedStatement::LexicalWhile {
                                condition, body, ..
                            } => {
                                // Bounded While-Loops v1: re-check admission
                                // at the trust boundary and require an exact
                                // `bool` condition before validating the body
                                // like any nested block.
                                $this.validate_loop_pair(condition, body)?;
                                if condition.ty != ResolvedType::Bool {
                                    return Err(hir_error("`while` condition must be bool"));
                                }
                                $frames.push($Frame::BlockAfterWhileCondition {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                    body,
                                    entry: scope.clone(),
                                });
                                let enabled = $publication.enabled;
                                $publication.enabled = false;
                                $frames.push($Frame::RestorePublication(enabled));
                                $frames.push($Frame::Enter {
                                    expression: condition,
                                    scope,
                                    path: format!("{path}.s{index}.condition"),
                                });
                            }
                        }
                    }
                }
                $Frame::BlockAfterUnsafe {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                } => {
                    // The body block validated like any nested block; its
                    // merged scope continues into the enclosing block.
                    let scope = $scopes.pop().expect("unsafe body scope retained");
                    match &statements[index] {
                        ResolvedStatement::CopyDiscard { value, .. } => {
                            super::copy_discard::require(value)?
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            if body.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&body.ty)
                            {
                                return Err(hir_error(
                                    "unsafe boundary bodies must produce a scalar Copy value",
                                ));
                            }
                        }
                        _ => unreachable!("no-binding statement continuation"),
                    }
                    $frames.push($Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                $Frame::BlockAfterWhileCondition {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                    body,
                    entry,
                } => {
                    // The condition validated like any expression; it must be
                    // exactly `bool` because it re-evaluates before every
                    // iteration.
                    let scope = $scopes.pop().expect("while condition scope retained");
                    let (ResolvedStatement::While { condition, .. }
                    | ResolvedStatement::LexicalWhile { condition, .. }) = &statements[index]
                    else {
                        unreachable!("while condition frame resumes at a while statement")
                    };
                    if scope != entry {
                        return Err(hir_error("while condition changes ownership liveness"));
                    }
                    if condition.ty != ResolvedType::Bool {
                        return Err(hir_error("`while` condition must be bool"));
                    }
                    $frames.push($Frame::BlockAfterWhileBody {
                        expression,
                        statements,
                        tail,
                        index,
                        outer,
                        outer_ids,
                        path,
                        entry: scope.clone(),
                    });
                    let enabled = $publication.enabled;
                    $publication.enabled = false;
                    $frames.push($Frame::RestorePublication(enabled));
                    $frames.push($Frame::Enter {
                        expression: body,
                        scope,
                        path: format!("{path}.s{index}.body"),
                    });
                }
                $Frame::BlockAfterWhileBody {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                    entry,
                } => {
                    // Zero or more iterations run: every route reaching the
                    // continuation must preserve the loop-entry ownership state.
                    let after = $scopes.pop().expect("while body scope retained");
                    let body = statements[index].child(1).expect("loop body");
                    if !crate::hir::lexical_exit::returns_only(body) && after != entry {
                        return Err(hir_error("while loop body changes ownership liveness"));
                    }
                    $frames.push($Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope: entry,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                $Frame::BlockAfterLet {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                } => {
                    let mut scope = $scopes.pop().expect("block let scope retained");
                    let ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("let frame resumes at a let statement")
                    };
                    let statement_path = format!("{path}.s{index}");
                    if !binding.id.matches_local($function, &statement_path) {
                        return Err(hir_error(format!(
                            "local `{}` has a non-canonical identity",
                            binding.id
                        )));
                    }
                    $this.insert_value(&binding.id)?;
                    $this.require_type(&binding.ty, &value.ty, "local binding")?;
                    if binding.ownership != value.ownership {
                        return Err(hir_error(format!(
                            "local `{}` has inconsistent ownership",
                            binding.id
                        )));
                    }
                    $this.validate_declared_ownership(&binding.ty, binding.ownership)?;
                    if binding.ty == ResolvedType::SliceU8 {
                        let (place, origin) = match &value.kind {
                            ResolvedExprKind::Place(place) => {
                                let origin = $this.byte_slice_aliases.get(&place.root).cloned().ok_or_else(|| {
                                    hir_error("byte-slice local alias lacks symbolic parameter-root provenance")
                                })?;
                                (place, origin)
                            }
                            ResolvedExprKind::BorrowPlace { place, .. } => (place, place.clone()),
                            ResolvedExprKind::Call { .. }
                                if crate::vec_ops::string_slot::inspect(value).is_some() =>
                            {
                                let place = crate::vec_ops::string_slot::place(value)
                                    .expect("authenticated slot");
                                (place, place.clone())
                            }
                            ResolvedExprKind::VecFieldRead { args, .. } => {
                                let ResolvedExprKind::Place(place) = &args[0].kind else {
                                    return Err(hir_error("vector view has no carrier place"));
                                };
                                (place, place.clone())
                            }
                            ResolvedExprKind::ByteRange { source, .. } => {
                                $this.byte_range_origin(source, &scope)?
                            }
                            _ => {
                                return Err(hir_error(
                                    "byte-slice local must be a direct immutable alias or authenticated view",
                                ));
                            }
                        };
                        let is_authenticated_view = crate::vec_ops::string_slot::inspect(value)
                            .is_some()
                            || matches!(
                                &value.kind,
                                ResolvedExprKind::BorrowPlace { .. }
                                    | ResolvedExprKind::VecFieldRead { .. }
                            );
                        if *mutable || (!is_authenticated_view && !place.projections.is_empty()) {
                            return Err(hir_error(
                                "byte-slice local alias must be immutable and unprojected",
                            ));
                        }
                        $this.byte_slice_aliases
                            .insert(binding.id.clone(), origin.clone());
                        let borrowed_origin = match &value.kind {
                            ResolvedExprKind::Call { .. }
                                if crate::vec_ops::string_slot::inspect(value).is_some() =>
                            {
                                Some(origin)
                            }
                            ResolvedExprKind::BorrowPlace { .. }
                            | ResolvedExprKind::VecFieldRead { .. }
                            | ResolvedExprKind::ByteRange { .. } => Some(origin),
                            _ => None,
                        };
                        if let Some(origin) = borrowed_origin {
                            let owner = scope.get_mut(&origin.root).ok_or_else(|| {
                                hir_error("byte view owner disappeared before lexical borrow")
                            })?;
                            owner.active_loans.extend($this.exact_loan_id(
                                $function,
                                &value.id,
                                &LoanCause::SliceView,
                            )?);
                        }
                    } else if binding.ty == ResolvedType::Str {
                        let (origin, creates_loan) =
                            $this.borrowed_str_let_origin(value, binding)?;
                        if *mutable {
                            return Err(hir_error(
                                "borrowed-str local alias must be immutable and unprojected",
                            ));
                        }
                        $this.borrowed_str_aliases
                            .insert(binding.id.clone(), origin.clone());
                        if creates_loan {
                            let owner = scope.get_mut(&origin.root).ok_or_else(|| {
                                hir_error("String view owner disappeared before lexical borrow")
                            })?;
                            owner.active_loans.extend($this.exact_loan_id(
                                $function,
                                &value.id,
                                &LoanCause::StrView,
                            )?);
                        }
                    }
                    let contract_local = !$allow_moves && contract_owners::fresh(value, &scope);
                    if $this.is_owned_resource(&binding.ty, binding.ownership)? {
                        if !$allow_moves && !contract_owners::fresh(value, &scope) {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a local binding",
                            ));
                        }
                        $this.mark_value_sources_moved(value, &mut scope)?;
                    }
                    scope.insert(
                        binding.id.clone(),
                        ValidationBinding {
                            contract_local,
                            mutable: *mutable,
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            availability: Availability::Available,
                            active_loans: BTreeSet::new(),
                            moved_places: BTreeMap::new(),
                            definitely_partial: BTreeSet::new(),
                        },
                    );
                    $frames.push($Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                $Frame::BlockAfterAssign {
                    expression,
                    statements,
                    tail,
                    index,
                    scope,
                    outer,
                    outer_ids,
                    path,
                } => {
                    let assigned_scope = $scopes.pop().expect("block assign scope retained");
                    let mut scope = scope;
                    let ResolvedStatement::Assign {
                        binding,
                        field,
                        value: assigned,
                        span: assignment_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("assign frame resumes at an assignment statement")
                    };
                    if crate::stdin_stream_ops::hir_reopen(assigned, &binding.id) {
                        scope = assigned_scope;
                        stdin_stream::reopen_reader(&mut scope, &binding.id)?;
                    } else if crate::hir::iterator_loop::is_step_reassignment(assigned, &binding.id)
                    {
                        scope = assigned_scope;
                        iterator_loops::reopen_step(&mut scope, &binding.id)?;
                    } else if crate::string_ops::is_same_owner_concat_hir(assigned, &binding.id) {
                        scope = assigned_scope;
                        iterator_loops::reopen_string(&mut scope, &binding.id)?;
                    } else if crate::string_ops::replacement::admitted(binding, assigned) {
                        if scope
                            .get(&binding.id)
                            .is_none_or(|target| target.availability != Availability::Available)
                        {
                            return Err(hir_error(
                                "String replacement starts with an unavailable owner",
                            ));
                        }
                        scope = assigned_scope;
                        $this.mark_value_sources_moved(assigned, &mut scope)?;
                        string_replacement::reopen(&mut scope, &binding.id)?;
                    }
                    let target = scope.get(&binding.id).cloned();
                    let Some(target) = target else {
                        return Err(hir_error(format!(
                            "assignment target `{}` has no enclosing binding",
                            binding.id
                        )));
                    };
                    let assigned_place = Place {
                        root: binding.id.clone(),
                        projections: field.iter().cloned().map(PlaceProjection::Field).collect(),
                    };
                    if $this.binding_has_live_overlapping_loans(
                        &target,
                        &assigned_place,
                        &assigned.id,
                        LoanPointPhase::After,
                    )? {
                        return Err(hir_error(
                            "SPX-T265: assignment would replace lexically borrowed byte storage",
                        ));
                    }
                    match field {
                        Some(field) => {
                            if target.ownership != OwnershipMode::Value {
                                return Err(hir_error(
                                    "field assignment base is not a value-owned aggregate",
                                ));
                            }
                            $this.validate_assign_field(
                                &target.ty,
                                field,
                                assigned,
                                *assignment_span,
                            )?;
                        }
                        None => {
                            $this.require_type(&target.ty, &assigned.ty, "assignment")?;
                            if (target.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&target.ty))
                                && !crate::string_ops::replacement::admitted(binding, assigned)
                                && !owner_renewal::assignment_is_admitted(
                                    $this.$program,
                                    $this.byte_helpers,
                                    binding,
                                    assigned,
                                )
                            {
                                return Err(hir_error(
                                    "explicit mutation v1 supports only scalar Copy values",
                                ));
                            }
                        }
                    }
                    if target.availability != Availability::Available {
                        return Err(hir_error(format!(
                            "assignment target `{}` is not available",
                            binding.id
                        )));
                    }
                    $frames.push($Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                $Frame::BlockTail {
                    expression,
                    outer_ids,
                    mut outer,
                } => {
                    let block_scope = $scopes.pop().expect("block tail scope retained");
                    Self::merge_availability(&mut outer, &block_scope, &outer_ids);
                    $publication.publish(&outer);
                    let (ResolvedExprKind::Block { tail, .. }
                    | ResolvedExprKind::VecConstructor { tail, .. }) = &expression.kind
                    else {
                        unreachable!()
                    };
                    $this.finish_expr(expression, &tail.ty, tail.ownership)?;
                    $scopes.push(outer);
                }
                $Frame::RecordNext {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                } => {
                    if index == fields.len() {
                        if seen.len() != expected.len() {
                            return Err(hir_error(format!(
                                "constructor for `{record}` is missing required fields"
                            )));
                        }
                        let ownership =
                            $this.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                        $this.finish_expr(expression, &expression.ty, ownership)?;
                        $scopes.push(scope);
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(hir_error(format!(
                                "constructor for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(hir_error(format!(
                                "constructor for `{record}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        $frames.push($Frame::RecordAfterField {
                            expression,
                            fields,
                            expected,
                            record,
                            arguments,
                            seen,
                            index,
                            path,
                        });
                        $frames.push($Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
            other => owned_dispatch_2!(other, $this, $function, $allow_moves, $allowed_effects, $program, $publication, $frames, $scopes, $Frame),
        }
    };
}
