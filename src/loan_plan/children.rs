//! Borrowed evaluation-order children for CFG construction.

use std::array;
use std::iter::{Chain, Map, Once};
use std::option;
use std::slice;

use crate::hir::{
    ResolvedClosureCapture, ResolvedExpr, ResolvedExprKind, ResolvedFieldInitializer,
};

// Named, higher-ranked function-pointer types keep the iterator types explicit:
// each projection is a plain `fn`, so no closure type leaks into the enum.
type CaptureValue = for<'a> fn(&'a ResolvedClosureCapture) -> &'a ResolvedExpr;
type FieldValue = for<'a> fn(&'a ResolvedFieldInitializer) -> &'a ResolvedExpr;
type Captures<'a> = Map<slice::Iter<'a, ResolvedClosureCapture>, CaptureValue>;
type Fields<'a> = Map<slice::Iter<'a, ResolvedFieldInitializer>, FieldValue>;

pub(super) enum EvaluationChildren<'a> {
    Empty,
    Optional(option::IntoIter<&'a ResolvedExpr>),
    One(array::IntoIter<&'a ResolvedExpr, 1>),
    Two(array::IntoIter<&'a ResolvedExpr, 2>),
    Three(array::IntoIter<&'a ResolvedExpr, 3>),
    Slice(slice::Iter<'a, ResolvedExpr>),
    Captures(Captures<'a>),
    Fields(Fields<'a>),
    Invoke(Chain<Once<&'a ResolvedExpr>, slice::Iter<'a, ResolvedExpr>>),
    Update(Chain<Once<&'a ResolvedExpr>, Fields<'a>>),
}

fn capture_value(capture: &ResolvedClosureCapture) -> &ResolvedExpr {
    &capture.value
}

fn field_value(field: &ResolvedFieldInitializer) -> &ResolvedExpr {
    &field.value
}

impl<'a> Iterator for EvaluationChildren<'a> {
    type Item = &'a ResolvedExpr;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Optional(items) => items.next(),
            Self::One(items) => items.next(),
            Self::Two(items) => items.next(),
            Self::Three(items) => items.next(),
            Self::Slice(items) => items.next(),
            Self::Captures(items) => items.next(),
            Self::Fields(items) => items.next(),
            Self::Invoke(items) => items.next(),
            Self::Update(items) => items.next(),
        }
    }
}

pub(super) fn evaluation_children(expression: &ResolvedExpr) -> EvaluationChildren<'_> {
    match &expression.kind {
        ResolvedExprKind::LexicalExit { value, .. } => {
            EvaluationChildren::Optional(value.as_deref().into_iter())
        }
        ResolvedExprKind::Closure { captures, .. } => {
            EvaluationChildren::Captures(captures.iter().map(capture_value as CaptureValue))
        }
        ResolvedExprKind::FunctionReference { .. } => EvaluationChildren::Empty,
        ResolvedExprKind::Invoke { callable, args } => {
            EvaluationChildren::Invoke(std::iter::once(callable.as_ref()).chain(args.iter()))
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => EvaluationChildren::Slice(args.iter()),
        ResolvedExprKind::NativeRustImportCall(call) => EvaluationChildren::Slice(call.args.iter()),
        ResolvedExprKind::HostCommandCall(call) => EvaluationChildren::Slice(call.args.iter()),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => EvaluationChildren::Three([source.as_ref(), start.as_ref(), end.as_ref()].into_iter()),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => {
            EvaluationChildren::One([value.as_ref()].into_iter())
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            EvaluationChildren::Two([left.as_ref(), right.as_ref()].into_iter())
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            EvaluationChildren::Fields(fields.iter().map(field_value as FieldValue))
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => EvaluationChildren::Update(
            std::iter::once(base.as_ref()).chain(fields.iter().map(field_value as FieldValue)),
        ),
        ResolvedExprKind::Block { .. }
        | ResolvedExprKind::VecConstructor { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Int(_)
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
        | ResolvedExprKind::BorrowPlace { .. } => EvaluationChildren::Empty,
    }
}
