//! Initial attached plans use the existing caller's actual CFG reservation.
//! This is construction, never authority for the later independent rebuild.
use super::{Diagnostic, LoanPlan, ResolvedFunction, ResolvedProgram};
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::staged;

#[derive(Clone, Copy)]
enum Group {
    Authored,
    Instances,
}
impl Group {
    fn len(self, program: &ResolvedProgram) -> usize {
        match self {
            Self::Authored => program.functions.len(),
            Self::Instances => program.function_instances.len(),
        }
    }
    fn function(self, program: &ResolvedProgram, index: usize) -> &ResolvedFunction {
        match self {
            Self::Authored => &program.functions[index],
            Self::Instances => &program.function_instances[index].function,
        }
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io("SPX-H006", String::new())
}

pub(in crate::hir) fn attach(program: &mut ResolvedProgram) -> Result<(), Diagnostic> {
    // One exact ledger/report spans both groups. Acquisition grants no quota.
    let captured = if crate::bounded_output::active_remaining().is_some() {
        Some(CfgReservation::acquire().map_err(|_| refused())?)
    } else {
        None
    };
    let result = (|| {
        let plans = staged!(
            HirLoanPlans,
            collect(program, Group::Authored, captured.as_ref())
        )?;
        for (function, plan) in program.functions.iter_mut().zip(plans) {
            function.loan_plan = plan;
        }
        // Preserve the original authored attachment before instance planning.
        let plans = staged!(
            HirInstanceLoanPlans,
            collect(program, Group::Instances, captured.as_ref())
        )?;
        for (instance, plan) in program.function_instances.iter_mut().zip(plans) {
            instance.function.loan_plan = plan;
        }
        Ok(())
    })();
    if let Some(owner) = captured {
        if owner.first().is_some() {
            return Err(owner.diagnostic());
        }
    }
    result
}

fn collect(
    program: &ResolvedProgram,
    group: Group,
    captured: Option<&CfgReservation>,
) -> Result<Vec<LoanPlan>, Diagnostic> {
    let Some(owner) = captured else {
        // Keep the original unobserved planner and exact-size collection route.
        return (0..group.len(program))
            .map(|index| crate::loan_plan::build_plan(program, group.function(program, index)))
            .collect();
    };
    owner.reserve(0).map_err(|_| refused())?;
    let mut plans = Vec::new();
    for index in 0..group.len(program) {
        // Build in original source order. A semantic error is moved unchanged;
        // no attached plan sizes or authorizes this independent constructor.
        let plan = match crate::loan_plan::build_plan_with_reservation(
            program,
            group.function(program, index),
            owner,
        ) {
            Ok(plan) => plan,
            Err(crate::loan_plan::FundedError::Physical(reason)) => {
                debug_assert_eq!(owner.first(), Some(reason));
                return Err(refused());
            }
            Err(crate::loan_plan::FundedError::Semantic(error)) => return Err(error),
        };
        // Preserve the original fallible collection's first-element-before-
        // storage order. The paid route deliberately uses an exact directory;
        // the original fallible shunt did not guarantee that backing shape.
        if plans.is_empty() {
            let count = group.len(program);
            owner.array::<LoanPlan>(count).map_err(|_| refused())?;
            let requested = std::alloc::Layout::array::<LoanPlan>(count)
                .expect("array layout checked by exact owner")
                .size();
            if plans.try_reserve_exact(count).is_err() {
                owner.allocation(requested);
                return Err(refused());
            }
            if plans.capacity() != count {
                let excess = plans
                    .capacity()
                    .saturating_sub(count)
                    .saturating_mul(std::mem::size_of::<LoanPlan>());
                let _ = owner.capacity(requested, excess);
                return Err(refused());
            }
        }
        owner.reserve(0).map_err(|_| refused())?;
        plans.push(plan);
    }
    Ok(plans)
}

#[cfg(test)]
mod tests;
