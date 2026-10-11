//! Closed post-construction use domain over the exact typed suffix.
//!
//! Every Str/Slice expression takes its representation from its
//! producer-bound carrier fact; a type alone never selects one. After each
//! lowered expression the plan's transitions at that expression replay, so
//! owner liveness follows the canonical cleanup plan exactly. Leased views end
//! at every end point of their exact `SliceView` loan, and every such end must
//! lie inside the lexical scope that produced them. Any end reached while a
//! consuming host call holds a staged token of the view, including ends nested
//! in that call's later operands, retires it only once every holder has
//! received its token, never between a read and its call. Vec
//! moves and the consuming sort happen only as top-level statements. Anything
//! outside this domain refuses the module.
use super::cleanup::{Failure, Owned};
use super::lower::{unsupported, Binding, Lower, Repr, Scope};
use super::{import, Refusal};
use crate::ast::BinaryOp;
use crate::cleanup_plan::CheckedOperation;
use crate::construction_fusion::native_provenance::Carrier;
use crate::hir::{
    DeclarationId, OwnershipMode as O, Place, ResolvedBinding, ResolvedExpr as E,
    ResolvedExprKind as K, ResolvedMatchArm, ResolvedMatchPattern as P, ResolvedStatement as S,
    ResolvedType as T,
};
use crate::loan_plan::LoanPointPhase;
use crate::wasm::aggregate::{
    STATUS_ADD_OVERFLOW, STATUS_SUB_OVERFLOW, STATUS_VEC_GET_OUT_OF_BOUNDS,
};
use crate::wasm::numeric_conversions::OUT_OF_RANGE_STATUS;
use wasm_encoder::{BlockType, Instruction as I, ValType};

fn scalar(ty: &T) -> Result<Repr, Refusal> {
    match ty {
        T::I64 => Ok(Repr::I64),
        T::Usize => Ok(Repr::Usize),
        T::Bool => Ok(Repr::Bool),
        T::U8 => Ok(Repr::U8),
        _ => Err(unsupported("scalar type")),
    }
}

impl<'a> Lower<'a> {
    pub(super) fn statement(&mut self, statement: &'a S, top: bool) -> Result<(), Refusal> {
        match statement {
            S::Let { binding, value, .. } => {
                if self.is_vector(&binding.ty) {
                    return self.vector_let(binding, value, top);
                }
                let repr = self.expr(value)?;
                if matches!(repr, Repr::Vector | Repr::OptionU8) {
                    return Err(unsupported("binding representation"));
                }
                // A leased value's exact view, where one is still open; a
                // binding without one refuses as a consuming operand.
                let view = match repr {
                    Repr::Leased => self.view_of(value).ok(),
                    _ => None,
                };
                let local = self.local(repr.val());
                self.op(I::LocalSet(local));
                self.bindings.push(Binding {
                    id: &binding.id,
                    local,
                    repr,
                    view,
                });
                Ok(())
            }
            S::Assign {
                binding,
                field: None,
                value,
                ..
            } => {
                let (local, repr) = self.lookup(&binding.id)?;
                if !matches!(repr, Repr::I64 | Repr::Usize | Repr::Bool | Repr::U8)
                    || self.expr(value)? != repr
                {
                    return Err(unsupported("assignment"));
                }
                self.op(I::LocalSet(local));
                Ok(())
            }
            S::While {
                condition, body, ..
            }
            | S::LexicalWhile {
                condition, body, ..
            } => {
                self.op(I::Block(BlockType::Empty));
                self.op(I::Loop(BlockType::Empty));
                if self.expr(condition)? != Repr::Bool {
                    return Err(unsupported("loop condition"));
                }
                self.op(I::I32Eqz);
                self.op(I::BrIf(1));
                self.expr(body)?;
                self.op(I::Drop);
                self.op(I::Br(0));
                self.op(I::End);
                self.op(I::End);
                Ok(())
            }
            _ => Err(unsupported("statement")),
        }
    }

    fn is_vector(&self, ty: &T) -> bool {
        ty == &self.selected.vector.ty
    }

    /// `let next = previous;` or `let next = vec_sort_owned<Row>(previous);`.
    /// The host renews the handle; the plan's transitions move liveness.
    fn vector_let(
        &mut self,
        binding: &'a ResolvedBinding,
        value: &'a E,
        top: bool,
    ) -> Result<(), Refusal> {
        if !top || value.ownership != O::Own {
            return Err(unsupported("Vec transfer scope"));
        }
        let selected = self.selected;
        let (function, place, argument) = match &value.kind {
            K::Place(place) => (import::VEC_MOVE, place, None),
            K::Call {
                callee,
                type_arguments,
                instance: None,
                args,
            } if callee.as_str() == crate::vec_ops::SORT_OWNED_ID
                && type_arguments.as_slice() == std::slice::from_ref(&selected.item.ty) =>
            {
                let [argument] = args.as_slice() else {
                    return Err(unsupported("sort arity"));
                };
                let K::Place(place) = &argument.kind else {
                    return Err(unsupported("sort operand"));
                };
                if argument.ownership != O::Own || !self.is_vector(&argument.ty) {
                    return Err(unsupported("sort operand"));
                }
                (import::SORT, place, Some(argument))
            }
            _ => return Err(unsupported("Vec transfer")),
        };
        if !place.projections.is_empty() {
            return Err(unsupported("Vec projection"));
        }
        let source = self.vector_carrier(&place.root)?;
        self.loan_points(&value.id, LoanPointPhase::Before);
        if let Some(argument) = argument {
            self.visit(argument)?;
        }
        let renewed = self.local(ValType::I64);
        self.op(I::LocalGet(self.domain));
        self.op(I::LocalGet(source));
        self.op(I::Call(function));
        self.owned(renewed);
        self.loan_points(&value.id, LoanPointPhase::After);
        self.post(&value.id, Owned::Leaf(renewed))?;
        self.bindings.push(Binding {
            id: &binding.id,
            local: self.vector_carrier(&binding.id)?,
            repr: Repr::Vector,
            view: None,
        });
        Ok(())
    }

    /// Nonconsuming Vec operand (length or field read): its plan carrier.
    fn vector_arg(&mut self, argument: &'a E) -> Result<u32, Refusal> {
        let K::Place(place) = &argument.kind else {
            return Err(unsupported("Vec operand"));
        };
        if !self.is_vector(&argument.ty)
            || !matches!(argument.ownership, O::Own | O::Borrow)
            || !place.projections.is_empty()
        {
            return Err(unsupported("Vec operand"));
        }
        let carrier = self.vector_carrier(&place.root)?;
        self.visit(argument)?;
        Ok(carrier)
    }

    /// The carrier is the producer's own: a field read's field type and fused
    /// `bytes` flag, inherited unchanged by aliases. Independent provenance,
    /// when present, may only refuse a disagreement.
    fn carrier(&self, e: &E, repr: Repr) -> Result<(), Refusal> {
        let carrier = match repr {
            Repr::Text => Carrier::OrdinaryStr,
            Repr::Slice => Carrier::OrdinarySlice,
            Repr::Leased => Carrier::LeasedSlice,
            _ => return Err(Refusal::Carrier),
        };
        self.selected.cross_check(e, carrier)
    }

    fn temp(&mut self, repr: Repr) -> u32 {
        let local = self.local(repr.val());
        self.op(I::LocalSet(local));
        local
    }

    fn host(&mut self, function: u32, operands: &[u32]) {
        self.op(I::LocalGet(self.domain));
        for local in operands {
            self.op(I::LocalGet(*local));
        }
        self.op(I::Call(function));
    }

    /// Lower one expression between its loan points, then replay the plan's
    /// transitions at it. Loans end before owner transitions at the same point.
    pub(super) fn expr(&mut self, e: &'a E) -> Result<Repr, Refusal> {
        self.loan_points(&e.id, LoanPointPhase::Before);
        let repr = self.expr_inner(e)?;
        self.loan_points(&e.id, LoanPointPhase::After);
        self.post(&e.id, Owned::None)?;
        Ok(repr)
    }
    /// The view operand of a consuming host call (`view_len`, `view_byte`).
    /// Its token is staged once the operand is read, so a leased operand holds
    /// its exact open view from that read until the caller's `release` right
    /// after the call. Every loan end of that view reached meanwhile, at the
    /// operand's own after point or anywhere in a later operand (a nested
    /// `byte_len` of the same view in the index), defers rather than retiring
    /// the staged token; a failure exit in between retires it once. Loan
    /// points and transitions otherwise replay exactly as in `expr`.
    fn consumed_operand(&mut self, e: &'a E) -> Result<Repr, Refusal> {
        self.loan_points(&e.id, LoanPointPhase::Before);
        let repr = self.expr_inner(e)?;
        if repr == Repr::Leased {
            let view = self.view_of(e)?;
            self.holds.push(view);
        }
        self.loan_points(&e.id, LoanPointPhase::After);
        self.post(&e.id, Owned::None)?;
        Ok(repr)
    }
    /// Loan points and transitions of an expression lowered outside `expr`.
    fn visit(&mut self, e: &E) -> Result<(), Refusal> {
        self.loan_points(&e.id, LoanPointPhase::Before);
        self.loan_points(&e.id, LoanPointPhase::After);
        self.post(&e.id, Owned::None)
    }

    fn expr_inner(&mut self, e: &'a E) -> Result<Repr, Refusal> {
        match &e.kind {
            K::Int(value) if e.ty == T::I64 => {
                self.op(I::I64Const(*value));
                Ok(Repr::I64)
            }
            K::Usize(value) if e.ty == T::Usize => {
                // Exact u64 bit pattern; every Usize operation is unsigned.
                self.op(I::I64Const(i64::from_ne_bytes(value.to_ne_bytes())));
                Ok(Repr::Usize)
            }
            K::Uint8(value) => {
                self.op(I::I32Const(i32::from(*value)));
                Ok(Repr::U8)
            }
            K::Bool(value) => {
                self.op(I::I32Const(i32::from(*value)));
                Ok(Repr::Bool)
            }
            K::Place(place) => self.place(e, place),
            K::BorrowPlace { operation, place } => self.str_bytes(e, operation, place),
            K::VecFieldRead {
                element,
                field,
                bytes,
                args,
            } => self.field_read(e, element, field, *bytes, args),
            K::Binary { op, left, right } => self.binary(e, *op, left, right),
            K::Call {
                callee,
                type_arguments,
                instance: None,
                args,
            } => self.call(e, callee.as_str(), type_arguments, args),
            K::Match {
                scrutinee, arms, ..
            } => self.option_match(e, scrutinee, arms),
            K::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let repr = scalar(&e.ty)?;
                if self.expr(condition)? != Repr::Bool {
                    return Err(unsupported("if condition"));
                }
                self.op(I::If(BlockType::Result(repr.val())));
                if self.expr(then_branch)? != repr {
                    return Err(unsupported("if branch"));
                }
                self.op(I::Else);
                if self.expr(else_branch)? != repr {
                    return Err(unsupported("if branch"));
                }
                self.op(I::End);
                Ok(repr)
            }
            K::Block { statements, tail } => {
                let mark = self.bindings.len();
                self.scopes.push(Scope { views: Vec::new() });
                for statement in statements {
                    self.statement(statement, false)?;
                }
                let repr = self.expr(tail)?;
                // Loans end first, then the plan's own scope-end finalizers.
                self.close_scope(e)?;
                self.block_end(e)?;
                self.bindings.truncate(mark);
                Ok(repr)
            }
            _ => Err(unsupported("expression")),
        }
    }

    fn place(&mut self, e: &E, place: &Place) -> Result<Repr, Refusal> {
        if !place.projections.is_empty() {
            return Err(unsupported("projection"));
        }
        let (local, repr) = self.lookup(&place.root)?;
        if repr == Repr::Vector {
            return Err(unsupported("Vec value in expression"));
        }
        if matches!(repr, Repr::Text | Repr::Slice | Repr::Leased) {
            self.carrier(e, repr)?;
        }
        self.op(I::LocalGet(local));
        Ok(repr)
    }

    fn str_bytes(
        &mut self,
        e: &E,
        operation: &DeclarationId,
        place: &Place,
    ) -> Result<Repr, Refusal> {
        if operation.as_str() != crate::byte_ops::STR_AS_BYTES_ID || !place.projections.is_empty() {
            return Err(unsupported("borrowed place operation"));
        }
        let (local, repr) = self.lookup(&place.root)?;
        if repr != Repr::Text {
            return Err(Refusal::Carrier);
        }
        self.carrier(e, Repr::Slice)?;
        self.op(I::LocalGet(local));
        Ok(Repr::Slice)
    }

    fn field_read(
        &mut self,
        e: &'a E,
        element: &T,
        field: &DeclarationId,
        bytes: bool,
        args: &'a [E],
    ) -> Result<Repr, Refusal> {
        let selected = self.selected;
        let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &selected.record.kind
        else {
            return Err(unsupported("row record"));
        };
        let [root, index] = args else {
            return Err(unsupported("field read arity"));
        };
        if element != &selected.item.ty || index.ty != T::Usize {
            return Err(unsupported("field read operand"));
        }
        let vector = self.vector_arg(root)?;
        let K::Place(position) = &index.kind else {
            return Err(unsupported("field read index"));
        };
        let (position, Repr::Usize) = self.lookup(&position.root)? else {
            return Err(unsupported("field read index"));
        };
        self.visit(index)?;
        // The read's ordinary out-of-range status leaves through the plan's
        // exact exit for this expression, before the host is asked for a row.
        self.op(I::LocalGet(position));
        self.host(import::VEC_LEN, &[vector]);
        self.op(I::I64GeU);
        self.op(I::If(BlockType::Empty));
        self.fail(&e.id, Failure::VecRead, STATUS_VEC_GET_OUT_OF_BOUNDS)?;
        self.op(I::End);
        let declaration = fields
            .iter()
            .find(|candidate| &candidate.id == field)
            .ok_or(unsupported("field read field"))?;
        match (&declaration.ty, bytes) {
            (T::I64, false) => {
                self.host(import::FIELD_SCALAR, &[vector, position]);
                Ok(Repr::I64)
            }
            (T::String, _) => {
                let repr = if bytes { Repr::Slice } else { Repr::Text };
                self.carrier(e, repr)?;
                self.host(import::FIELD_TEXT, &[vector, position]);
                Ok(repr)
            }
            (T::Bytes, false) if declaration.id == selected.byte_field.id => {
                self.carrier(e, Repr::Leased)?;
                let view = self.local(ValType::I64);
                self.host(import::PAYLOAD_BORROW, &[vector, position]);
                self.owned(view);
                self.open_view(view, &e.id)?;
                self.op(I::LocalGet(view));
                Ok(Repr::Leased)
            }
            _ => Err(unsupported("field read type")),
        }
    }

    fn binary(
        &mut self,
        e: &'a E,
        op: BinaryOp,
        left: &'a E,
        right: &'a E,
    ) -> Result<Repr, Refusal> {
        match op {
            BinaryOp::And | BinaryOp::Or => {
                if self.expr(left)? != Repr::Bool {
                    return Err(unsupported("logical operand"));
                }
                self.op(I::If(BlockType::Result(ValType::I32)));
                if op == BinaryOp::Or {
                    self.op(I::I32Const(1));
                    self.op(I::Else);
                }
                if self.expr(right)? != Repr::Bool {
                    return Err(unsupported("logical operand"));
                }
                if op == BinaryOp::And {
                    self.op(I::Else);
                    self.op(I::I32Const(0));
                }
                self.op(I::End);
                Ok(Repr::Bool)
            }
            BinaryOp::Add | BinaryOp::Sub => self.arithmetic(e, op, left, right),
            BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::Eq
            | BinaryOp::Ne => {
                let repr = self.expr(left)?;
                if self.expr(right)? != repr {
                    return Err(unsupported("comparison operands"));
                }
                use BinaryOp::{Eq, Ge, Gt, Le, Lt, Ne};
                let instruction = match (repr, op) {
                    (Repr::I64, Lt) => I::I64LtS,
                    (Repr::I64, Le) => I::I64LeS,
                    (Repr::I64, Gt) => I::I64GtS,
                    (Repr::I64, Ge) => I::I64GeS,
                    (Repr::Usize, Lt) => I::I64LtU,
                    (Repr::Usize, Le) => I::I64LeU,
                    (Repr::Usize, Gt) => I::I64GtU,
                    (Repr::Usize, Ge) => I::I64GeU,
                    (Repr::I64 | Repr::Usize, Eq) => I::I64Eq,
                    (Repr::I64 | Repr::Usize, Ne) => I::I64Ne,
                    (Repr::U8, Lt) => I::I32LtU,
                    (Repr::U8, Le) => I::I32LeU,
                    (Repr::U8, Gt) => I::I32GtU,
                    (Repr::U8, Ge) => I::I32GeU,
                    (Repr::U8 | Repr::Bool, Eq) => I::I32Eq,
                    (Repr::U8 | Repr::Bool, Ne) => I::I32Ne,
                    _ => return Err(unsupported("comparison type")),
                };
                self.op(instruction);
                Ok(Repr::Bool)
            }
            _ => Err(unsupported("binary operator")),
        }
    }

    /// Checked i64/usize addition and subtraction, operands left to right;
    /// overflow takes the plan's exact failure exit for this expression.
    fn arithmetic(
        &mut self,
        e: &'a E,
        op: BinaryOp,
        left: &'a E,
        right: &'a E,
    ) -> Result<Repr, Refusal> {
        let repr = self.expr(left)?;
        let a = self.temp(repr);
        if self.expr(right)? != repr || !matches!(repr, Repr::I64 | Repr::Usize) {
            return Err(unsupported("arithmetic operands"));
        }
        let b = self.temp(repr);
        let c = self.local(ValType::I64);
        let add = op == BinaryOp::Add;
        self.op(I::LocalGet(a));
        self.op(I::LocalGet(b));
        self.op(if add { I::I64Add } else { I::I64Sub });
        self.op(I::LocalSet(c));
        match (repr, add) {
            (Repr::I64, true) => {
                for (x, y) in [(a, c), (b, c)] {
                    self.op(I::LocalGet(x));
                    self.op(I::LocalGet(y));
                    self.op(I::I64Xor);
                }
                self.op(I::I64And);
                self.op(I::I64Const(0));
                self.op(I::I64LtS);
            }
            (Repr::I64, false) => {
                for (x, y) in [(a, b), (a, c)] {
                    self.op(I::LocalGet(x));
                    self.op(I::LocalGet(y));
                    self.op(I::I64Xor);
                }
                self.op(I::I64And);
                self.op(I::I64Const(0));
                self.op(I::I64LtS);
            }
            (_, true) => {
                self.op(I::LocalGet(c));
                self.op(I::LocalGet(a));
                self.op(I::I64LtU);
            }
            (_, false) => {
                self.op(I::LocalGet(a));
                self.op(I::LocalGet(b));
                self.op(I::I64LtU);
            }
        }
        self.op(I::If(BlockType::Empty));
        let status = if add {
            STATUS_ADD_OVERFLOW
        } else {
            STATUS_SUB_OVERFLOW
        };
        let operation = if add {
            CheckedOperation::Add
        } else {
            CheckedOperation::Sub
        };
        self.fail(&e.id, Failure::Arithmetic(operation), status)?;
        self.op(I::End);
        self.op(I::LocalGet(c));
        Ok(repr)
    }

    fn call(
        &mut self,
        e: &'a E,
        callee: &str,
        type_arguments: &[T],
        args: &'a [E],
    ) -> Result<Repr, Refusal> {
        let result = match (callee, args) {
            (crate::string_ops::I64_FROM_USIZE_ID, [value]) => {
                if self.expr(value)? != Repr::Usize {
                    return Err(unsupported("conversion operand"));
                }
                let value = self.temp(Repr::Usize);
                self.op(I::LocalGet(value));
                self.op(I::I64Const(0));
                self.op(I::I64LtS);
                self.op(I::If(BlockType::Empty));
                self.fail(&e.id, Failure::Conversion, OUT_OF_RANGE_STATUS)?;
                self.op(I::End);
                self.op(I::LocalGet(value));
                Repr::I64
            }
            (crate::str_ops::LEN_BYTES_ID, [text]) => {
                if self.expr(text)? != Repr::Text {
                    return Err(Refusal::Carrier);
                }
                let text = self.temp(Repr::Text);
                self.host(import::TEXT_LEN, &[text]);
                Repr::I64
            }
            (crate::byte_ops::LEN_ID, [view]) => {
                let mark = self.holds.len();
                let carrier = self.consumed_operand(view)?;
                let function = match carrier {
                    Repr::Slice => import::TEXT_LEN,
                    Repr::Leased => import::VIEW_LEN,
                    _ => return Err(Refusal::Carrier),
                };
                let view = self.temp(carrier);
                self.host(function, &[view]);
                self.release(mark);
                Repr::Usize
            }
            (crate::byte_ops::GET_ID, [view, index]) => {
                // The view's hold spans the index: a same-view end inside it
                // (even a nested call's own release) cannot retire this token.
                let mark = self.holds.len();
                let carrier = self.consumed_operand(view)?;
                let function = match carrier {
                    Repr::Slice => import::TEXT_BYTE,
                    Repr::Leased => import::VIEW_BYTE,
                    _ => return Err(Refusal::Carrier),
                };
                let view = self.temp(carrier);
                if self.expr(index)? != Repr::Usize {
                    return Err(unsupported("byte index"));
                }
                let index = self.temp(Repr::Usize);
                self.host(function, &[view, index]);
                self.release(mark);
                return Ok(Repr::OptionU8);
            }
            (crate::vec_ops::LEN_ID, [vector])
                if type_arguments == std::slice::from_ref(&self.selected.item.ty) =>
            {
                let vector = self.vector_arg(vector)?;
                self.host(import::VEC_LEN, &[vector]);
                Repr::Usize
            }
            _ => return Err(unsupported("call")),
        };
        if scalar(&e.ty)? != result {
            return Err(unsupported("call result type"));
        }
        Ok(result)
    }

    /// `match byte_get(..) { Some { value } => .., None {} => .. }`.
    fn option_match(
        &mut self,
        e: &'a E,
        scrutinee: &'a E,
        arms: &'a [ResolvedMatchArm],
    ) -> Result<Repr, Refusal> {
        let T::Nominal {
            declaration,
            arguments,
        } = &scrutinee.ty
        else {
            return Err(unsupported("match scrutinee"));
        };
        if declaration.as_str() != crate::prelude::OPTION_ID || arguments.as_slice() != [T::U8] {
            return Err(unsupported("match scrutinee"));
        }
        let mut some = None;
        let mut none = None;
        for arm in arms {
            if arm.guard.is_some() {
                return Err(unsupported("match guard"));
            }
            match &arm.pattern {
                P::Variant { case, fields, .. }
                    if case.as_str() == crate::prelude::OPTION_SOME_ID && some.is_none() =>
                {
                    let [field] = fields.as_slice() else {
                        return Err(unsupported("Some pattern"));
                    };
                    if field.field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID
                        || field.binding.ty != T::U8
                    {
                        return Err(unsupported("Some pattern"));
                    }
                    some = Some((&field.binding, &arm.value));
                }
                P::Variant { case, fields, .. }
                    if case.as_str() == crate::prelude::OPTION_NONE_ID
                        && fields.is_empty()
                        && none.is_none() =>
                {
                    none = Some(&arm.value);
                }
                _ => return Err(unsupported("match pattern")),
            }
        }
        let (Some((binding, some)), Some(none), 2) = (some, none, arms.len()) else {
            return Err(unsupported("match arms"));
        };
        let repr = scalar(&e.ty)?;
        if self.expr(scrutinee)? != Repr::OptionU8 {
            return Err(unsupported("match scrutinee"));
        }
        let selected = self.temp(Repr::OptionU8);
        self.op(I::LocalGet(selected));
        self.op(I::I32Const(0));
        self.op(I::I32GeS);
        self.op(I::If(BlockType::Result(repr.val())));
        let mark = self.bindings.len();
        let value = self.local(ValType::I32);
        self.op(I::LocalGet(selected));
        self.op(I::LocalSet(value));
        self.bindings.push(Binding {
            id: &binding.id,
            local: value,
            repr: Repr::U8,
            view: None,
        });
        if self.expr(some)? != repr {
            return Err(unsupported("match arm type"));
        }
        self.bindings.truncate(mark);
        self.op(I::Else);
        if self.expr(none)? != repr {
            return Err(unsupported("match arm type"));
        }
        self.op(I::End);
        Ok(repr)
    }
}
