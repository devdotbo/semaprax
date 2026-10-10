//! Typed target interfaces for independently replayed lexical control flow.
//! A target identifies meaning, never a backend label or permission to jump.
use super::{ExpressionId, FunctionExecutionId, ResolvedType, ValueId};
pub use crate::ast::LexicalExitKind;

pub const LEXICAL_CONTROL_FLOW_SCHEMA_V1: &str = "semaprax.lexical-control-flow.v1";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ControlTarget {
    Function(FunctionExecutionId),
    Loop {
        owner: FunctionExecutionId,
        /// The exact condition ID of the enclosing authenticated resolved While.
        site: ExpressionId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowCompletion {
    /// A reachable ordinary value; ownership is validated on the expression.
    Normal(ResolvedType),
    /// No normal value exists; the exact target remains on the exit node.
    Exit,
}
impl FlowCompletion {
    /// Both alternatives are independently checked before this type-only join.
    pub fn join(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Exit, Self::Exit) => Some(Self::Exit),
            (Self::Exit, normal @ Self::Normal(_)) | (normal @ Self::Normal(_), Self::Exit) => {
                Some(normal.clone())
            }
            (Self::Normal(left), Self::Normal(right)) if left == right => Some(self.clone()),
            _ => None,
        }
    }
}

/// Source For lowering must preserve the latch, rather than skip its advance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceForKind {
    Copy,
    BorrowRecord,
    OwnedIterator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopLatchKind {
    Condition,
    SourceForStep {
        kind: SourceForKind,
        step: ExpressionId,
    },
    /// Only the authenticated Yield-arm tail runs; never re-enter the authored body.
    OwnedIteratorStep {
        next: ExpressionId,
        assignment: ExpressionId,
        step: ValueId,
        remainder: ValueId,
    },
}

mod walk;
pub(crate) use walk::{child, contains, requires};

mod owner;
pub(crate) use owner::{owner, validate_owner};

pub(super) mod rekey;

mod latch;
pub(crate) use latch::validate_latch;

mod wrapper;
pub(crate) use wrapper::{source_for_wrapper, SourceForWrapper};

mod validation;
pub(crate) use validation::validate_function;

pub(crate) fn reachable_arm(arms: &[super::ResolvedMatchArm]) -> Option<&super::ResolvedMatchArm> {
    arms.iter().find(|arm| arm.value.ty != ResolvedType::Never)
}
pub(crate) fn normal_arm(arms: &[super::ResolvedMatchArm]) -> Option<&super::ResolvedMatchArm> {
    reachable_arm(arms).or_else(|| arms.first())
}
