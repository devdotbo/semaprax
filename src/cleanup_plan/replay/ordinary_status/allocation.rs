//! Independent typed site discovery for a source-held allocation candidate.
//! This catalogue deliberately does not call the candidate builder classifier.
//! Existing consuming/read predicates retain their ordinary source semantics;
//! no decoded row, plan schema or operation name can create this selection.
use super::*;
use crate::cleanup_plan::AllocationOperation as A;

pub(super) struct Selection {
    moves: BTreeSet<ExpressionId>,
    reads: BTreeSet<ExpressionId>,
}
impl Selection {
    pub(super) fn new(function: &ResolvedFunction) -> Result<Self, Diagnostic> {
        Ok(Self {
            moves: crate::literal_format::checked_moving_string_operands(function)?,
            reads: crate::string_ops::conditions::function_reads(function),
        })
    }
    pub(super) fn operation(&self, expression: &ResolvedExpr) -> Option<A> {
        match &expression.kind {
            ResolvedExprKind::String(_) => Some(A::StringLiteral),
            ResolvedExprKind::Place(_) if expression.ty == ResolvedType::String
                && expression.ownership == OwnershipMode::Own
                && !self.moves.contains(&expression.id)
                && !self.reads.contains(&expression.id) => Some(A::StringClone),
            ResolvedExprKind::LiteralFormat { .. } => Some(A::LiteralFormat),
            ResolvedExprKind::Call { callee, instance: None, type_arguments, .. } => {
                use crate::string_ops::StringOp as S;
                if let Some(operation) = crate::string_ops::by_id(callee.as_str()) {
                    return match operation {
                        S::Concat => Some(A::StringConcat),
                        S::FromChar => Some(A::StringFromChar),
                        S::FromI64 => Some(A::StringFromI64),
                        S::FromUsize => Some(A::StringFromUsize),
                        S::FromStr => Some(A::StringFromStr),
                        S::FromUtf8 => Some(A::StringFromUtf8),
                        S::Slice => Some(A::StringSlice),
                        S::Trim => Some(A::StringTrim),
                        S::MapNew => Some(A::MapBacking),
                        S::MapAdd | S::MapSet => Some(A::MapKeyClone),
                        S::MapKeyAt => Some(A::StringClone),
                        _ => None,
                    };
                }
                use crate::byte_ops::ByteOp as B;
                if let Some(operation) = crate::byte_ops::by_id(callee.as_str()) {
                    return match operation {
                        B::Copy => Some(A::BytesCopy),
                        B::Zeroed | B::ZeroedBounded => Some(A::BytesZeroed),
                        _ => None,
                    };
                }
                use crate::vec_ops::VecOp as V;
                if let Some(operation) = crate::vec_ops::by_id(callee.as_str()) {
                    return match operation {
                        V::WithCapacity | V::ReserveExact | V::ReserveOwned => Some(A::VecBacking),
                        V::CloneAt => Some(A::VecElementClone),
                        _ => None,
                    };
                }
                use crate::map_ops::MapOp as M;
                if let Some(operation) = crate::map_ops::by_id(callee.as_str()) {
                    return match operation {
                        M::New => Some(A::MapBacking),
                        M::Add | M::Set if type_arguments.first() == Some(&ResolvedType::String) => Some(A::MapKeyClone),
                        M::KeyAt if expression.ty == ResolvedType::String => Some(A::StringClone),
                        M::SetNew => Some(A::SetBacking),
                        M::SetInsert if type_arguments.first() == Some(&ResolvedType::String) => Some(A::SetKeyClone),
                        M::SetKeyAt if expression.ty == ResolvedType::String => Some(A::StringClone),
                        _ => None,
                    };
                }
                if crate::box_ops::by_id(callee.as_str()) == Some(crate::box_ops::BoxOp::New) {
                    Some(A::BoxBacking)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
