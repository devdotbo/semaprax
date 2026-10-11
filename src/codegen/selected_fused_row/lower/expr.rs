//! Closed suffix expressions. Checked scalar failures select the same ordinary
//! status domains and codes and leave through the plan's exit for that exact
//! status source; leased views reach only the authenticated length and byte
//! observations. Any other consumer is a refused carrier crossing.
use super::*;
use crate::ast::BinaryOp;
use crate::hir::{ResolvedMatchArm, ResolvedMatchPattern};

impl Lower<'_, '_> {
    /// Lower one expression, then apply the canonical transitions keyed by it.
    pub(super) fn expression(&mut self, e: &E) -> Result<Value, Diagnostic> {
        if self.depth >= DEPTH {
            return Err(error(
                "selected expression nesting exceeds the lowering bound",
            ));
        }
        self.depth += 1;
        let value = self.expression_inner(e);
        self.depth -= 1;
        let value = value?;
        self.apply(&e.id)?;
        Ok(value)
    }
    fn expression_inner(&mut self, e: &E) -> Result<Value, Diagnostic> {
        let scalar = Kind::scalar(&e.ty);
        match &e.kind {
            K::Int(value) if scalar == Some(Kind::I64) => {
                Ok(Value::Scalar(crate::codegen::c_i64(*value), Kind::I64))
            }
            K::Usize(value) if scalar == Some(Kind::Usize) => {
                Ok(Value::Scalar(format!("UINT64_C({value})"), Kind::Usize))
            }
            K::Uint8(value) if scalar == Some(Kind::U8) => {
                Ok(Value::Scalar(format!("UINT8_C({value})"), Kind::U8))
            }
            K::Bool(value) if scalar == Some(Kind::Bool) => Ok(Value::Scalar(
                if *value { "true" } else { "false" }.to_owned(),
                Kind::Bool,
            )),
            K::Place(place) if place.projections.is_empty() => self.place(e, &place.root),
            K::Block { statements, tail } => {
                self.scopes.push(Vec::new());
                for statement in statements {
                    self.statement(statement)?;
                }
                let value = self.expression(tail)?;
                let value = match value {
                    Value::Scalar(code, kind) => Value::Scalar(self.materialize(code, kind), kind),
                    Value::Leased { ref name, .. }
                        if self.scopes.last().is_some_and(|scope| scope.contains(name)) =>
                    {
                        return Err(error("leased view escapes its lexical iteration"));
                    }
                    Value::Leased {
                        temporary: true, ..
                    } => return Err(error("temporary leased view escapes its consumer")),
                    other => other,
                };
                self.close_scope()?;
                self.place_region(e)?;
                Ok(value)
            }
            K::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let kind = scalar.ok_or_else(|| error("selected if-expression is not scalar"))?;
                let condition = self.scalar(condition, Kind::Bool)?;
                let result = self.temp(kind);
                self.open(&format!("if ({condition}) {{"));
                let then_value = self.scalar(then_branch, kind)?;
                self.line(&format!("{result} = {then_value};"));
                self.close("} else {");
                self.indent += 1;
                let else_value = self.scalar(else_branch, kind)?;
                self.line(&format!("{result} = {else_value};"));
                self.close("}");
                Ok(Value::Scalar(result, kind))
            }
            K::Binary { op, left, right } => self.binary(e, *op, left, right),
            K::VecFieldRead {
                element,
                field,
                bytes,
                args,
            } => self.field_read(e, element, field, *bytes, args),
            K::BorrowPlace { operation, place }
                if operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
                    && place.projections.is_empty() =>
            {
                if self.carrier(e)? != Carrier::OrdinarySlice {
                    return Err(error(
                        "str_as_bytes view does not have an ordinary carrier fact",
                    ));
                }
                let (name, kind) = self.lookup(&place.root)?;
                if kind != Kind::Text {
                    return Err(error("str_as_bytes source is not an ordinary String view"));
                }
                Ok(Value::Text(name.to_owned()))
            }
            K::Match {
                scrutinee, arms, ..
            } => self.option_byte(e, scrutinee, arms),
            K::Call {
                callee,
                instance: None,
                type_arguments,
                args,
            } => self.call(e, callee.as_str(), type_arguments, args),
            _ => Err(error(
                "expression is outside the closed selected suffix domain",
            )),
        }
    }
    fn place(&mut self, e: &E, root: &ValueId) -> Result<Value, Diagnostic> {
        let (name, kind) = self.lookup(root)?;
        let name = name.to_owned();
        match kind {
            Kind::Rows if self.is_vector(&e.ty) => Ok(Value::Rows(name)),
            Kind::Text if self.carrier(e)? != Carrier::LeasedSlice => Ok(Value::Text(name)),
            Kind::Leased if self.carrier(e)? == Carrier::LeasedSlice => Ok(Value::Leased {
                name,
                temporary: false,
            }),
            kind if Kind::scalar(&e.ty) == Some(kind) => Ok(Value::Scalar(name, kind)),
            _ => Err(error("selected place kind disagrees with its typed read")),
        }
    }
    fn binary(&mut self, e: &E, op: BinaryOp, left: &E, right: &E) -> Result<Value, Diagnostic> {
        let kind = Kind::scalar(&e.ty).ok_or_else(|| error("selected binary is not scalar"))?;
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            if kind != Kind::Bool {
                return Err(error("selected boolean operator is not bool"));
            }
            // Lazy right operand: it executes only when required.
            let left = self.scalar(left, Kind::Bool)?;
            let result = self.temp(Kind::Bool);
            self.line(&format!("{result} = {left};"));
            self.open(&if op == BinaryOp::And {
                format!("if ({result}) {{")
            } else {
                format!("if (!{result}) {{")
            });
            let right = self.scalar(right, Kind::Bool)?;
            self.line(&format!("{result} = {right};"));
            self.close("}");
            return Ok(Value::Scalar(result, Kind::Bool));
        }
        let operand = Kind::scalar(&left.ty)
            .filter(|operand| Kind::scalar(&right.ty) == Some(*operand))
            .ok_or_else(|| error("selected binary operands are not one scalar kind"))?;
        // Snapshot the left operand: a right block operand may assign its binding.
        let left = self.scalar(left, operand)?;
        let snapshot = self.temp(operand);
        self.line(&format!("{snapshot} = {left};"));
        let left = snapshot;
        let right = self.scalar(right, operand)?;
        let comparison = match op {
            BinaryOp::Eq => Some("=="),
            BinaryOp::Ne => Some("!="),
            BinaryOp::Lt => Some("<"),
            BinaryOp::Le => Some("<="),
            BinaryOp::Gt => Some(">"),
            BinaryOp::Ge => Some(">="),
            _ => None,
        };
        if let Some(symbol) = comparison {
            if kind != Kind::Bool {
                return Err(error("selected comparison is not bool"));
            }
            return Ok(Value::Scalar(
                format!("({left} {symbol} {right})"),
                Kind::Bool,
            ));
        }
        let (builtin, code, message) = match op {
            BinaryOp::Add => ("add", "ADD", "addition overflow"),
            BinaryOp::Sub => ("sub", "SUB", "subtraction overflow"),
            BinaryOp::Mul if operand == Kind::I64 => ("mul", "MUL", "multiplication overflow"),
            _ => {
                return Err(error(
                    "selected arithmetic operator is outside the closed domain",
                ))
            }
        };
        if kind != operand || !matches!(operand, Kind::I64 | Kind::Usize) {
            return Err(error("selected arithmetic is not i64 or usize"));
        }
        let result = self.temp(kind);
        let failure = self.fail(
            &e.id,
            &format!(
                "spx_status = spx_rt_arithmetic_failure(spx_ctx, SPX_STATUS_ARITHMETIC_{code}_OVERFLOW, \"{message}\");"
            ),
        )?;
        self.line(&format!(
            "if (__builtin_{builtin}_overflow({left}, {right}, &{result})) {failure}"
        ));
        Ok(Value::Scalar(result, kind))
    }
    /// Authenticated row read. Out-of-range index is ordinary Vec failure 2
    /// through the plan's exit for this read.
    fn row(
        &mut self,
        site: &crate::hir::ExpressionId,
        rows: &str,
        index: &str,
    ) -> Result<String, Diagnostic> {
        let row = self.temp_row();
        let failure = self.fail(site, "spx_status = spx_vec_failure(spx_ctx, UINT32_C(2));")?;
        self.line(&format!(
            "if (!spx_fused_rows_at(spx_domain, &{rows}, {index}, &{row})) {failure}"
        ));
        Ok(row)
    }
    fn temp_row(&mut self) -> String {
        let name = format!("spx_r{}", self.temps);
        self.temps += 1;
        writeln!(
            self.decls,
            "    __attribute__((unused)) const spx_fused_row *{name} = NULL;"
        )
        .expect("writing to a string cannot fail");
        name
    }
    fn field_read(
        &mut self,
        e: &E,
        element: &T,
        field: &crate::hir::DeclarationId,
        bytes: bool,
        args: &[E],
    ) -> Result<Value, Diagnostic> {
        let sites = self.sites;
        if *element != sites.item.ty {
            return Err(error("selected field read targets a foreign record"));
        }
        let [root, index] = args else {
            return Err(error("selected field read has the wrong arity"));
        };
        let K::Place(place) = &root.kind else {
            return Err(error("selected field read root is not a named row Vec"));
        };
        if !place.projections.is_empty() || !self.is_vector(&root.ty) {
            return Err(error("selected field read root is not a named row Vec"));
        }
        let (rows, kind) = self.lookup(&place.root)?;
        if kind != Kind::Rows {
            return Err(error("selected field read root is not the held row Vec"));
        }
        let rows = rows.to_owned();
        let selected = crate::hir::vec_field::field(&sites.program.declarations, element, field)
            .ok_or_else(|| error("selected field identity is absent from the record"))?;
        let index = self.scalar(index, Kind::Usize)?;
        let index = self.materialize(index, Kind::Usize);
        match (&selected.declaration.ty, bytes, &e.ty, e.ownership) {
            (T::I64, false, T::I64, O::Value) => {
                let row = self.row(&e.id, &rows, &index)?;
                Ok(Value::Scalar(format!("{row}->scalar"), Kind::I64))
            }
            (T::String, _, T::Str | T::SliceU8, O::Borrow) => {
                let expected = if bytes {
                    Carrier::OrdinarySlice
                } else {
                    Carrier::OrdinaryStr
                };
                if self.carrier(e)? != expected {
                    return Err(error(
                        "String field view does not have its ordinary carrier fact",
                    ));
                }
                let row = self.row(&e.id, &rows, &index)?;
                let text = self.temp(Kind::Text);
                self.line(&format!(
                    "{text} = (spx_fused_text){{ (const unsigned char *){row}->string, spx_string_length_v10({row}->string) }};"
                ));
                Ok(Value::Text(text))
            }
            (T::Bytes, false, T::SliceU8, O::Borrow) => {
                if self.carrier(e)? != Carrier::LeasedSlice {
                    return Err(error(
                        "Bytes field view does not have its leased carrier fact",
                    ));
                }
                let _ = self.row(&e.id, &rows, &index)?;
                let view = self.temp(Kind::Leased);
                self.line(&format!(
                    "if (!spx_fused_rows_borrow(spx_domain, &{rows}, {index}, &{view})) {AUTHORITY}"
                ));
                Ok(Value::Leased {
                    name: view,
                    temporary: true,
                })
            }
            _ => Err(error(
                "selected field read shape is outside the closed domain",
            )),
        }
    }
    /// A consumer of a temporary loan ends it immediately after its use.
    fn release(&mut self, value: &Value) {
        if let Value::Leased {
            name,
            temporary: true,
        } = value
        {
            self.line(&format!(
                "if (!spx_fused_borrow_end(spx_domain, &{name})) {AUTHORITY}"
            ));
        }
    }
    fn call(
        &mut self,
        e: &E,
        callee: &str,
        type_arguments: &[T],
        args: &[E],
    ) -> Result<Value, Diagnostic> {
        let kind = Kind::scalar(&e.ty);
        match (callee, args) {
            (crate::vec_ops::LEN_ID, [vector])
                if type_arguments == std::slice::from_ref(&self.sites.item.ty)
                    && kind == Some(Kind::Usize) =>
            {
                let Value::Rows(rows) = self.expression(vector)? else {
                    return Err(error("selected vec_len does not read the held row Vec"));
                };
                let result = self.temp(Kind::Usize);
                self.line(&format!(
                    "if (!spx_fused_rows_len(spx_domain, &{rows}, &{result})) {AUTHORITY}"
                ));
                Ok(Value::Scalar(result, Kind::Usize))
            }
            (crate::byte_ops::LEN_ID, [slice]) if kind == Some(Kind::Usize) => {
                let value = self.expression(slice)?;
                let result = self.temp(Kind::Usize);
                match &value {
                    Value::Text(text) => self.line(&format!("{result} = {text}.len;")),
                    Value::Leased { name, .. } => self.line(&format!(
                        "if (!spx_fused_view_len(spx_domain, {name}, &{result})) {AUTHORITY}"
                    )),
                    _ => return Err(error("byte_len operand is not a provenance-bound view")),
                }
                self.release(&value);
                Ok(Value::Scalar(result, Kind::Usize))
            }
            (crate::str_ops::LEN_BYTES_ID, [text]) if kind == Some(Kind::I64) => {
                let Value::Text(text) = self.expression(text)? else {
                    return Err(error("str_len_bytes operand is not an ordinary String view"));
                };
                Ok(Value::Scalar(format!("(int64_t)({text}.len)"), Kind::I64))
            }
            (crate::string_ops::I64_FROM_USIZE_ID, [value]) if kind == Some(Kind::I64) => {
                let value = self.scalar(value, Kind::Usize)?;
                let value = self.materialize(value, Kind::Usize);
                let failure = self.fail(
                    &e.id,
                    &format!(
                        "if (!spx_status_record_adapter(spx_ctx, \"{}\", UINT32_C({}), SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &spx_status)) spx_runtime_invariant_failure(\"conversion status could not be recorded\");",
                        crate::string_ops::CONVERT_STATUS_DOMAIN,
                        crate::string_ops::CONVERT_OUT_OF_RANGE_CODE
                    ),
                )?;
                self.line(&format!("if ({value} > (uint64_t)INT64_MAX) {failure}"));
                Ok(Value::Scalar(format!("(int64_t){value}"), Kind::I64))
            }
            _ => Err(error(
                "call is outside the closed selected domain; private carriers never cross into ordinary or public consumers",
            )),
        }
    }
    /// `match byte_get(view, index) { Some { value } => .., None {} => .. }`.
    fn option_byte(
        &mut self,
        e: &E,
        scrutinee: &E,
        arms: &[ResolvedMatchArm],
    ) -> Result<Value, Diagnostic> {
        let kind = Kind::scalar(&e.ty).ok_or_else(|| error("selected match is not scalar"))?;
        let K::Call {
            callee,
            instance: None,
            args,
            ..
        } = &scrutinee.kind
        else {
            return Err(error("selected match scrutinee is not byte_get"));
        };
        let [slice, index] = args.as_slice() else {
            return Err(error("selected byte_get has the wrong arity"));
        };
        if callee.as_str() != crate::byte_ops::GET_ID {
            return Err(error("selected match scrutinee is not byte_get"));
        }
        let some_arm = arms
            .iter()
            .find(|arm| case(arm, crate::prelude::OPTION_SOME_ID));
        let none_arm = arms
            .iter()
            .find(|arm| case(arm, crate::prelude::OPTION_NONE_ID));
        let (Some(some_arm), Some(none_arm), 2) = (some_arm, none_arm, arms.len()) else {
            return Err(error(
                "selected byte_get match is not exactly Some and None",
            ));
        };
        let ResolvedMatchPattern::Variant { fields, .. } = &some_arm.pattern else {
            return Err(error("selected Some arm is not a variant pattern"));
        };
        let [field] = fields.as_slice() else {
            return Err(error("selected Some arm does not bind exactly its value"));
        };
        if field.field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID || field.binding.ty != T::U8
        {
            return Err(error("selected Some arm does not bind the u8 value"));
        }
        let ResolvedMatchPattern::Variant { fields: none, .. } = &none_arm.pattern else {
            return Err(error("selected None arm is not a variant pattern"));
        };
        if !none.is_empty() {
            return Err(error("selected None arm binds a field"));
        }
        let view = self.expression(slice)?;
        let index = self.scalar(index, Kind::Usize)?;
        let index = self.materialize(index, Kind::Usize);
        let some = self.temp(Kind::Bool);
        let byte = self.temp(Kind::U8);
        match &view {
            Value::Text(text) => self.line(&format!(
                "{some} = {index} < {text}.len; {byte} = {some} ? {text}.ptr[{index}] : UINT8_C(0);"
            )),
            Value::Leased { name, .. } => self.line(&format!(
                "if (!spx_fused_view_get(spx_domain, {name}, {index}, &{some}, &{byte})) {AUTHORITY}"
            )),
            _ => return Err(error("byte_get operand is not a provenance-bound view")),
        }
        self.release(&view);
        self.apply(&scrutinee.id)?;
        let result = self.temp(kind);
        self.open(&format!("if ({some}) {{"));
        let value = self.bind(&field.binding, Kind::U8)?;
        self.line(&format!("{value} = {byte};"));
        let arm = self.scalar(&some_arm.value, kind)?;
        self.line(&format!("{result} = {arm};"));
        self.close("} else {");
        self.indent += 1;
        let arm = self.scalar(&none_arm.value, kind)?;
        self.line(&format!("{result} = {arm};"));
        self.close("}");
        Ok(Value::Scalar(result, kind))
    }
}

fn case(arm: &ResolvedMatchArm, id: &str) -> bool {
    arm.guard.is_none()
        && matches!(&arm.pattern, ResolvedMatchPattern::Variant { variant, case, .. }
            if variant.as_str() == crate::prelude::OPTION_ID && case.as_str() == id)
}
