//! Complete 4ec shared-sealing receiver before production stage attribution.
use super::*;
pub(super) fn prepare_before_stage<'a, T>(
    seed: &ConstructorSeed<'_>,
    program: &'a ResolvedProgram,
    seal: impl FnOnce(Storage<'a>, Payer<'_>) -> Result<T, Diagnostic>,
) -> Result<(T, Deferred<'a>), Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::CanonicalLoanStorage,
    );
    if !std::ptr::eq(seed.program(), program) {
        return Err(hir_error(
            "canonical shared-loan constructor belongs to another program",
        ));
    }
    // Acquisition follows the unchanged ModuleIndex and ConstructorSeed
    // phases. One report/ledger is held across ALL plans and their append; a
    // storage-only test injection keeps its separate legacy payer below.
    let captured = if crate::bounded_output::active_remaining().is_some() {
        Some(CfgReservation::acquire().map_err(|_| refused())?)
    } else {
        None
    };
    let payer = captured
        .as_ref()
        .map_or_else(Payer::current, Payer::Captured);
    let result = (|| {
        payer.reserve(0)?;
        let mut deferred = Deferred::new(program);
        let mut storage = Storage {
            program,
            ids: Vec::new(),
            liveness: Vec::new(),
            origins: Vec::new(),
            _scratch: Scratch::default(),
        };
        for function in program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function))
        {
            let plan = match captured.as_ref() {
                Some(owner) => {
                    match crate::loan_plan::build_plan_with_reservation(program, function, owner) {
                        Ok(plan) => plan,
                        Err(crate::loan_plan::FundedError::Physical(reason)) => {
                            // The tag is copied from this exact owner's first failure.
                            debug_assert_eq!(owner.first(), Some(reason));
                            return Err(refused());
                        }
                        Err(crate::loan_plan::FundedError::Semantic(error)) => return Err(error),
                    }
                }
                None => crate::loan_plan::build_plan(program, function)?,
            };
            // Compare the COMPLETE plan before append consumes it. An
            // attached plan is never a sizing/admission source.
            deferred.compare(function, &plan);
            append(&mut storage, plan, payer)?;
        }
        Ok((seal(storage, payer)?, deferred))
    })();
    // Only this exact affine owner can mark physical failure. Never infer
    // payment from H006 text, and never reserve/render after semantic error.
    if let Some(owner) = captured {
        if owner.first().is_some() {
            return Err(owner.diagnostic());
        }
    }
    result
}
