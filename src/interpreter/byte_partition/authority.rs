//! Immutable loan-plan authority for the pending ordinary partition factory.
//! No constructor is exposed from raw HIR in production. Its source receiver
//! borrows only the sealed original-source owner after its full compiler gates.
//! The ordinary evaluator installer still remains closed.
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
    /// Only the immutable original-source owner can fund a production loan
    /// receiver. Raw HIR, copied declarations and matching stable IDs cannot.
    pub(crate) fn from_source(
        owner: &'p crate::hir::byte_partition::ValidatedPartitionProgram<'_>,
        function: &'p ResolvedFunction,
        receiving_work: &mut usize,
    ) -> Result<Self, Refusal> {
        let result = (|| {
            owner
                .receive_current_graph(receiving_work)
                .map_err(|_| Refusal::Identity)?;
            let program = owner.program();
            let mut found = false;
            for current in &program.functions {
                *receiving_work = receiving_work.checked_sub(1).ok_or(Refusal::Metadata)?;
                if std::ptr::eq(current, function) {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(Refusal::Identity);
            }
            if function.loan_plan.loans.len() > crate::loan_plan::MAX_LOANS_PER_FUNCTION_V1 {
                return Err(Refusal::Metadata);
            }
            Ok(Self { program, function })
        })();
        if result.is_err() {
            *receiving_work = 0;
        }
        result
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
