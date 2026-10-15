//! Remaining expression families retain their original evaluator bodies.
//! Entry work is charged once by the small dispatcher before delegation.
use super::*;

impl Evaluator<'_> {
    pub(super) fn evaluate_remaining(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        match &expression.kind {
            ResolvedExprKind::LexicalExit {
                kind,
                target,
                value,
            } => self.evaluate_lexical_exit(
                expression,
                *kind,
                target,
                value.as_deref(),
                environment,
                depth,
            ),
            // Resumable Effects v1 (#204): `resumable::settle_yield` owns the
            // whole yield site -- refusal on every ordinary lane, suspension
            // on a fresh invocation, drift check + answer on a replayed one.
            ResolvedExprKind::Yield { request } => {
                let produced = self.evaluate(request, environment, depth)?;
                resumable::settle_yield(&mut self.resumption, &expression.id, produced, environment)
            }
            ResolvedExprKind::Closure { .. }
            | ResolvedExprKind::FunctionReference { .. }
            | ResolvedExprKind::Invoke { .. } => {
                self.evaluate_function_value(expression, environment, depth)
            }
            ResolvedExprKind::Int(value) => Ok(Value::Int(*value)),
            ResolvedExprKind::Int32(value) => Ok(Value::Int32(*value)),
            ResolvedExprKind::Uint8(value) => Ok(Value::Uint8(*value)),
            ResolvedExprKind::Usize(value) => Ok(Value::Usize(*value)),
            ResolvedExprKind::Char(value) => Ok(Value::Char(*value)),
            ResolvedExprKind::Float32(bits) => Ok(Value::Float32(f32::from_bits(*bits))),
            ResolvedExprKind::Float64(bits) => Ok(Value::Float64(f64::from_bits(*bits))),
            ResolvedExprKind::Bool(value) => Ok(Value::Bool(*value)),
            ResolvedExprKind::ArrayU8(values) => Ok(Value::ArrayU8(Arc::from(values.as_slice()))),
            ResolvedExprKind::RepeatArrayU8 { value, count } => {
                let length = usize::try_from(*count)
                    .map_err(|_| Flow::Guard("fixed byte array length does not fit host usize"))?;
                Ok(Value::ArrayU8(Arc::from(vec![*value; length])))
            }
            ResolvedExprKind::BorrowedUtf8Literal(value) => {
                if !value.is_backed() {
                    return Err(Flow::Guard("static text backing is absent"));
                }
                Ok(Value::BorrowedStr(BorrowedStrValue {
                    invocation_root: BorrowedRoot::SourceExprStatic(expression.id.clone()),
                    bytes: Utf8Backing::Static(value.clone()),
                }))
            }
            ResolvedExprKind::String(value) => {
                if self.checked_allocation.is_some() {
                    return Ok(Value::CheckedString(self.checked_utf8(&[value])?));
                }
                Ok(Value::String(self.materialize_utf8_copy(value)?))
            }
            ResolvedExprKind::Place(place) => {
                self.evaluate_slot_place(expression, place, environment)
            }
            ResolvedExprKind::BorrowPlace { operation, place } => {
                let op = crate::byte_ops::by_id(operation.as_str())
                    .ok_or(Flow::Guard("unknown compiler-owned byte view"))?;
                if op == crate::byte_ops::ByteOp::StringAsStr
                    || op == crate::byte_ops::ByteOp::StrAsBytes
                        && (!place.projections.is_empty()
                            || matches!(environment.get(&place.root), Some(Value::String(_))))
                {
                    return projected_string_view::read(
                        environment,
                        place,
                        op == crate::byte_ops::ByteOp::StrAsBytes,
                    );
                }
                let source = self
                    .lookup_place(environment, place)?
                    .ok_or(Flow::Guard("unresolved byte view storage root"))?;
                match (op, source) {
                    (crate::byte_ops::ByteOp::BytesAsSlice, Value::Bytes(value)) => {
                        if value.allocation == 0 || value.allocation > self.byte_allocation_count()
                        {
                            return Err(Flow::Guard(
                                "owned byte view has an invalid logical allocation",
                            ));
                        }
                        Ok(Value::BorrowedSlice(
                            self.fused.slice(&value, place.root.clone())?,
                        ))
                    }
                    (crate::byte_ops::ByteOp::ArrayAsSlice, Value::ArrayU8(bytes)) => Ok(
                        Value::BorrowedSlice(BorrowedSliceValue::whole(place.root.clone(), bytes)),
                    ),
                    (crate::byte_ops::ByteOp::StrAsBytes, Value::BorrowedStr(value)) => {
                        string_slot::validate_slice_extent(&value.bytes)?;
                        Ok(Value::BorrowedSlice(BorrowedSliceValue::from_view(
                            value.invocation_root,
                            value.bytes,
                        )))
                    }
                    _ => Err(Flow::Guard("ill-typed compiler-owned byte view")),
                }
            }
            ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(Flow::Guard("unknown compiler-owned byte range"));
                }
                let source = self.evaluate(source, environment, depth)?;
                let start = self.evaluate(start, environment, depth)?;
                let end = self.evaluate(end, environment, depth)?;
                let (Value::BorrowedSlice(source), Value::Usize(start), Value::Usize(end)) =
                    (source, start, end)
                else {
                    return Err(Flow::Guard("ill-typed compiler-owned byte range"));
                };
                if start > end {
                    return Err(Flow::Failure(normalize_byte_range(
                        crate::byte_ops::RANGE_START_AFTER_END_CODE,
                    )));
                }
                let Some((start, end)) = usize::try_from(start).ok().zip(usize::try_from(end).ok())
                else {
                    return Err(Flow::Failure(normalize_byte_range(
                        crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
                    )));
                };
                if end > source.read(&mut self.fused, <[u8]>::len)? {
                    return Err(Flow::Failure(normalize_byte_range(
                        crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
                    )));
                }
                Ok(Value::BorrowedSlice(source.range(start, end)))
            }
            ResolvedExprKind::Unary { op, value } => {
                let inner = self.evaluate(value, environment, depth)?;
                match (*op, inner) {
                    (UnaryOp::Neg, Value::Int(inner)) => inner.checked_neg().map_or_else(
                        || {
                            Err(Flow::Failure(normalize_arithmetic(
                                StatusCase::NegationOverflow,
                            )))
                        },
                        |negated| Ok(Value::Int(negated)),
                    ),
                    (UnaryOp::Neg, Value::Int32(inner)) => inner.checked_neg().map_or_else(
                        || {
                            Err(Flow::Failure(normalize_arithmetic(
                                StatusCase::NegationOverflow,
                            )))
                        },
                        |negated| Ok(Value::Int32(negated)),
                    ),
                    (UnaryOp::Neg, Value::Float32(inner)) => Ok(Value::Float32(-inner)),
                    (UnaryOp::Neg, Value::Float64(inner)) => Ok(Value::Float64(-inner)),
                    (UnaryOp::Not, Value::Bool(inner)) => Ok(Value::Bool(!inner)),
                    _ => Err(Flow::Guard("unsupported unary operand")),
                }
            }
            ResolvedExprKind::HostCommandCall(call) => match call.operation {
                fs if crate::filesystem_ops::is_filesystem(fs) => {
                    self.evaluate_filesystem_operation(call, environment, depth)
                }
                Operation::NetConnect
                | Operation::NetSend
                | Operation::NetRecv
                | Operation::NetStreamStdout
                | Operation::NetWait
                | Operation::NetClose
                | Operation::NetTlsConnect
                | Operation::NetListen
                | Operation::NetAccept
                | Operation::NetTlsAccept
                | Operation::HttpsGet
                | Operation::HttpsPost
                | Operation::NetCloseListener => {
                    self.evaluate_network_operation(call, environment, depth)
                }
                Operation::ArgsLen => {
                    if !call.args.is_empty() {
                        return Err(Flow::Guard("invalid args_len arity"));
                    }
                    let input = self.command_input.as_ref().ok_or(Flow::Guard(
                        "args_len reached an evaluator without command input",
                    ))?;
                    Ok(Value::Usize(input.arguments.len() as u64))
                }
                env if environment::handles(env) => {
                    self.evaluate_environment_operation(call, environment, depth)
                }
                Operation::ArgUtf8 => {
                    let [argument] = call.args.as_slice() else {
                        return Err(Flow::Guard("invalid arg_utf8 arity"));
                    };
                    let index = match self.evaluate(argument, environment, depth)? {
                        Value::Usize(value) => usize::try_from(value).ok(),
                        _ => return Err(Flow::Guard("ill-typed arg_utf8 index")),
                    };
                    let bytes = index
                        .and_then(|index| {
                            self.command_input
                                .as_ref()
                                .and_then(|input| input.arguments.get(index))
                        })
                        .cloned()
                        .ok_or_else(|| {
                            Flow::Failure(normalize_command_input(
                                crate::command_io_ops::ARG_INDEX_OUT_OF_BOUNDS,
                            ))
                        })?;
                    if std::str::from_utf8(bytes.as_ref()).is_err() {
                        return Err(Flow::Failure(normalize_command_input(
                            crate::command_io_ops::ARG_INVALID_UTF8,
                        )));
                    }
                    Ok(Value::BorrowedStr(BorrowedStrValue {
                        invocation_root: ValueId::intrinsic_parameter(
                            crate::command_io_ops::ARG_UTF8_ID,
                            usize::MAX,
                        )
                        .into(),
                        bytes: bytes.into(),
                    }))
                }
                Operation::StdinRead => {
                    if !call.args.is_empty() {
                        return Err(Flow::Guard("invalid stdin_read arity"));
                    }
                    let input = self.command_input.as_ref().ok_or(Flow::Guard(
                        "stdin_read reached an evaluator without command input",
                    ))?;
                    if input.stdin_consumed {
                        return Err(Flow::Failure(normalize_command_input(
                            crate::command_io_ops::STDIN_READ_FAILED,
                        )));
                    }
                    let length = u64::try_from(input.stdin.len()).map_err(|_| {
                        Flow::Failure(normalize_command_input(
                            crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                        ))
                    })?;
                    if length > crate::command_io_ops::MAX_INPUT_BYTES {
                        return Err(Flow::Failure(normalize_command_input(
                            crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                        )));
                    }
                    let next_count = self.next_byte_allocation.checked_add(1).ok_or_else(|| {
                        Flow::Failure(normalize_command_input(
                            crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                        ))
                    })?;
                    let next_payload =
                        self.allocated_byte_payload
                            .checked_add(length)
                            .ok_or_else(|| {
                                Flow::Failure(normalize_command_input(
                                    crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                                ))
                            })?;
                    if next_count > crate::byte_data_capacity::MAX_BYTES_COPY_SITES
                        || next_payload > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
                    {
                        return Err(Flow::Failure(normalize_command_input(
                            crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                        )));
                    }
                    let bytes = Arc::<[u8]>::from(input.stdin.as_ref());
                    self.next_byte_allocation = next_count;
                    self.allocated_byte_payload = next_payload;
                    self.command_input
                        .as_mut()
                        .expect("command input presence was checked")
                        .stdin_consumed = true;
                    Ok(Value::Bytes(OwnedBytesValue {
                        allocation: next_count,
                        bytes: bytes.into(),
                    }))
                }
                Operation::StderrWrite | Operation::StdoutAppend | Operation::StderrAppend => {
                    let [argument] = call.args.as_slice() else {
                        return Err(Flow::Guard("invalid command output arity"));
                    };
                    let value = self.evaluate(argument, environment, depth)?;
                    let Value::BorrowedSlice(value) = value else {
                        return Err(Flow::Guard("ill-typed command output operand"));
                    };
                    let bytes = value.bytes();
                    let stdout_length = self.stdout_transcript.as_ref().map_or(0, Vec::len);
                    let stderr_length = self.stderr_transcript.as_ref().map_or(0, Vec::len);
                    let combined = stdout_length
                        .checked_add(stderr_length)
                        .and_then(|length| length.checked_add(bytes.len()))
                        .ok_or(Flow::Guard("command transcript length overflowed"))?;
                    if combined > crate::command_io_ops::MAX_OUTPUT_BYTES as usize {
                        if matches!(call.operation, Operation::StderrWrite) {
                            return Err(Flow::Guard(
                                "combined command transcript exceeds verified capacity",
                            ));
                        }
                        return Err(Flow::Failure(normalize_command_output(
                            crate::command_io_ops::OUTPUT_CAPACITY_EXCEEDED,
                        )));
                    }
                    match call.operation {
                        Operation::StdoutAppend => self
                            .stdout_transcript
                            .as_mut()
                            .ok_or(Flow::Guard(
                                "stdout_append reached an evaluator without command output",
                            ))?
                            .extend_from_slice(bytes),
                        Operation::StderrWrite | Operation::StderrAppend => self
                            .stderr_transcript
                            .as_mut()
                            .ok_or(Flow::Guard(
                                "stderr output reached an evaluator without command output",
                            ))?
                            .extend_from_slice(bytes),
                        _ => unreachable!("command output operation was matched above"),
                    }
                    Ok(Value::Usize(bytes.len() as u64))
                }
                _ => Err(Flow::Guard("unknown host operation")),
            },
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let mut values = BTreeMap::new();
                for field in fields {
                    let value = self.evaluate(&field.value, environment, depth)?;
                    if values.insert(field.field.clone(), value).is_some() {
                        return Err(Flow::Guard(
                            "record construction repeated an authenticated field identity",
                        ));
                    }
                }
                Ok(Value::Record(Arc::new(OwnedRecordValue {
                    record: record.clone(),
                    fields: values,
                })))
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                if !nested_owned::construct_ok(self.declarations, expression) {
                    return Err(Flow::Guard(
                        "variant construction is outside owned byte variant v1",
                    ));
                }
                let ResolvedType::Nominal {
                    declaration: concrete_variant,
                    ..
                } = &expression.ty
                else {
                    return Err(Flow::Guard("owned byte variant type is not nominal"));
                };
                if concrete_variant != variant {
                    return Err(Flow::Guard(
                        "variant constructor identity disagrees with its concrete type",
                    ));
                }
                let declared_fields =
                    concrete_variant_case_fields(self.declarations, &expression.ty, case).ok_or(
                        Flow::Guard("variant constructor references an unauthenticated case"),
                    )?;
                if fields.len() != declared_fields.len() {
                    return Err(Flow::Guard(
                        "variant constructor field inventory is incomplete",
                    ));
                }
                let mut values = BTreeMap::new();
                for field in fields {
                    let declared_ty = declared_fields
                        .iter()
                        .find_map(|(field_id, ty)| (field_id == &field.field).then_some(ty))
                        .ok_or(Flow::Guard(
                            "variant constructor references an unauthenticated field",
                        ))?;
                    let value = self.evaluate(&field.value, environment, depth)?;
                    if !self.value_has_type(&value, declared_ty) {
                        return Err(Flow::Guard(
                            "variant constructor payload disagrees with its concrete field type",
                        ));
                    }
                    if values.insert(field.field.clone(), value).is_some() {
                        return Err(Flow::Guard(
                            "variant construction repeated an authenticated field identity",
                        ));
                    }
                }
                Ok(Value::Variant(Arc::new(OwnedVariantValue {
                    ty: expression.ty.clone(),
                    variant: variant.clone(),
                    case: case.clone(),
                    fields: values,
                })))
            }
            ResolvedExprKind::VecFieldRead {
                origin: _,
                element,
                field,
                bytes,
                args,
            } => self.evaluate_vec_field(element, field, *bytes, args, environment, depth),
            // Only the call boundary stages a row loan, against its parameter.
            ResolvedExprKind::RowLoan { .. } => Err(Flow::Guard(
                "row loan reached evaluation outside its call argument slot",
            )),
            ResolvedExprKind::LiteralFormat { template, args } => {
                self.evaluate_literal_format(template, args, environment, depth)
            }
            // Refutable Match v1: one scrutinee evaluation, arms tested in
            // order, guards evaluated once after their pattern matched, and
            // failing guards fall through. Every evaluated node charges fuel
            // through the ordinary recursive `evaluate` calls.
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                let staged = if *mode == hir::ResolvedMatchMode::Borrow {
                    // Borrow-mode resolution authenticates an unprojected
                    // named place. Charge that expression node, but retain
                    // the environment's owner and stage only an Arc alias.
                    self.charge()?;
                    let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                        return Err(Flow::Guard(
                            "borrowed record match has a non-place scrutinee",
                        ));
                    };
                    if !place.projections.is_empty() {
                        return Err(Flow::Guard(
                            "borrowed record match has a projected scrutinee",
                        ));
                    }
                    self.lookup(environment, &place.root)?
                        .ok_or(Flow::Guard("borrowed record owner is unavailable"))?
                } else {
                    self.evaluate(scrutinee, environment, depth)?
                };
                if let Value::Record(record) = staged {
                    let [arm] = arms.as_slice() else {
                        return Err(Flow::Guard(
                            "owned-byte record match does not have exactly one arm",
                        ));
                    };
                    if arm.guard.is_some() {
                        return Err(Flow::Guard(
                            "owned-byte record match unexpectedly has a guard",
                        ));
                    }
                    let crate::hir::ResolvedMatchPattern::Record {
                        record: pattern_record,
                        fields,
                        ..
                    } = &arm.pattern
                    else {
                        return Err(Flow::Guard(
                            "owned-byte record match has a non-record pattern",
                        ));
                    };
                    if pattern_record != &record.record || fields.len() != record.fields.len() {
                        return Err(Flow::Guard(
                            "owned-byte record pattern disagrees with its runtime carrier",
                        ));
                    }
                    let mut bindings = Vec::new();
                    match mode {
                        hir::ResolvedMatchMode::Own => {
                            bind_owned_pattern(
                                self.declarations,
                                &scrutinee.ty,
                                record,
                                fields,
                                &mut bindings,
                            )?;
                        }
                        hir::ResolvedMatchMode::Borrow => {
                            bind_borrowed_pattern(
                                self.declarations,
                                &scrutinee.ty,
                                &record,
                                fields,
                                &mut bindings,
                            )?;
                        }
                        hir::ResolvedMatchMode::Value => {
                            nested_owned::bind_copy_record_pattern(
                                self.declarations,
                                expression,
                                &record,
                                fields,
                                &mut bindings,
                            )?;
                        }
                    }
                    let base = environment.len();
                    environment.extend(bindings);
                    let outcome = self.evaluate(&arm.value, environment, depth);
                    environment.truncate(base);
                    return outcome;
                }
                if let Value::Variant(variant) = staged {
                    if arms.iter().any(|arm| arm.guard.is_some()) {
                        return self.evaluate_guarded_copy_variant(
                            *mode,
                            scrutinee,
                            arms,
                            &variant,
                            environment,
                            depth,
                        );
                    }
                    let agg = nested_owned::bc_match(self.declarations, *mode, &scrutinee.ty, arms)
                        || variant_admission::is_admitted_copy_scalar_variant(
                            self.declarations,
                            &scrutinee.ty,
                        )
                        || crate::list_ops::is_step(&scrutinee.ty);
                    if !nested_owned::variant_ok(self.declarations, *mode, &scrutinee.ty, arms) {
                        return Err(Flow::Guard(
                            "owned byte variant match is outside the authenticated profile",
                        ));
                    }
                    let ResolvedType::Nominal {
                        declaration: expected_variant,
                        ..
                    } = &scrutinee.ty
                    else {
                        return Err(Flow::Guard("owned byte variant scrutinee is not nominal"));
                    };
                    if variant.ty != scrutinee.ty || &variant.variant != expected_variant {
                        return Err(Flow::Guard(
                            "owned byte variant runtime carrier disagrees with its scrutinee",
                        ));
                    }
                    // A trailing `_` arm of a Value-mode match selects every
                    // case no earlier arm named; it binds nothing.
                    if let Some(arm) = arms.iter().find(|arm| {
                        matches!(arm.pattern, hir::ResolvedMatchPattern::Wildcard)
                            || nested_owned::arm_case_patterns(&arm.pattern).is_some_and(|patterns| {
                                patterns.iter().any(|pattern| matches!(pattern, hir::ResolvedMatchPattern::Variant { case, .. } if case == &variant.case))
                            })
                    }) {
                        if matches!(arm.pattern, hir::ResolvedMatchPattern::Wildcard) {
                            if *mode != hir::ResolvedMatchMode::Value {
                                return Err(Flow::Guard(
                                    "variant wildcard arm reached a non-value match",
                                ));
                            }
                            return self.evaluate(&arm.value, environment, depth);
                        }
                    }
                    let (arm, pattern) = arms
                        .iter()
                        .find_map(|arm| {
                            nested_owned::arm_case_patterns(&arm.pattern)?
                                .iter()
                                .find(|pattern| matches!(pattern, hir::ResolvedMatchPattern::Variant { case, .. } if case == &variant.case))
                                .map(|pattern| (arm, pattern))
                        })
                        .ok_or(Flow::Guard(
                            "owned byte variant active case selected no exhaustive arm",
                        ))?;
                    let hir::ResolvedMatchPattern::Variant {
                        variant: pattern_variant,
                        case: pattern_case,
                        fields,
                    } = pattern
                    else {
                        unreachable!("admitted owned byte variant arm is a variant pattern")
                    };
                    if pattern_variant != &variant.variant || pattern_case != &variant.case {
                        return Err(Flow::Guard(
                            "owned byte variant pattern identity disagrees with its carrier",
                        ));
                    }
                    let declared_fields =
                        concrete_variant_case_fields(self.declarations, &variant.ty, &variant.case)
                            .ok_or(Flow::Guard(
                                "owned byte variant carrier has an unauthenticated active case",
                            ))?;
                    if fields.len() != declared_fields.len()
                        || variant.fields.len() != declared_fields.len()
                    {
                        return Err(Flow::Guard(
                            "owned byte variant active payload inventory is inconsistent",
                        ));
                    }
                    let mut bindings = Vec::with_capacity(fields.len());
                    match mode {
                        hir::ResolvedMatchMode::Own => {
                            let mut variant = Arc::try_unwrap(variant).map_err(|_| {
                                Flow::Guard("owned byte variant still has a live alias at transfer")
                            })?;
                            for field in fields {
                                let declared_ty = declared_fields
                                    .iter()
                                    .find_map(|(field_id, ty)| {
                                        (field_id == &field.field).then_some(ty)
                                    })
                                    .ok_or(Flow::Guard(
                                        "owned byte variant pattern references an unauthenticated field",
                                    ))?;
                                let value =
                                    variant.fields.remove(&field.field).ok_or(Flow::Guard(
                                        "owned byte variant pattern references an absent payload",
                                    ))?;
                                if !self.value_has_type(&value, declared_ty) {
                                    return Err(Flow::Guard(
                                        "owned byte variant payload type changed before transfer",
                                    ));
                                }
                                bindings.push((field.binding.id.clone(), value));
                            }
                            if !variant.fields.is_empty() {
                                return Err(Flow::Guard(
                                    "owned byte variant transfer left unauthenticated payloads",
                                ));
                            }
                        }
                        hir::ResolvedMatchMode::Borrow => {
                            for field in fields {
                                let declared_ty = declared_fields
                                    .iter()
                                    .find_map(|(field_id, ty)| {
                                        (field_id == &field.field).then_some(ty)
                                    })
                                    .ok_or(Flow::Guard(
                                        "borrowed byte variant pattern references an unauthenticated field",
                                    ))?;
                                let value = variant.fields.get(&field.field).ok_or(Flow::Guard(
                                    "borrowed byte variant pattern references an absent payload",
                                ))?;
                                if !self.value_has_type(value, declared_ty) {
                                    return Err(Flow::Guard(
                                        "borrowed byte variant payload type changed before aliasing",
                                    ));
                                }
                                bindings.push((field.binding.id.clone(), self.clone_value(value)?));
                            }
                        }
                        hir::ResolvedMatchMode::Value if agg => {
                            bindings = nested_owned::bc_bind_fields(
                                self,
                                &declared_fields,
                                fields,
                                &variant,
                            )?;
                        }
                        hir::ResolvedMatchMode::Value => {
                            if !is_admitted_fieldless_variant(self.declarations, &variant.ty)
                                || !fields.is_empty()
                                || !variant.fields.is_empty()
                            {
                                return Err(Flow::Guard(
                                    "owned byte variant reached a plain value match",
                                ));
                            }
                        }
                    }
                    let base = environment.len();
                    environment.extend(bindings);
                    let outcome = self.evaluate(&arm.value, environment, depth);
                    environment.truncate(base);
                    return outcome;
                }
                for arm in arms {
                    let mut aggregate_bindings = Vec::new();
                    let selected = match &arm.pattern {
                        crate::hir::ResolvedMatchPattern::Wildcard => true,
                        crate::hir::ResolvedMatchPattern::Binding(binding) => {
                            let _ = binding;
                            true
                        }
                        crate::hir::ResolvedMatchPattern::Literal(value) => {
                            pattern_value_matches(&staged, *value)
                        }
                        crate::hir::ResolvedMatchPattern::Or(alternatives) => {
                            let mut matched = false;
                            for alternative in alternatives {
                                match alternative {
                                    crate::hir::ResolvedMatchPattern::Literal(value) => {
                                        matched |= pattern_value_matches(&staged, *value);
                                    }
                                    _ => {
                                        return Err(Flow::Guard(
                                            "or-pattern alternative is not a literal",
                                        ));
                                    }
                                }
                            }
                            matched
                        }
                        crate::hir::ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            if variant.as_str() != crate::prelude::OPTION_ID {
                                return Err(Flow::Guard(
                                    "non-Option variant reached byte evaluation",
                                ));
                            }
                            match (&staged, case.as_str()) {
                                (Value::OptionU8(None), crate::prelude::OPTION_NONE_ID)
                                    if fields.is_empty() =>
                                {
                                    true
                                }
                                (Value::OptionU8(Some(value)), crate::prelude::OPTION_SOME_ID)
                                    if fields.len() == 1
                                        && fields[0].field.as_str()
                                            == crate::prelude::OPTION_SOME_VALUE_ID =>
                                {
                                    aggregate_bindings
                                        .push((fields[0].binding.id.clone(), Value::Uint8(*value)));
                                    true
                                }
                                (Value::OptionU8(_), _) => false,
                                (Value::OptionI64(None), crate::prelude::OPTION_NONE_ID)
                                    if fields.is_empty() =>
                                {
                                    true
                                }
                                (Value::OptionI64(Some(value)), crate::prelude::OPTION_SOME_ID)
                                    if fields.len() == 1
                                        && fields[0].field.as_str()
                                            == crate::prelude::OPTION_SOME_VALUE_ID =>
                                {
                                    aggregate_bindings
                                        .push((fields[0].binding.id.clone(), Value::Int(*value)));
                                    true
                                }
                                (Value::OptionI64(_), _) => false,
                                _ => {
                                    return Err(Flow::Guard(
                                        "variant pattern has non-variant value",
                                    ));
                                }
                            }
                        }
                        crate::hir::ResolvedMatchPattern::Record { .. } => {
                            return Err(Flow::Guard(
                                "aggregate match shape reached scalar evaluation",
                            ));
                        }
                    };
                    if !selected {
                        continue;
                    }
                    let aggregate_count = aggregate_bindings.len();
                    environment.extend(aggregate_bindings);
                    // Binding arms capture the staged scrutinee value.
                    let mut has_bound_value = false;
                    if let crate::hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                        let bound_value = self.clone_value(&staged)?;
                        environment.push((binding.id.clone(), bound_value));
                        has_bound_value = true;
                    }
                    let guard_ok = match &arm.guard {
                        Some(guard) => match self.evaluate(guard.as_ref(), environment, depth)? {
                            Value::Bool(flag) => flag,
                            _ => return Err(Flow::Guard("non-boolean match guard")),
                        },
                        None => true,
                    };
                    if !guard_ok {
                        if has_bound_value {
                            environment.pop();
                        }
                        environment.truncate(environment.len().saturating_sub(aggregate_count));
                        continue;
                    }
                    let outcome = self.evaluate(&arm.value, environment, depth);
                    if has_bound_value {
                        environment.pop();
                    }
                    environment.truncate(environment.len().saturating_sub(aggregate_count));
                    return outcome;
                }
                Err(Flow::Guard("refutable match selected no arm"))
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                let base = self.evaluate(base, environment, depth)?;
                let Value::Record(base) = base else {
                    return Err(Flow::Guard("record update base is not a record carrier"));
                };
                if &base.record != record {
                    return Err(Flow::Guard("record update base identity is inconsistent"));
                }
                let mut replacements = Vec::with_capacity(fields.len());
                for field in fields {
                    let value = self.evaluate(&field.value, environment, depth)?;
                    replacements.push((field.field.clone(), value));
                }
                update_owned_record(self.declarations, &expression.ty, base, replacements)
            }
            ResolvedExprKind::Try { .. } => self.evaluate_owned_try(expression, environment, depth),
            ResolvedExprKind::Project { .. } => {
                self.evaluate_copy_projection(expression, environment, depth)
            }
            ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::NativeRustImportCall(_) => Err(Flow::Guard(
                "aggregate/import/match/try shape reached evaluation",
            )),
            ResolvedExprKind::Binary { .. }
            | ResolvedExprKind::If { .. }
            | ResolvedExprKind::Call { .. }
            | ResolvedExprKind::Block { .. }
            | ResolvedExprKind::VecConstructor { .. } => {
                unreachable!("recursive expression dispatched by exact kind")
            }
        }
    }
}
