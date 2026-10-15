//! Complete original expression/pattern dispatcher with summary-only carriers.
//! Source preorder, closure captures/body exclusion, argument order, let and
//! match binding epochs, update-base materialization and temporary predicates
//! retain their original locations. No identity clone feeds a discarded slot.
use super::*;
impl<'s> Builder<'s, '_> {
    pub(super) fn collect_expression(
        &mut self,
        expression: &'s ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedExpr),
            Children(&'a ResolvedExpr, usize),
            Finish(&'a ResolvedExpr),
            AddBinding(&'a ResolvedBinding),
            AddPatternBindings(&'a crate::hir::ResolvedMatchPattern),
            AddUpdateBase(&'a ResolvedExpr),
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == 24) };

        let mut frames = inventory_flags::vector(self.payer)?;
        inventory_flags::push(&mut frames, Frame::Enter(expression), self.payer)?;
        loop {
            metadata::step(self.payer, 1)?;
            let Some(frame) = frames.pop() else {
                break;
            };
            metadata::step(self.payer, 1)?;

            match frame {
                Frame::Enter(expression) => {
                    inventory_flags::grow(&mut frames, 2, self.payer)?;
                    inventory_flags::push(&mut frames, Frame::Finish(expression), self.payer)?;
                    inventory_flags::push(&mut frames, Frame::Children(expression, 0), self.payer)?;
                }
                Frame::Children(expression, index) => {
                    inventory_flags::grow(&mut frames, 2, self.payer)?;
                    let mut enter = None;
                    let mut action = None;
                    match &expression.kind {
                        ResolvedExprKind::Closure { captures, .. } => {
                            enter = captures.get(index).map(|capture| &capture.value);
                        }
                        ResolvedExprKind::FunctionReference { .. } => {}
                        ResolvedExprKind::Invoke { callable, args } => {
                            enter = if index == 0 {
                                Some(callable.as_ref())
                            } else {
                                args.get(index - 1)
                            };
                        }
                        ResolvedExprKind::Call { args, .. }
                        | ResolvedExprKind::LiteralFormat { args, .. }
                        | ResolvedExprKind::VecFieldRead { args, .. }
                        | ResolvedExprKind::RowLoan { args, .. } => {
                            enter = args.get(index);
                        }
                        ResolvedExprKind::NativeRustImportCall(call) => {
                            enter = call.args.get(index);
                        }
                        ResolvedExprKind::HostCommandCall(call) => {
                            enter = call.args.get(index);
                        }
                        ResolvedExprKind::ByteRange {
                            source, start, end, ..
                        } => {
                            enter = [source.as_ref(), start.as_ref(), end.as_ref()]
                                .get(index)
                                .copied();
                        }
                        ResolvedExprKind::Unary { value, .. }
                        | ResolvedExprKind::Project { base: value, .. }
                        | ResolvedExprKind::Try { operand: value, .. }
                        | ResolvedExprKind::TryOption { operand: value, .. }
                        | ResolvedExprKind::Upcast { source: value }
                        | ResolvedExprKind::Yield { request: value } => {
                            enter = (index == 0).then_some(value.as_ref());
                        }
                        ResolvedExprKind::LexicalExit { value, .. } => {
                            enter = value.as_deref().filter(|_| index == 0);
                        }
                        ResolvedExprKind::Binary { left, right, .. } => {
                            enter = match index {
                                0 => Some(left),
                                1 => Some(right),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::Block { statements, tail }
                        | ResolvedExprKind::VecConstructor {
                            statements, tail, ..
                        } => {
                            // Only lets add a trailing binding step; other statements keep walking.
                            let per_statement = |statement: &ResolvedStatement| {
                                statement.child_count().saturating_add(usize::from(matches!(
                                    statement,
                                    ResolvedStatement::Let { .. }
                                )))
                            };
                            let mut offset = 0usize;
                            let mut matched = None;
                            for statement in statements.iter() {
                                metadata::step(self.payer, 1)?;
                                let steps = per_statement(statement);
                                if index < offset + steps {
                                    let within = index - offset;
                                    if within < statement.child_count() {
                                        enter = statement.child(within);
                                    }
                                    // Only `let` statements carry bindings;
                                    // assignment, unsafe, and while
                                    // statements add none here.
                                    if within == statement.child_count() {
                                        if let ResolvedStatement::Let { binding, .. } = statement {
                                            action = Some(Frame::AddBinding(binding));
                                        }
                                    }
                                    matched = Some(());
                                    break;
                                }
                                offset += steps;
                            }
                            if matched.is_none() && index == offset {
                                enter = Some(tail);
                            }
                        }
                        ResolvedExprKind::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            enter = match index {
                                0 => Some(condition),
                                1 => Some(then_branch),
                                2 => Some(else_branch),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::ConstructRecord { fields, .. }
                        | ResolvedExprKind::ConstructVariant { fields, .. } => {
                            enter = fields.get(index).map(|field| &field.value);
                        }
                        ResolvedExprKind::Match {
                            scrutinee, arms, ..
                        } => {
                            enter = if index == 0 {
                                Some(scrutinee)
                            } else {
                                let mut cursor = index - 1;
                                let mut found = None;
                                for arm in arms {
                                    metadata::step(self.payer, 1)?;
                                    if cursor == 0 {
                                        action = Some(Frame::AddPatternBindings(&arm.pattern));
                                        break;
                                    }
                                    cursor -= 1;
                                    if let Some(guard) = &arm.guard {
                                        if cursor == 0 {
                                            found = Some(guard.as_ref());
                                            break;
                                        }
                                        cursor -= 1;
                                    }
                                    if cursor == 0 {
                                        found = Some(&arm.value);
                                        break;
                                    }
                                    cursor -= 1;
                                }
                                found
                            };
                        }
                        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                            if index == 0 {
                                enter = Some(base);
                            } else if index == 1 {
                                action = Some(Frame::AddUpdateBase(base));
                            } else {
                                enter = fields.get(index - 2).map(|field| &field.value);
                            }
                        }
                        ResolvedExprKind::Int(_)
                        | ResolvedExprKind::Int32(_)
                        | ResolvedExprKind::Char(_)
                        | ResolvedExprKind::Uint8(_)
                        | ResolvedExprKind::Usize(_)
                        | ResolvedExprKind::ArrayU8(_)
                        | ResolvedExprKind::RepeatArrayU8 { .. }
                        | ResolvedExprKind::Float32(_)
                        | ResolvedExprKind::Float64(_)
                        | ResolvedExprKind::Bool(_)
                        | ResolvedExprKind::String(_)
                        | ResolvedExprKind::BorrowedUtf8Literal(_)
                        | ResolvedExprKind::Place(_)
                        | ResolvedExprKind::BorrowPlace { .. } => {}
                    }
                    if enter.is_some() || action.is_some() {
                        inventory_flags::push(
                            &mut frames,
                            Frame::Children(expression, index + 1),
                            self.payer,
                        )?;
                        if let Some(action) = action {
                            inventory_flags::push(&mut frames, action, self.payer)?;
                        }
                        if let Some(child) = enter {
                            inventory_flags::push(&mut frames, Frame::Enter(child), self.payer)?;
                        }
                    }
                }
                Frame::Finish(expression) => self.collect_owned_temporary(expression)?,
                Frame::AddBinding(binding) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(&binding.ty)?;
                    }
                }
                Frame::AddPatternBindings(pattern) => {
                    self.collect_pattern_bindings(pattern)?;
                }
                Frame::AddUpdateBase(base) => {
                    // A place expression normally needs no temporary. Record
                    // update deliberately materializes an owned base epoch.
                    if matches!(base.kind, ResolvedExprKind::Place(_))
                        && base.ownership == OwnershipMode::Own
                        && self.needs_drop(&base.ty)?
                    {
                        self.add_slot(&base.ty)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn collect_pattern_bindings(
        &mut self,
        pattern: &'s crate::hir::ResolvedMatchPattern,
    ) -> Result<(), Diagnostic> {
        use crate::hir::{ResolvedMatchPattern, ResolvedRecordMatchFieldPattern};

        enum Item<'a> {
            Pattern(&'a ResolvedMatchPattern),
            RecordField(&'a ResolvedRecordMatchFieldPattern),
        }
        let mut pending = inventory_flags::vector(self.payer)?;
        inventory_flags::push(&mut pending, Item::Pattern(pattern), self.payer)?;
        loop {
            metadata::step(self.payer, 1)?;
            let Some(item) = pending.pop() else {
                break;
            };
            metadata::step(self.payer, 1)?;
            match item {
                Item::Pattern(ResolvedMatchPattern::Binding(binding)) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(&binding.ty)?;
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Record { record, fields, .. }) => {
                    let declared =
                        metadata::fields(self.program.declarations(), record, self.payer)?
                            .ok_or_else(|| error("record pattern has no field inventory"))?;
                    for declaration in declared.iter().rev() {
                        let mut found = None;
                        for field in fields {
                            metadata::step(self.payer, 1)?;
                            if inventory_text::owner_matches(
                                &field.field,
                                &declaration.id,
                                self.payer,
                            )? {
                                found = Some(field);
                                break;
                            }
                        }
                        let field = found
                            .ok_or_else(|| error("record pattern field inventory is incomplete"))?;
                        inventory_flags::grow(&mut pending, 1, self.payer)?;
                        inventory_flags::push(
                            &mut pending,
                            Item::RecordField(&field.pattern),
                            self.payer,
                        )?;
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Variant { fields, .. }) => {
                    for field in fields {
                        metadata::step(self.payer, 1)?;
                        if field.binding.ownership == OwnershipMode::Own
                            && self.needs_drop(&field.binding.ty)?
                        {
                            self.add_slot(&field.binding.ty)?;
                        }
                    }
                }
                Item::Pattern(
                    ResolvedMatchPattern::Wildcard
                    | ResolvedMatchPattern::Literal(_)
                    | ResolvedMatchPattern::Or(_),
                ) => {}
                Item::RecordField(ResolvedRecordMatchFieldPattern::Binding(binding)) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(&binding.ty)?;
                    }
                }
                Item::RecordField(ResolvedRecordMatchFieldPattern::Record {
                    record,
                    fields,
                    ..
                }) => {
                    let declared =
                        metadata::fields(self.program.declarations(), record, self.payer)?
                            .ok_or_else(|| error("nested record pattern has no field inventory"))?;
                    for declaration in declared.iter().rev() {
                        let mut found = None;
                        for field in fields {
                            metadata::step(self.payer, 1)?;
                            if inventory_text::owner_matches(
                                &field.field,
                                &declaration.id,
                                self.payer,
                            )? {
                                found = Some(field);
                                break;
                            }
                        }
                        let field = found.ok_or_else(|| {
                            error("nested record pattern field inventory is incomplete")
                        })?;
                        inventory_flags::grow(&mut pending, 1, self.payer)?;
                        inventory_flags::push(
                            &mut pending,
                            Item::RecordField(&field.pattern),
                            self.payer,
                        )?;
                    }
                }
                Item::RecordField(ResolvedRecordMatchFieldPattern::Wildcard) => {}
            }
        }
        Ok(())
    }

    fn collect_owned_temporary(&mut self, expression: &'s ResolvedExpr) -> Result<(), Diagnostic> {
        if expression.ownership == OwnershipMode::Own
            && self.needs_drop(&expression.ty)?
            && (expression.ty == ResolvedType::String
                || !matches!(expression.kind, ResolvedExprKind::Place(_)))
        {
            self.add_slot(&expression.ty)?;
        }
        Ok(())
    }
}
