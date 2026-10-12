//! Selected replay of Row Loan v1: a cleanup-inert borrowed leaf whose two
//! children are the exact whole-Vec carrier and Copy index of a current flat
//! owned-leaf record. It owns no storage, status, commit or transition.
use super::*;

pub(in crate::cleanup_plan::replay) fn validate(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let ResolvedExprKind::RowLoan { loan, args } = &expression.kind else {
        return Ok(());
    };
    collections::prepay(
        program,
        function,
        collections::type_key_bytes(&loan.element)?,
        budget,
    )?;
    if crate::cleanup::selected_flat_record_fields(program, &loan.element)?.is_none() {
        return Err(replay_error(
            function,
            "selected row loan element is not a current flat record",
        ));
    }
    budget.charge(function, 8, "selected row loan facts")?;
    let [owner, _] = args.as_slice() else {
        return Err(replay_error(
            function,
            "selected row loan has invalid arity",
        ));
    };
    if !crate::cleanup_plan::row_loan::exact_places(args)
        || crate::cleanup::selected_vector_element(program, &owner.ty)? != Some(&loan.element)
        || expression.ty != loan.element
        || expression.ownership != OwnershipMode::Borrow
    {
        return Err(replay_error(
            function,
            "selected row loan carrier, index or borrowed result disagrees",
        ));
    }
    Ok(())
}
