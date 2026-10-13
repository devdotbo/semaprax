//! Borrowed fallible discovery cursor: inline stack refs with caller-paid spill.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;

const INLINE: usize = 32;

pub(super) struct Cursor<'a, 'p> {
    function: &'a ResolvedFunction,
    next_root: usize,
    inline: [Option<&'a ResolvedExpr>; INLINE],
    inline_len: usize,
    spill: Vec<&'a ResolvedExpr>,
    payer: &'p CfgReservation,
    failed: bool,
}

impl<'a, 'p> Cursor<'a, 'p> {
    pub(super) fn new(function: &'a ResolvedFunction, payer: &'p CfgReservation) -> Self {
        Self {
            function,
            next_root: 0,
            inline: [None; INLINE],
            inline_len: 0,
            spill: Vec::new(),
            payer,
            failed: false,
        }
    }

    pub(super) fn reset(&mut self) -> Result<(), Diagnostic> {
        self.payer.reserve(0).map_err(|_| storage::marker())?;
        if self.failed || self.inline_len != 0 || !self.spill.is_empty() {
            return Err(storage::marker());
        }
        self.next_root = 0;
        Ok(())
    }

    /// Yields expressions in preorder: roots are `requires`, `body`, `ensures`.
    pub(super) fn next(
        &mut self,
        work: &mut WorkCounter<'_>,
    ) -> Result<Option<&'a ResolvedExpr>, Diagnostic> {
        if self.failed {
            return Err(storage::marker());
        }
        let result = self.advance(work);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn advance(
        &mut self,
        work: &mut WorkCounter<'_>,
    ) -> Result<Option<&'a ResolvedExpr>, Diagnostic> {
        self.payer.reserve(0).map_err(|_| storage::marker())?;
        let expression = match self.pop() {
            Some(expression) => expression,
            None => match self.root() {
                Some(root) => root,
                None => return Ok(None),
            },
        };
        charge(work)?;
        push_children(expression, self)?;
        Ok(Some(expression))
    }

    fn root(&mut self) -> Option<&'a ResolvedExpr> {
        let function = self.function;
        let index = self.next_root;
        let requires = function.requires.len();
        let root = if index < requires {
            function.requires.get(index)
        } else if index == requires {
            Some(&function.body)
        } else {
            function.ensures.get(index - requires - 1)
        }?;
        self.next_root += 1;
        Some(root)
    }

    // Invariant: a non-empty spill implies the inline array is full.
    fn pop(&mut self) -> Option<&'a ResolvedExpr> {
        if let Some(expression) = self.spill.pop() {
            return Some(expression);
        }
        if self.inline_len == 0 {
            return None;
        }
        self.inline_len -= 1;
        Some(
            self.inline[self.inline_len]
                .take()
                .expect("inline slot below length is populated"),
        )
    }

    fn push(&mut self, expression: &'a ResolvedExpr) -> Result<(), Diagnostic> {
        self.payer.reserve(0).map_err(|_| storage::marker())?;
        if self.spill.is_empty() && self.inline_len < INLINE {
            self.inline[self.inline_len] = Some(expression);
            self.inline_len += 1;
            return Ok(());
        }
        storage::push(&mut self.spill, expression, Some(self.payer))
    }

    fn extend(
        &mut self,
        expressions: impl IntoIterator<Item = &'a ResolvedExpr>,
    ) -> Result<(), Diagnostic> {
        for expression in expressions {
            self.push(expression)?;
        }
        Ok(())
    }
}

fn push_children<'a>(
    expression: &'a ResolvedExpr,
    pending: &mut Cursor<'a, '_>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::LexicalExit { value, .. } => {
            pending.extend(value.as_deref())?;
        }
        ResolvedExprKind::Closure { captures, .. } => {
            pending.extend(captures.iter().rev().map(|capture| &capture.value))?
        }
        ResolvedExprKind::FunctionReference { .. } => {}
        ResolvedExprKind::Invoke { callable, args } => {
            pending.extend(args.iter().rev())?;
            pending.push(callable)?;
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            pending.push(tail)?;
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child)?;
                    }
                }
            }
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => pending.extend(args.iter().rev())?,
        ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter().rev())?,
        ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter().rev())?,
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            pending.push(end)?;
            pending.push(start)?;
            pending.push(source)?;
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => pending.push(value)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            pending.push(right)?;
            pending.push(left)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(else_branch)?;
            pending.push(then_branch)?;
            pending.push(condition)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value))?;
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                pending.push(&arm.value)?;
                if let Some(guard) = &arm.guard {
                    pending.push(guard)?;
                }
            }
            pending.push(scrutinee)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value))?;
            pending.push(base)?;
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
    Ok(())
}

#[cfg(test)]
mod tests;
