//! Complete saved original contract-clause domain owner.
use super::super::super::*;
impl HirValidator<'_> {
    pub(in crate::hir::validation) fn validate_contracts_owned_reference(
        &mut self,
        execution: &FunctionExecutionId,
        contracts: &[ResolvedExpr],
        outer: &BTreeMap<ValueId, ValidationBinding>,
        phase: &str,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        for (index, contract) in contracts.iter().enumerate() {
            let mut scope = outer.clone();
            // Every clause starts a new freshness domain. Entry parameters,
            // body owners and the provisional result are protected even if
            // an internal caller retained an earlier temporary scope.
            for binding in scope.values_mut() {
                binding.contract_local = false;
            }
            self.validate_expr_iterative_owned_reference(
                execution,
                contract,
                &mut scope,
                &format!("{phase}.{index}"),
                false,
                None,
            )?;
            self.require_type(
                &contract.ty,
                &ResolvedType::Bool,
                if phase == "requires" {
                    "precondition"
                } else {
                    "postcondition"
                },
            )?;
        }
        Ok(())
    }
}
