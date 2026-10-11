//! HIR execution, not a precomputed sum or source-spelling shortcut.
use super::*;
impl<'s, 'a> Engine<'s, 'a> {
    pub(super) fn expression(&mut self, expression: &'a E, depth: usize) -> Result<Flat, Flow> {
        if depth > DEPTH {
            return Err(Flow::DepthExceeded);
        }
        let result = self.charge().and_then(|()| self.node(expression, depth));
        if result.is_err() && self.failed_site.is_none() {
            self.failed_site = Some(expression);
        }
        result
    }
    fn node(&mut self, expression: &'a E, depth: usize) -> Result<Flat, Flow> {
        match &expression.kind {
            K::Int(v) => Ok(Flat::Scalar(Value::Int(*v))),
            K::Usize(v) => Ok(Flat::Scalar(Value::Usize(*v))),
            K::Uint8(v) => Ok(Flat::Scalar(Value::Uint8(*v))),
            K::Bool(v) => Ok(Flat::Scalar(Value::Bool(*v))),
            K::Place(place) if place.projections.is_empty() => {
                self.read(&place.root, expression.ownership == OwnershipMode::Own)
            }
            K::Unary { op, value: operand } => {
                let Flat::Scalar(value) = self.expression(operand, depth + 1)? else {
                    return Err(refuse());
                };
                Ok(Flat::Scalar(match (op, value) {
                    (UnaryOp::Not, Value::Bool(v)) => Value::Bool(!v),
                    (UnaryOp::Neg, Value::Int(v)) => {
                        Value::Int(v.checked_neg().ok_or_else(|| {
                            Flow::Failure(super::super::super::normalize_arithmetic(
                                crate::cleanup_plan::StatusCase::NegationOverflow,
                            ))
                        })?)
                    }
                    _ => return Err(refuse()),
                }))
            }
            K::Binary {
                op,
                left: lhs,
                right: rhs,
            } => {
                let Flat::Scalar(left) = self.expression(lhs, depth + 1)? else {
                    return Err(refuse());
                };
                match (op, &left) {
                    (BinaryOp::And, Value::Bool(false)) => {
                        return Ok(Flat::Scalar(Value::Bool(false)))
                    }
                    (BinaryOp::Or, Value::Bool(true)) => {
                        return Ok(Flat::Scalar(Value::Bool(true)))
                    }
                    _ => {}
                }
                let Flat::Scalar(right) = self.expression(rhs, depth + 1)? else {
                    return Err(refuse());
                };
                super::super::super::combine(*op, left, right)
                    .ok_or_else(refuse)?
                    .map(Flat::Scalar)
                    .map_err(Flow::Failure)
            }
            K::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let Flat::Scalar(Value::Bool(condition)) = self.expression(condition, depth + 1)?
                else {
                    return Err(refuse());
                };
                self.expression(if condition { then_branch } else { else_branch }, depth + 1)
            }
            K::Block { statements, tail } => self.block(expression, statements, tail, depth),
            K::ConstructRecord { fields, .. } => self.construct(expression, fields, depth),
            K::VecFieldRead {
                element,
                field,
                bytes,
                args,
            } => {
                if *element != *self.selection.element() {
                    return Err(refuse());
                }
                self.field(field, *bytes, args, depth)
            }
            K::BorrowPlace { operation, place }
                if place.projections.is_empty()
                    && operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID =>
            {
                let Flat::Text(row) = self.read(&place.root, false)? else {
                    return Err(refuse());
                };
                if self.bytes(Span::Text(row), <[u8]>::len)? as u64
                    > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
                {
                    return Err(refuse());
                }
                Ok(Flat::Slice(Span::Text(row)))
            }
            K::Call {
                callee,
                instance: None,
                args,
                ..
            } => self.call(expression, callee.as_str(), args, depth),
            K::Match {
                scrutinee, arms, ..
            } => {
                let Flat::Scalar(Value::OptionU8(value)) = self.expression(scrutinee, depth + 1)?
                else {
                    return Err(refuse());
                };
                for arm in arms {
                    self.charge()?;
                    if arm.guard.is_some() {
                        return Err(refuse());
                    }
                    let Pattern::Variant {
                        variant,
                        case,
                        fields,
                    } = &arm.pattern
                    else {
                        return Err(refuse());
                    };
                    if variant.as_str() != crate::prelude::OPTION_ID {
                        return Err(refuse());
                    }
                    let matches = if value.is_some() {
                        case.as_str() == crate::prelude::OPTION_SOME_ID
                    } else {
                        case.as_str() == crate::prelude::OPTION_NONE_ID
                    };
                    if !matches {
                        continue;
                    }
                    let base = self.len;
                    if let Some(value) = value {
                        let [field] = fields.as_slice() else {
                            return Err(refuse());
                        };
                        if field.field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID {
                            return Err(refuse());
                        }
                        self.bind(&field.binding.id, Flat::Scalar(Value::Uint8(value)))?;
                    } else if !fields.is_empty() {
                        return Err(refuse());
                    }
                    let result = self.expression(&arm.body, depth + 1);
                    let cleanup = self.truncate(base);
                    return match result {
                        Ok(value) => {
                            cleanup?;
                            Ok(value)
                        }
                        Err(failure) => Err(failure),
                    };
                }
                Err(refuse())
            }
            _ => Err(refuse()),
        }
    }
    fn block(
        &mut self,
        expression: &'a E,
        statements: &'a [S],
        tail: &'a E,
        depth: usize,
    ) -> Result<Flat, Flow> {
        let base = self.len;
        let scope = self.registry.enter()?;
        let result = (|| {
            for statement in statements {
                self.charge()?;
                match statement {
                    S::Let { binding, value, .. } => {
                        let value = self.expression(value, depth + 1)?;
                        self.bind(&binding.id, value)?;
                    }
                    S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } => {
                        let value = self.expression(value, depth + 1)?;
                        let index = self.slot(&binding.id)?;
                        let previous = &self.slots[index].as_ref().ok_or_else(refuse)?.value;
                        if matches!(previous, Flat::Vector(true) | Flat::Row) {
                            return Err(refuse());
                        }
                        self.slots[index].as_mut().unwrap().value = value;
                    }
                    S::While {
                        condition, body, ..
                    } => loop {
                        self.charge()?;
                        let Flat::Scalar(Value::Bool(condition)) =
                            self.expression(condition, depth + 1)?
                        else {
                            return Err(refuse());
                        };
                        if !condition {
                            break;
                        }
                        self.expression(body, depth + 1)?;
                    },
                    _ => return Err(refuse()),
                }
            }
            self.expression(tail, depth + 1)
        })();
        let cleanup = self.registry.leave(scope);
        if std::ptr::eq(expression, &self.selection.function().body) {
            // Root owners stay live for the canonical publication/failure exit.
            return match result {
                Ok(value) => {
                    cleanup?;
                    Ok(value)
                }
                Err(failure) => Err(failure),
            };
        }
        let truncation = self.truncate(base);
        match result {
            Ok(value) => {
                cleanup?;
                truncation?;
                Ok(value)
            }
            Err(failure) => Err(failure),
        }
    }
    fn borrowed_vector(&mut self, expression: &'a E) -> Result<(), Flow> {
        self.charge()?;
        let K::Place(place) = &expression.kind else {
            return Err(refuse());
        };
        if !place.projections.is_empty()
            || !matches!(self.read(&place.root, false)?, Flat::Vector(false))
        {
            return Err(refuse());
        }
        Ok(())
    }
    fn call(
        &mut self,
        expression: &'a E,
        id: &str,
        args: &'a [E],
        depth: usize,
    ) -> Result<Flat, Flow> {
        self.charge()?;
        match (id, args) {
            (crate::vec_ops::WITH_CAPACITY_ID, [count]) => {
                let Flat::Scalar(Value::Usize(count)) = self.expression(count, depth + 1)? else {
                    return Err(refuse());
                };
                self.allocate(expression, count)
            }
            (crate::vec_ops::PUSH_ID, [owner, row]) => {
                let owner = self.expression(owner, depth + 1)?;
                self.staged_vector = matches!(owner, Flat::Vector(true));
                let row = self.expression(row, depth + 1)?;
                let result = self.publish(expression, owner, row);
                if result.is_ok() {
                    self.staged_vector = false;
                }
                result
            }
            (crate::vec_ops::LEN_ID, [owner]) => {
                self.borrowed_vector(owner)?;
                Ok(Flat::Scalar(Value::Usize(self.rows.len() as u64)))
            }
            (crate::vec_ops::SORT_OWNED_ID, [owner]) => {
                if !matches!(self.expression(owner, depth + 1)?, Flat::Vector(true)) {
                    return Err(refuse());
                }
                self.sort()?;
                Ok(Flat::Vector(true))
            }
            (crate::string_ops::I64_FROM_USIZE_ID, [value]) => {
                let Flat::Scalar(Value::Usize(value)) = self.expression(value, depth + 1)? else {
                    return Err(refuse());
                };
                Ok(Flat::Scalar(Value::Int(
                    i64::try_from(value).map_err(|_| conversion_failure())?,
                )))
            }
            (crate::str_ops::LEN_BYTES_ID, [text]) => {
                let Flat::Text(row) = self.expression(text, depth + 1)? else {
                    return Err(refuse());
                };
                Ok(Flat::Scalar(Value::Int(
                    self.bytes(Span::Text(row), |bytes| bytes.len() as i64)?,
                )))
            }
            (crate::byte_ops::STR_AS_BYTES_ID, [text]) => {
                let Flat::Text(row) = self.expression(text, depth + 1)? else {
                    return Err(refuse());
                };
                if self.bytes(Span::Text(row), <[u8]>::len)? as u64
                    > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
                {
                    return Err(refuse());
                }
                Ok(Flat::Slice(Span::Text(row)))
            }
            (crate::byte_ops::LEN_ID, [slice]) => {
                let Flat::Slice(span) = self.expression(slice, depth + 1)? else {
                    return Err(refuse());
                };
                Ok(Flat::Scalar(Value::Usize(
                    self.bytes(span, |bytes| bytes.len() as u64)?,
                )))
            }
            (crate::byte_ops::GET_ID, [slice, index]) => {
                let Flat::Slice(span) = self.expression(slice, depth + 1)? else {
                    return Err(refuse());
                };
                let Flat::Scalar(Value::Usize(index)) = self.expression(index, depth + 1)? else {
                    return Err(refuse());
                };
                Ok(Flat::Scalar(Value::OptionU8(self.bytes(
                    span,
                    |bytes| {
                        usize::try_from(index)
                            .ok()
                            .and_then(|index| bytes.get(index))
                            .copied()
                    },
                )?)))
            }
            _ => Err(refuse()),
        }
    }
    fn field(
        &mut self,
        field: &crate::hir::DeclarationId,
        bytes: bool,
        args: &'a [E],
        depth: usize,
    ) -> Result<Flat, Flow> {
        let [owner, index] = args else {
            return Err(refuse());
        };
        self.borrowed_vector(owner)?;
        let Flat::Scalar(Value::Usize(index)) = self.expression(index, depth + 1)? else {
            return Err(refuse());
        };
        let index = usize::try_from(index)
            .ok()
            .filter(|index| *index < self.rows.len())
            .ok_or_else(|| {
                Flow::Failure(super::super::super::owned_vec::normalize_vec(
                    crate::vec_ops::GET_OUT_OF_BOUNDS_CODE,
                ))
            })?;
        let role = self
            .selection
            .fields()
            .iter()
            .zip(self.selection.order())
            .find(|(declared, _)| declared.id == *field)
            .map(|(_, role)| role)
            .ok_or_else(refuse)?;
        match role {
            Role::Induction if !bytes => Ok(Flat::Scalar(Value::Int(self.rows[index].priority))),
            Role::Text => {
                let state = self.registry.state.as_deref().ok_or_else(refuse)?;
                let view = TextView {
                    row: index,
                    context: std::ptr::from_ref(state) as usize,
                    scope: state.scopes[state.depth],
                    generation: self.generation,
                };
                if bytes {
                    if self.bytes(Span::Text(view), <[u8]>::len)? as u64
                        > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
                    {
                        return Err(refuse());
                    }
                    Ok(Flat::Slice(Span::Text(view)))
                } else {
                    Ok(Flat::Text(view))
                }
            }
            Role::Payload => {
                let Some(OwnedBytesValue {
                    bytes: Backing::Leased(lease),
                    ..
                }) = &self.rows[index].payload
                else {
                    return Err(refuse());
                };
                let length = self.registry.len(lease)?;
                let view = self.registry.borrow(lease)?;
                Ok(Flat::Slice(Span::Payload(view, length)))
            }
            _ => Err(refuse()),
        }
    }
    pub(super) fn sort(&mut self) -> Result<(), Flow> {
        for row in &self.rows {
            let Some(OwnedBytesValue {
                bytes: Backing::Leased(lease),
                ..
            }) = &row.payload
            else {
                return Err(refuse());
            };
            let state = self.registry.state(lease.context)?;
            Registry::checked_child(state, lease)?;
            state
                .core
                .check_move(&state.children[lease.index])
                .map_err(super::super::guard)?;
        }
        // Same stable, allocation-free ordering as ordinary owned-leaf Vec.
        for index in 1..self.rows.len() {
            let mut at = index;
            while at > 0 && self.compare(at, at - 1)?.is_lt() {
                self.rows.swap(at, at - 1);
                at -= 1;
            }
        }
        self.generation = self.generation.checked_add(1).ok_or_else(refuse)?;
        Ok(())
    }
    fn compare(&mut self, a: usize, b: usize) -> Result<std::cmp::Ordering, Flow> {
        for role in self.selection.order() {
            let order = match role {
                Role::Induction => self.rows[a].priority.cmp(&self.rows[b].priority),
                Role::Text => self.rows[a]
                    .text
                    .as_deref()
                    .ok_or_else(refuse)?
                    .cmp(self.rows[b].text.as_deref().ok_or_else(refuse)?),
                Role::Payload => self.registry.compare_bytes(
                    self.rows[a].payload.as_ref().ok_or_else(refuse)?,
                    self.rows[b].payload.as_ref().ok_or_else(refuse)?,
                )?,
            };
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(std::cmp::Ordering::Equal)
    }
}

fn conversion_failure() -> Flow {
    Flow::Failure(
        crate::conformance::NormalizedStatus::try_new(
            crate::string_ops::CONVERT_STATUS_DOMAIN,
            crate::string_ops::CONVERT_OUT_OF_RANGE_CODE,
            crate::conformance::StatusClass::Adapter,
            crate::conformance::Retryability::Known(false),
        )
        .expect("compiler-owned conversion status table is valid"),
    )
}
