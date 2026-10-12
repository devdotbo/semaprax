//! Saved original iterative validator arms, preserving literal content.
macro_rules! owned_dispatch_0 {
    ($frame:expr, $this:ident, $function:ident, $allow_moves:ident, $allowed_effects:ident, $program:ident, $publication:ident, $frames:ident, $scopes:ident, $Frame:ident) => {
        match $frame {
                $Frame::InvokeNext {
                    expression,
                    index,
                    mut scope,
                    path,
                } => {
                    let ResolvedExprKind::Invoke { callable, args } = &expression.kind else {
                        unreachable!()
                    };
                    let child = if index == 0 {
                        Some(callable.as_ref())
                    } else {
                        args.get(index - 1)
                    };
                    if let Some(child) = child {
                        let child_path = if index == 0 {
                            format!("{path}.callable")
                        } else {
                            format!("{path}.arg.{}", index - 1)
                        };
                        $frames.push($Frame::InvokeAfter {
                            expression,
                            index,
                            path,
                        });
                        $frames.push($Frame::Enter {
                            expression: child,
                            scope,
                            path: child_path,
                        });
                    } else {
                        $this.finish_affine_invocation(expression, &mut scope, $allow_moves)?;
                        $scopes.push(scope);
                    }
                }
                $Frame::InvokeAfter {
                    expression,
                    index,
                    path,
                } => {
                    let scope = $scopes.pop().expect("invocation child scope");
                    $frames.push($Frame::InvokeNext {
                        expression,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                $Frame::RestorePublication(enabled) => $publication.enabled = enabled,
                $Frame::Enter {
                    expression,
                    mut scope,
                    path,
                } => {
                    reject_nul_identity("resolved expression", expression.id.as_str())?;
                    if !expression.id.matches($function, &path) {
                        return Err(hir_error(format!(
                            "expression `{}` has a non-canonical identity",
                            expression.id
                        )));
                    }
                    if !$this.expression_ids.insert(expression.id.clone()) {
                        return Err(hir_error(format!(
                            "duplicate resolved expression identity `{}`",
                            expression.id
                        )));
                    }
                    $this.validate_type(&expression.ty)?;
                    let expected_path = path;
                    let path = continuation_path::after_header(expression, expected_path.len());
                    drop(expected_path);
                    match &expression.kind {
                        ResolvedExprKind::LexicalExit { value, .. } => {
                            if let Some(operand) = value {
                                $frames.push($Frame::LexicalReturn {
                                    expression,
                                    operand,
                                });
                                $frames.push($Frame::Enter {
                                    expression: operand,
                                    scope,
                                    path: format!("{path}.value"),
                                });
                            } else {
                                $this.finish_expr(
                                    expression,
                                    &ResolvedType::Never,
                                    OwnershipMode::Value,
                                )?;
                                $scopes.push(scope);
                            }
                        }
                        ResolvedExprKind::Closure { .. } => {
                            $this.validate_closure(
                                $function,
                                expression,
                                &mut scope,
                                path,
                                $allow_moves,
                            )?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::FunctionReference { target } => {
                            super::function_value::validate_reference(
                                $this.$program,
                                target,
                                &expression.ty,
                            )?;
                            $this.finish_expr(expression, &expression.ty, OwnershipMode::Value)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Invoke { .. } => {
                            super::function_value::validate_invocation(expression)?;
                            $frames.push($Frame::InvokeNext {
                                expression,
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::Int(_) => {
                            $this.finish_expr(expression, &ResolvedType::I64, OwnershipMode::Value)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Int32(_) => {
                            $this.finish_expr(expression, &ResolvedType::I32, OwnershipMode::Value)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Char(value) => {
                            if char::from_u32(*value).is_none() {
                                return Err(hir_error(
                                    "char literal bits are not a Unicode scalar value",
                                ));
                            }
                            $this.finish_expr(
                                expression,
                                &ResolvedType::Char,
                                OwnershipMode::Value,
                            )?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Uint8(_) => {
                            $this.finish_expr(expression, &ResolvedType::U8, OwnershipMode::Value)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Usize(_) => {
                            $this.finish_expr(
                                expression,
                                &ResolvedType::Usize,
                                OwnershipMode::Value,
                            )?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::BorrowPlace { operation, place } => {
                            let ty = $this.validate_borrowed_view(
                                operation,
                                place,
                                expression.span,
                                &scope,
                            )?;
                            $this.finish_expr(expression, &ty, OwnershipMode::Borrow)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::ByteRange {
                            operation,
                            source,
                            start,
                            end,
                        } => {
                            if operation.as_str() != crate::byte_ops::RANGE_ID {
                                return Err(hir_error(
                                    "byte range has an invalid compiler-owned operation",
                                ));
                            }
                            $this.byte_range_origin(source, &scope)?;
                            $frames.push($Frame::ByteRangeAfterSource {
                                expression,
                                start,
                                end,
                                path,
                            });
                            $frames.push($Frame::Enter {
                                expression: source,
                                scope,
                                path: format!("{path}.arg.0"),
                            });
                        }
                        ResolvedExprKind::ArrayU8(values) => {
                            let length = u32::try_from(values.len()).map_err(|_| {
                                hir_error("fixed-array literal length does not fit u32")
                            })?;
                            $this.finish_expr(
                                expression,
                                &ResolvedType::ArrayU8(length),
                                OwnershipMode::Value,
                            )?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::RepeatArrayU8 { count, .. } => {
                            if u64::from(*count) > crate::byte_data_capacity::MAX_ARRAY_BYTES {
                                return Err(hir_error(
                                    "fixed-array repeat exceeds the exact byte bound",
                                ));
                            }
                            $this.finish_expr(
                                expression,
                                &ResolvedType::ArrayU8(*count),
                                OwnershipMode::Value,
                            )?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Float32(bits) => {
                            $this.validate_finite_f32(*bits)?;
                            $this.finish_expr(expression, &ResolvedType::F32, OwnershipMode::Value)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Float64(bits) => {
                            $this.validate_finite_f64(*bits)?;
                            $this.finish_expr(expression, &ResolvedType::F64, OwnershipMode::Value)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Bool(_) => {
                            $this.finish_expr(
                                expression,
                                &ResolvedType::Bool,
                                OwnershipMode::Value,
                            )?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::BorrowedUtf8Literal(value) => {
                            $this.finish_static_text(expression, value)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::String(value) => {
                            $this.record_static_pool(value.len(), false)?;
                            $this.finish_expr(
                                expression,
                                &ResolvedType::String,
                                OwnershipMode::Own,
                            )?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Place(place) => {
                            let binding = scope.get(&place.root).ok_or_else(|| {
                                hir_error(format!(
                                    "resolved value `{}` is out of scope",
                                    place.root
                                ))
                            })?;
                            match (place.projections.is_empty(), binding.availability) {
                                (true, Availability::Available) => {
                                    match Self::place_availability(binding, &[]) {
                                        Availability::Available => {}
                                        Availability::Moved => {
                                            return Err(hir_error(format!(
                                                "resolved value `{}` is partially moved",
                                                place.root
                                            )));
                                        }
                                        Availability::MaybeMoved => {
                                            return Err(hir_error(format!(
                                                "resolved value `{}` may be partially moved",
                                                place.root
                                            )));
                                        }
                                    }
                                }
                                (true, Availability::Moved) => {
                                    return Err(hir_error(format!(
                                        "resolved value `{}` is used after it was moved",
                                        place.root
                                    )));
                                }
                                (true, Availability::MaybeMoved) => {
                                    return Err(hir_error(format!(
                                        "resolved value `{}` may have been moved",
                                        place.root
                                    )));
                                }
                                (false, _) => {
                                    match Self::place_availability(binding, &place.projections) {
                                        Availability::Available => {}
                                        Availability::Moved => {
                                            return Err(hir_error(format!(
                                                "resolved place rooted at `{}` is partially moved",
                                                place.root
                                            )));
                                        }
                                        Availability::MaybeMoved => {
                                            return Err(hir_error(format!(
                                                "resolved place rooted at `{}` may be conditionally moved",
                                                place.root
                                            )));
                                        }
                                    }
                                }
                            }
                            let (ty, ownership) = $this.resolve_place(place, binding)?;
                            $this.finish_expr(expression, &ty, ownership)?;
                            $scopes.push(scope);
                        }
                        ResolvedExprKind::Unary { op, value } => {
                            $frames.push($Frame::Unary {
                                expression,
                                op: *op,
                            });
                            $frames.push($Frame::Enter {
                                expression: value,
                                scope,
                                path: format!("{path}.value"),
                            });
                        }
                        ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } => {
                            let owned_generic_intrinsic = box_intrinsic::is_call(callee, instance);
                            match instance {
                                None if !type_arguments.is_empty() && !owned_generic_intrinsic => {
                                    return Err(hir_error(
                                        "monomorphic resolved call carries generic type arguments",
                                    ));
                                }
                                Some(actual)
                                    if FunctionInstanceId::derive(callee, type_arguments)
                                        != *actual =>
                                {
                                    return Err(hir_error(
                                        "resolved call instance disagrees with its template and arguments",
                                    ));
                                }
                                Some(_) if type_arguments.is_empty() => {
                                    return Err(hir_error(
                                        "generic resolved call has no concrete type arguments",
                                    ));
                                }
                                None | Some(_) => {}
                            }
                            if !owned_generic_intrinsic
                                && !generic_instance_arguments_are_admitted(
                                    $this.$program,
                                    callee,
                                    type_arguments,
                                )
                            {
                                return Err(hir_error(
                                    "resolved call has a generic type argument outside the direct-scalar or owned-record relay profile",
                                ));
                            }
                            let mut intrinsic_parameters = None;
                            let mut borrowed_parameters = None;
                            let retained_program = $this.$program;
                            let (params, return_type) = if let Some(signature) =
                                $this.intrinsic_signature(callee, type_arguments, instance, args)?
                            {
                                (signature.0, call_signature::ResultType::Owned(signature.1))
                            } else if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                                string_intrinsic::take_shape(expression, $allow_moves)?;
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "string operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(hir_error(format!(
                                        "string operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                intrinsic_parameters = Some(CallParameters::String(op));
                                (
                                    Vec::new(),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "borrowed string operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(hir_error(format!(
                                        "borrowed string operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                intrinsic_parameters = Some(CallParameters::Str(op));
                                (
                                    Vec::new(),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "byte operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(hir_error(format!(
                                        "byte operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                owned_buffer::require_admitted_chain(
                                    op,
                                    args,
                                    $this.buffer_reopen_sites.contains(&expression.id),
                                )?;
                                intrinsic_parameters = Some(CallParameters::Byte(op));
                                (
                                    Vec::new(),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "host I/O operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(hir_error(format!(
                                        "host I/O operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                match $allowed_effects {
                                    Some(allowed) if !allowed.contains(op.effect()) => {
                                        return Err(hir_error(format!(
                                            "call to `{callee}` requires undeclared effect `{}`",
                                            op.effect()
                                        )));
                                    }
                                    None => {
                                        return Err(hir_error(
                                            "contract calls effectful host I/O operation",
                                        ));
                                    }
                                    _ => {}
                                }
                                (
                                    crate::host_io_ops::resolved_params(op),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else {
                                let (params, return_type, target_effects) = if let Some(target) =
                                    retained_program.resolve_call_target(callee, instance.as_ref())
                                {
                                    borrowed_parameters = Some(target.params.as_slice());
                                    (
                                        Vec::new(),
                                        call_signature::ResultType::Borrowed(&target.return_type),
                                        std::borrow::Cow::Borrowed(target.effects.as_slice()),
                                    )
                                } else if $this.proof_generic_calls {
                                    let (params, result, effects) =
                                        generic_template::proof_signature(
                                            $this.$program,
                                            callee,
                                            instance.as_ref(),
                                            type_arguments,
                                        )?;
                                    (
                                        params,
                                        call_signature::ResultType::Owned(result),
                                        std::borrow::Cow::Owned(effects),
                                    )
                                } else {
                                    return Err(hir_error(format!(
                                        "resolved callee `{callee}` is not indexed"
                                    )));
                                };
                                let parameter_count = borrowed_parameters
                                    .map_or(params.len(), |parameters| parameters.len());
                                if args.len() != parameter_count {
                                    return Err(hir_error(format!(
                                        "call to `{callee}` has {} arguments but expects {}",
                                        args.len(),
                                        parameter_count
                                    )));
                                }
                                match $allowed_effects {
                                    Some(allowed) => {
                                        for effect in target_effects.iter() {
                                            if !allowed.contains(effect) {
                                                return Err(hir_error(format!(
                                                    "call to `{callee}` requires undeclared effect `{effect}`"
                                                )));
                                            }
                                        }
                                    }
                                    None if !target_effects.is_empty() => {
                                        return Err(hir_error(format!(
                                            "contract calls effectful function `{callee}`"
                                        )));
                                    }
                                    None => {}
                                }
                                (params, return_type)
                            };
                            let params = if let Some(parameters) = borrowed_parameters {
                                CallParameters::Borrowed(parameters)
                            } else {
                                intrinsic_parameters.unwrap_or(CallParameters::Owned(params))
                            };
                            let return_ownership = if matches!(
                                callee.as_str(),
                                crate::vec_ops::string_slot::ID
                                    | crate::vec_ops::string_slot::BYTES_ID
                            ) {
                                OwnershipMode::Borrow
                            } else {
                                $this.expected_ownership(&return_type, OwnershipMode::Own)?
                            };
                            $frames.push($Frame::CallNext {
                                expression,
                                args,
                                params,
                                return_type,
                                return_ownership,
                                index: 0,
                                scope,
                                path: path.into(),
                            });
                        }
                        ResolvedExprKind::VecFieldRead { args, .. }
                        | ResolvedExprKind::RowLoan { args, .. } => {
                            let (return_type, return_ownership, bytes) =
                                $this.validate_scoped_vec_node($function, expression, &scope)?;
                            $frames.push($Frame::CallNext {
                                expression,
                                args,
                                params: CallParameters::VecField(args),
                                return_type: call_signature::ResultType::Owned(return_type),
                                return_ownership,
                                index: 0,
                                scope,
                                path: if bytes {
                                    std::borrow::Cow::Owned(format!("{path}.arg.0"))
                                } else {
                                    std::borrow::Cow::Borrowed(path)
                                },
                            });
                        }
                        ResolvedExprKind::LiteralFormat { template, args } => {
                            $this.record_static_pool(template.len(), false)?;
                            $this.validate_literal_format_arguments($function, template, args)?;
                            $frames.push($Frame::CallNext {
                                expression,
                                args,
                                params: CallParameters::LiteralFormat(args),
                                return_type: call_signature::ResultType::Owned(
                                    ResolvedType::String,
                                ),
                                return_ownership: OwnershipMode::Own,
                                index: 0,
                                scope,
                                path: path.into(),
                            });
                        }
                        ResolvedExprKind::HostCommandCall(call) => {
                            if call.expression != expression.id
                                || call.args.len() != crate::command_io_ops::arity(call.operation)
                            {
                                return Err(hir_error(
                                    "host-command call has a non-canonical identity or shape",
                                ));
                            }
                            host_command::require_effects_view(call.operation, $allowed_effects)?;
                            $frames.push($Frame::CallNext {
                                expression,
                                args: &call.args,
                                params: CallParameters::Owned(
                                    crate::command_io_ops::resolved_params(call.operation),
                                ),
                                return_type: call_signature::ResultType::Owned(
                                    crate::command_io_ops::return_type(call.operation),
                                ),
                                return_ownership: crate::command_io_ops::result_ownership(
                                    call.operation,
                                ),
                                index: 0,
                                scope,
                                path: path.into(),
                            });
                        }
                        ResolvedExprKind::NativeRustImportCall(call) => {
                            if call.expression != expression.id {
                                return Err(hir_error(
                                    "native Rust import call has a non-canonical expression identity",
                                ));
                            }
                            let retained_program = $this.$program;
                            let import = retained_program
                                .interfaces
                                .iter()
                                .flat_map(|interface| &interface.imports)
                                .find(|import| import.id == call.import && import.native_rust)
                                .ok_or_else(|| {
                                    hir_error("native Rust import call has an unknown target")
                                })?;
                            if import.parameters.len() != call.args.len()
                                || import.result.kind != call.result
                            {
                                return Err(hir_error(
                                    "native Rust import call disagrees with its declaration",
                                ));
                            }
                            match $allowed_effects {
                                Some(allowed)
                                    if import
                                        .effects
                                        .iter()
                                        .any(|effect| !allowed.contains(effect)) =>
                                {
                                    return Err(hir_error(
                                        "native Rust import call requires an undeclared effect",
                                    ));
                                }
                                None if !import.effects.is_empty() => {
                                    return Err(hir_error(
                                        "contract calls an effectful native Rust import",
                                    ));
                                }
                                _ => {}
                            }
                            let result = call.result.value_type(&$this.$program.declarations)?;
                            $frames.push($Frame::NativeNext {
                                expression,
                                args: &call.args,
                                params: import.parameters.as_slice(),
                                result,
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::Binary { op, left, right } => {
                            $frames.push($Frame::BinaryLeft {
                                expression,
                                op: *op,
                                right,
                                path,
                            });
                            $frames.push($Frame::Enter {
                                expression: left,
                                scope,
                                path: format!("{path}.left"),
                            });
                        }
                        ResolvedExprKind::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            $frames.push($Frame::IfCondition {
                                expression,
                                then_branch,
                                else_branch,
                                path,
                            });
                            $frames.push($Frame::Enter {
                                expression: condition,
                                scope,
                                path: format!("{path}.condition"),
                            });
                        }
                        ResolvedExprKind::Block { statements, tail }
                        | ResolvedExprKind::VecConstructor {
                            statements, tail, ..
                        } => {
                            super::vec_constructor::authenticate(expression)?;
                            $this.validate_record_traversal(expression, &scope)?;
                            let outer_ids = scope.keys().cloned().collect();
                            let outer = scope.clone();
                            $frames.push($Frame::BlockNext {
                                expression,
                                statements,
                                tail,
                                index: 0,
                                scope,
                                outer,
                                outer_ids,
                                path,
                            });
                        }
                        ResolvedExprKind::ConstructRecord { record, fields } => {
                            if crate::map_ops::is_declaration(record.as_str())
                                || record.as_str() == crate::iterator_ops::ITER_ID
                                || record.as_str() == crate::list_ops::LIST_ID
                            {
                                return Err(hir_error(
                                    "iterator owner cannot be authored as a record",
                                ));
                            }
                            let declaration = $this
                                .$program
                                .declarations
                                .declaration(record)
                                .ok_or_else(|| {
                                    hir_error(format!("record `{record}` is not indexed"))
                                })?;
                            if !matches!(
                                declaration.kind,
                                DeclarationKind::Record | DeclarationKind::Class
                            ) {
                                return Err(hir_error(format!(
                                    "constructor target `{record}` is not a record or class"
                                )));
                            }
                            let expected = aggregate_metadata::record_fields($program, record)
                                .ok_or_else(|| {
                                    hir_error(format!("record `{record}` has no fields"))
                                })?;
                            let ResolvedType::Nominal {
                                declaration: instance,
                                arguments,
                            } = &expression.ty
                            else {
                                return Err(hir_error("record constructor result is not nominal"));
                            };
                            let parameters = $this
                                .$program
                                .declarations
                                .type_parameters(record)
                                .ok_or_else(|| {
                                    hir_error(format!("record `{record}` has no parameters"))
                                })?;
                            if instance != record
                                || arguments.len() != parameters.len()
                                || !super::type_reachability::record_args_ok(
                                    &$this.$program.declarations,
                                    record,
                                    arguments,
                                )
                            {
                                return Err(hir_error(format!(
                                    "constructor for `{record}` has an invalid concrete instance"
                                )));
                            }
                            $frames.push($Frame::RecordNext {
                                expression,
                                fields,
                                expected,
                                record,
                                arguments,
                                seen: BTreeSet::new(),
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::ConstructVariant {
                            variant,
                            case,
                            fields,
                        } => {
                            if expression.ty.is_compiler_byte_option() {
                                return Err(hir_error(
                                    "Option<u8> may be produced only by compiler-owned byte_get",
                                ));
                            }
                            let ResolvedType::Nominal {
                                declaration: instance,
                                arguments,
                            } = &expression.ty
                            else {
                                return Err(hir_error(
                                    "variant constructor has a non-nominal result",
                                ));
                            };
                            if instance != variant {
                                return Err(hir_error(
                                    "variant constructor result disagrees with its declaration",
                                ));
                            }
                            let declaration =
                                $this.$program.declarations.declaration(variant).ok_or_else(
                                    || hir_error(format!("variant `{variant}` is not indexed")),
                                )?;
                            if declaration.kind != DeclarationKind::Variant {
                                return Err(hir_error(format!(
                                    "constructor target `{variant}` is not a variant"
                                )));
                            }
                            let expected = aggregate_metadata::variant_cases($program, variant)
                                .and_then(|cases| cases.iter().find(|item| item.id == *case))
                                .ok_or_else(|| {
                                    hir_error(format!(
                                        "constructor for `{variant}` contains foreign case `{case}`"
                                    ))
                                })?
                                .fields
                                .as_slice();
                            $frames.push($Frame::VariantNext {
                                expression,
                                fields,
                                expected,
                                variant,
                                case,
                                arguments,
                                seen: BTreeSet::new(),
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::UpdateRecord {
                            base,
                            record,
                            fields,
                        } => {
                            $frames.push($Frame::UpdateBase {
                                expression,
                                record,
                                fields,
                                path,
                            });
                            $frames.push($Frame::Enter {
                                expression: base,
                                scope,
                                path: format!("{path}.base"),
                            });
                        }
                        ResolvedExprKind::Match {
                            scrutinee, arms, ..
                        } => {
                            $frames.push($Frame::MatchScrutinee {
                                expression,
                                arms,
                                path,
                            });
                            $frames.push($Frame::Enter {
                                expression: scrutinee,
                                scope,
                                path: format!("{path}.scrutinee"),
                            });
                        }
                        ResolvedExprKind::Project { base, field } => {
                            if matches!(&base.kind, ResolvedExprKind::Place(_)) {
                                return Err(hir_error(
                                    "place field projections must use a resolved place path",
                                ));
                            }
                            $frames.push($Frame::Project { expression, field });
                            $frames.push($Frame::Enter {
                                expression: base,
                                scope,
                                path: format!("{path}.base"),
                            });
                        }
                        // Class Inheritance v1: independently re-derive the
                        // upcast contract from the resolved source before the
                        // ancestor-typed result is accepted.
                        ResolvedExprKind::Upcast { source } => {
                            $frames.push($Frame::Upcast { expression });
                            $frames.push($Frame::Enter {
                                expression: source,
                                scope,
                                path: format!("{path}.source"),
                            });
                        }
                        // Resumable Effects v1 (issue #204): the request
                        // resolves like any other child expression; its
                        // own type and the whole node's ownership are
                        // independently re-derived on the way back up,
                        // matching every other node here.
                        ResolvedExprKind::Yield { request } => {
                            $frames.push($Frame::Yield { expression });
                            $frames.push($Frame::Enter {
                                expression: request,
                                scope,
                                path: format!("{path}.request"),
                            });
                        }
                        ResolvedExprKind::Try { operand, .. } => {
                            $frames.push($Frame::Try {
                                expression,
                                path,
                                option: false,
                            });
                            $frames.push($Frame::Enter {
                                expression: operand,
                                scope,
                                path: format!("{path}.operand"),
                            });
                        }
                        ResolvedExprKind::TryOption { operand, .. } => {
                            $frames.push($Frame::Try {
                                expression,
                                path,
                                option: true,
                            });
                            $frames.push($Frame::Enter {
                                expression: operand,
                                scope,
                                path: format!("{path}.operand"),
                            });
                        }
                    }
                }
                $Frame::Unary { expression, op } => {
                    let scope = $scopes.pop().expect("unary scope retained");
                    let ResolvedExprKind::Unary { value, .. } = &expression.kind else {
                        unreachable!()
                    };
                    // Negation keeps a numeric operand type; i64 negation is
                    // checked while IEEE-754 negation is total.
                    if matches!(op, UnaryOp::Neg)
                        && !matches!(
                            &value.ty,
                            ResolvedType::I64
                                | ResolvedType::I32
                                | ResolvedType::F32
                                | ResolvedType::F64
                        )
                    {
                        return Err(hir_error("unary operand has inconsistent resolved types"));
                    }
                    let expected = match op {
                        UnaryOp::Neg => value.ty.clone(),
                        UnaryOp::Not => ResolvedType::Bool,
                    };
                    $this.require_type(&value.ty, &expected, "unary operand")?;
                    $this.finish_expr(expression, &expected, OwnershipMode::Value)?;
                    $scopes.push(scope);
                }
                $Frame::ByteRangeAfterSource {
                    expression,
                    start,
                    end,
                    path,
                } => {
                    let scope = $scopes.pop().expect("byte range source scope retained");
                    $frames.push($Frame::ByteRangeAfterStart {
                        expression,
                        end,
                        path,
                    });
                    $frames.push($Frame::Enter {
                        expression: start,
                        scope,
                        path: format!("{path}.arg.1"),
                    });
                }
                $Frame::ByteRangeAfterStart {
                    expression,
                    end,
                    path,
                } => {
                    let scope = $scopes.pop().expect("byte range start scope retained");
                    $frames.push($Frame::ByteRangeAfterEnd { expression });
                    $frames.push($Frame::Enter {
                        expression: end,
                        scope,
                        path: format!("{path}.arg.2"),
                    });
                }
                $Frame::ByteRangeAfterEnd { expression } => {
                    let scope = $scopes.pop().expect("byte range end scope retained");
                    let ResolvedExprKind::ByteRange {
                        source, start, end, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    $this.require_type(&source.ty, &ResolvedType::SliceU8, "byte range source")?;
                    $this.require_type(&start.ty, &ResolvedType::Usize, "byte range start")?;
                    $this.require_type(&end.ty, &ResolvedType::Usize, "byte range end")?;
                    if source.ownership != OwnershipMode::Borrow
                        || start.ownership != OwnershipMode::Value
                        || end.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error("byte range operands have invalid ownership"));
                    }
                    $this.finish_expr(expression, &ResolvedType::SliceU8, OwnershipMode::Borrow)?;
                    $scopes.push(scope);
                }
            other => owned_dispatch_1!(other, $this, $function, $allow_moves, $allowed_effects, $program, $publication, $frames, $scopes, $Frame),
        }
    };
}
