//! Immutable loan-plan authority for the pending ordinary partition factory.
//! No constructor is exposed from raw HIR in production. The eventual full
//! source/HIR factory must publish this only after all its ordinary gates; the
//! owning tests construct it by actual complete HIR validation.
use crate::{
    construction_fusion::domain::Refusal,
    hir::{ResolvedFunction, ResolvedProgram},
};
pub(crate) struct LoanCapacity<'p> {
    program: &'p ResolvedProgram,
    function: &'p ResolvedFunction,
}
impl<'p> LoanCapacity<'p> {
    pub(crate) fn slots(&self) -> usize {
        crate::loan_plan::MAX_LOANS_PER_FUNCTION_V1
    }
    pub(super) fn program(&self) -> &'p ResolvedProgram {
        self.program
    }
    pub(super) fn function(&self) -> &'p ResolvedFunction {
        self.function
    }
    pub(super) fn matches(&self, other: &Self) -> bool {
        std::ptr::eq(self.program, other.program) && std::ptr::eq(self.function, other.function)
    }
    #[cfg(test)]
    pub(super) fn checked(
        program: &'p ResolvedProgram,
        function: &'p ResolvedFunction,
    ) -> Result<Self, Refusal> {
        if !program
            .functions
            .iter()
            .any(|current| std::ptr::eq(current, function))
        {
            return Err(Refusal::Identity);
        }
        crate::hir::validate(program).map_err(|_| Refusal::Identity)?;
        if function.loan_plan.loans.len() > crate::loan_plan::MAX_LOANS_PER_FUNCTION_V1 {
            return Err(Refusal::Metadata);
        }
        Ok(Self { program, function })
    }
}
