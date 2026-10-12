//! Checked operations execute in the same evaluator/fuel and unwind frames.
use super::*;
impl Evaluator<'_> {
    pub(in crate::interpreter) fn checked_utf8(
        &mut self,
        pieces: &[&str],
    ) -> Result<OwnedUtf8, Flow> {
        let length = pieces
            .iter()
            .try_fold(0usize, |length, piece| length.checked_add(piece.len()));
        if let Some(length) = length {
            self.charge_utf8_materialization(length)?;
        }
        self.checked_allocation
            .as_mut()
            .ok_or(Flow::Guard("checked UTF8 provider is absent"))?
            .materialize(pieces)
    }
    pub(in crate::interpreter) fn evaluate_checked_string_op(
        &mut self,
        operation: crate::string_ops::StringOp,
        arguments: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        use crate::string_ops::StringOp as S;
        if operation == S::Take {
            let [argument] = arguments else {
                return Err(Flow::Guard("checked String take arity"));
            };
            self.begin_expression(argument, depth)?;
            let ResolvedExprKind::Place(place) = &argument.kind else {
                return Err(Flow::Guard("checked String take requires named storage"));
            };
            if !place.projections.is_empty()
                || argument.ty != ResolvedType::String
                || argument.ownership != hir::OwnershipMode::Own
            {
                return Err(Flow::Guard("checked String take shape changed"));
            }
            return match take_owned_place(environment, place) {
                Some(value @ Value::CheckedString(_)) => Ok(value),
                _ => Err(Flow::Guard("checked String take owner is unavailable")),
            };
        }
        // Preserve the exact existing condition inspection: each expression is
        // charged in order, but the authenticated owner is never cloned.
        if matches!(operation, S::Len | S::IsEmpty | S::StartsWith | S::Contains)
            && arguments
                .iter()
                .all(|argument| self.string_condition_reads.contains(&argument.id))
        {
            let mut texts = [None; 2];
            for (index, argument) in arguments.iter().enumerate() {
                if index == texts.len() {
                    return Err(Flow::Guard("checked String condition arity"));
                }
                self.begin_expression(argument, depth)?;
                let ResolvedExprKind::Place(place) = &argument.kind else {
                    return Err(Flow::Guard("checked String condition is not storage"));
                };
                if !place.projections.is_empty() {
                    return Err(Flow::Guard("checked String condition is not whole storage"));
                }
                texts[index] = environment.get(&place.root).and_then(text);
            }
            return scalar(operation, texts[0], texts[1]);
        }
        if arguments.len() > 3 || arguments.len() != operation.arity() {
            return Err(Flow::Guard("checked String operation arity"));
        }
        // No owning argument Vec is constructed on this joined operation path.
        // Every prefix remains owned until all arguments have executed LTR.
        let mut values = [None, None, None];
        for (index, argument) in arguments.iter().enumerate() {
            values[index] = Some(self.evaluate(argument, environment, depth)?);
        }
        match operation {
            S::Concat => {
                let left = values[0]
                    .as_ref()
                    .and_then(text)
                    .ok_or(Flow::Guard("checked concat left owner"))?;
                let right = values[1]
                    .as_ref()
                    .and_then(text)
                    .ok_or(Flow::Guard("checked concat right owner"))?;
                let result = self.checked_utf8(&[left, right]);
                // Grouped argument ownership has committed. Both owners drop
                // before returning either the selected failure or fresh result.
                drop(values);
                result.map(Value::CheckedString)
            }
            S::FromStr => {
                let value = values[0]
                    .as_ref()
                    .and_then(borrowed_text)
                    .ok_or(Flow::Guard("checked borrowed UTF8 operand"))?;
                self.checked_utf8(&[value]).map(Value::CheckedString)
            }
            S::FromChar => {
                let Some(Value::Char(number)) = values[0].as_ref() else {
                    return Err(Flow::Guard("checked character operand"));
                };
                let character = char::from_u32(*number)
                    .ok_or(Flow::Guard("checked character is not a scalar"))?;
                let mut bytes = [0; 4];
                self.checked_utf8(&[character.encode_utf8(&mut bytes)])
                    .map(Value::CheckedString)
            }
            S::FromI64 | S::FromUsize => {
                let (magnitude, negative) = match (operation, values[0].as_ref()) {
                    (S::FromI64, Some(Value::Int(number))) => (number.unsigned_abs(), *number < 0),
                    (S::FromUsize, Some(Value::Usize(number))) => (*number as u64, false),
                    _ => return Err(Flow::Guard("checked decimal operand")),
                };
                let mut bytes = [0; 20];
                let decimal = decimal(&mut bytes, magnitude, negative);
                self.checked_utf8(&[decimal]).map(Value::CheckedString)
            }
            S::Slice | S::Trim => {
                let value = values[0]
                    .as_ref()
                    .and_then(text)
                    .ok_or(Flow::Guard("checked text owner"))?;
                let range = if operation == S::Trim {
                    crate::string_ops::trim_range(value.as_bytes())
                } else {
                    let (Some(Value::Int(start)), Some(Value::Int(end))) =
                        (values[1].as_ref(), values[2].as_ref())
                    else {
                        return Err(Flow::Guard("checked slice offsets"));
                    };
                    if *start < 0 || *start > *end || *end > value.len() as i64 {
                        return Err(self
                            .checked_allocation
                            .as_mut()
                            .unwrap()
                            .text_failure(crate::string_ops::TEXT_OUT_OF_RANGE_CODE));
                    }
                    let range = *start as usize..*end as usize;
                    if !value.is_char_boundary(range.start) || !value.is_char_boundary(range.end) {
                        return Err(self
                            .checked_allocation
                            .as_mut()
                            .unwrap()
                            .text_failure(crate::string_ops::TEXT_NOT_CHAR_BOUNDARY_CODE));
                    }
                    range
                };
                self.checked_utf8(&[&value[range]])
                    .map(Value::CheckedString)
            }
            S::LenChars => {
                let value = values[0]
                    .as_ref()
                    .and_then(text)
                    .ok_or(Flow::Guard("checked text owner"))?;
                Ok(Value::Int(value.chars().count() as i64))
            }
            S::Len | S::IsEmpty | S::StartsWith | S::Contains => scalar(
                operation,
                values[0].as_ref().and_then(text),
                values[1].as_ref().and_then(text),
            ),
            _ => Err(Flow::Guard(
                "checked String operation receiver is not joined",
            )),
        }
    }
}
fn text(value: &Value) -> Option<&str> {
    if let Value::CheckedString(text) = value {
        Some(text.as_str())
    } else {
        None
    }
}
fn scalar(
    operation: crate::string_ops::StringOp,
    left: Option<&str>,
    right: Option<&str>,
) -> Result<Value, Flow> {
    use crate::string_ops::StringOp as S;
    let left = left.ok_or(Flow::Guard("checked String owner is unavailable"))?;
    match operation {
        S::Len => Ok(Value::Int(left.len() as i64)),
        S::IsEmpty => Ok(Value::Bool(left.is_empty())),
        S::StartsWith => Ok(Value::Bool(
            left.starts_with(right.ok_or(Flow::Guard("checked prefix owner"))?),
        )),
        S::Contains => Ok(Value::Bool(
            left.contains(right.ok_or(Flow::Guard("checked needle owner"))?),
        )),
        _ => Err(Flow::Guard("checked String scalar operation is not joined")),
    }
}

// Exactly 20 bytes cover i64::MIN including its sign and u64::MAX. No
// intermediate System String exists before the physical provider is called.
fn decimal(bytes: &mut [u8; 20], mut magnitude: u64, negative: bool) -> &str {
    let mut begin = bytes.len();
    loop {
        begin -= 1;
        bytes[begin] = b'0' + (magnitude % 10) as u8;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    if negative {
        begin -= 1;
        bytes[begin] = b'-';
    }
    std::str::from_utf8(&bytes[begin..]).expect("closed decimal digits")
}
