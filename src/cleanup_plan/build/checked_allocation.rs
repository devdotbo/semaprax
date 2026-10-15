//! Candidate allocation branches. No default builder receives a source owner.
use super::super::{AllocationCase, AllocationOperation};
use super::*;

/// Candidate only: independent allocation replay and executor receiving remain
/// mandatory before this plan can authorize a kernel or output publication.
pub(in crate::cleanup_plan) fn candidate(
    current: super::super::checked_allocation::Function<'_>,
) -> Result<CleanupPlan, Diagnostic> {
    let program = current.program();
    let function = current.function();
    if !current.held_by(program, function) {
        return Err(plan_error(
            "allocation projection is not the current function",
        ));
    }
    // Lexical status predeclaration needs its own independent evaluated-site
    // join; do not attach v20 to an unjoined lexical certificate.
    if crate::hir::lexical_exit::requires(function)? {
        return Err(plan_error(
            "checked allocation lexical status join is pending",
        ));
    }
    let mut builder = PlanBuilder::new(program, function)?;
    builder.allocation = Some(current);
    let mut plan = builder.build()?;
    plan.schema = super::super::CLEANUP_PLAN_SCHEMA_V20;
    Ok(plan)
}

impl PlanBuilder<'_> {
    pub(super) fn checked_source(
        &mut self,
        expression: &ResolvedExpr,
        operation: CheckedOperation,
        normalized_cases: Vec<StatusCase>,
    ) -> Result<StatusSourceId, Diagnostic> {
        let source = StatusSourceId {
            expression: expression.id.clone(),
            lane: StatusLane::OperationFailure,
        };
        self.add_status_source(
            source.clone(),
            StatusProducer::CheckedArithmetic {
                operation,
                normalized_cases,
            },
        )?;
        Ok(source)
    }

    pub(super) fn split_allocation(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<(BlockId, FlowState), Diagnostic> {
        let Some(current) = self.allocation else {
            return Ok((block, state));
        };
        if !current.held_by(self.program, self.function) {
            return Err(plan_error(
                "allocation projection changed its current subject",
            ));
        }
        let Some(operation) = operation(expression) else {
            return Ok((block, state));
        };
        let source = StatusSourceId {
            expression: expression.id.clone(),
            lane: StatusLane::AllocationFailure,
        };
        self.add_status_source(
            source.clone(),
            StatusProducer::CheckedAllocation {
                operation,
                normalized_cases: [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow],
            },
        )?;
        self.split_status(block, state, region, source)
    }
}

/// Closed operation catalogue. Argument transfers and implicit String reads
/// are separately classified by the unchanged constructor-owned move sets.
/// User calls do not allocate a second copy of their returned carrier here;
/// their current callee plan owns all reached materializations.
pub(in crate::cleanup_plan) fn operation(expression: &ResolvedExpr) -> Option<AllocationOperation> {
    use crate::byte_ops::ByteOp as B;
    use crate::string_ops::StringOp as S;
    use AllocationOperation as A;
    match &expression.kind {
        ResolvedExprKind::String(_) => Some(A::StringLiteral),
        ResolvedExprKind::Place(_)
            if expression.ty == ResolvedType::String
                && expression.ownership == OwnershipMode::Own =>
        {
            Some(A::StringClone)
        }
        ResolvedExprKind::LiteralFormat { .. } => Some(A::LiteralFormat),
        ResolvedExprKind::Call {
            callee,
            instance: None,
            type_arguments,
            ..
        } => {
            if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                return match op {
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
            if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                return match op {
                    B::Copy => Some(A::BytesCopy),
                    B::Zeroed | B::ZeroedBounded => Some(A::BytesZeroed),
                    _ => None,
                };
            }
            if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
                use crate::vec_ops::VecOp as V;
                return match op {
                    V::WithCapacity | V::ReserveExact | V::ReserveOwned => Some(A::VecBacking),
                    V::CloneAt => Some(A::VecElementClone),
                    _ => None,
                };
            }
            if let Some(op) = crate::map_ops::by_id(callee.as_str()) {
                use crate::map_ops::MapOp as M;
                return match op {
                    M::New => Some(A::MapBacking),
                    M::Add | M::Set if type_arguments.first() == Some(&ResolvedType::String) => {
                        Some(A::MapKeyClone)
                    }
                    M::KeyAt if expression.ty == ResolvedType::String => Some(A::StringClone),
                    M::SetNew => Some(A::SetBacking),
                    M::SetInsert if type_arguments.first() == Some(&ResolvedType::String) => {
                        Some(A::SetKeyClone)
                    }
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
