//! Explicit identity-only payment through the original two resolution passes.
//! Source and semantic errors remain owned compatibility diagnostics. This
//! private route changes neither verifier authority nor selected admission.
use super::*;
use declaration_index::identity_payer::IdentityIndexError;

#[derive(Debug)]
pub(crate) enum Failure<E> {
    Source(Vec<Diagnostic>),
    Semantic(Diagnostic),
    Identity(E),
}
impl<E> From<IdentityIndexError<E>> for Failure<E> {
    fn from(error: IdentityIndexError<E>) -> Self {
        match error {
            IdentityIndexError::Semantic(error) => Self::Semantic(error),
            IdentityIndexError::Identity(error) => Self::Identity(error),
        }
    }
}

/// One explicit callback survives both invariant passes. It cannot replace
/// IDs or authorize a source shape; full ordinary verification runs first.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private identity-only caller migration; ordinary API remains legacy"
    )
)]
pub(crate) fn resolve<E>(
    program: &Program,
    private: Option<source_verify::private_names::PrivateNames<'_>>,
    mut reserve: impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    let diagnostics = source_verify::verify_presenting(program, private);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(Failure::Source(diagnostics));
    }
    let clause_program = record_invariants::clause_program(program);
    let inventory_program = clause_program.as_ref().unwrap_or(program);
    let resolved = pass(inventory_program, &mut reserve)?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass(&desugared, &mut reserve)
}

fn pass<E>(
    program: &Program,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    // Preserve the old Resolver field-evaluation order before index creation.
    let byte_helpers = crate::loop_calls::byte_writer::SourceIndex::new(program);
    let declarations = DeclarationIndex::from_verified_with_identity_reservation(
        program,
        crate::prelude::declarations_for_program(program),
        reserve,
    )?;
    (Resolver {
        program,
        byte_helpers,
        declarations,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve()
    .map_err(Failure::Semantic)
}

/// Actual caller acquires the fixed report BEFORE invoking either pass.
/// Setup is Copy-only; owned Source/Semantic errors are explicitly unfunded.
#[derive(Debug)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private identity-only outward route remains inactive"
    )
)]
pub(crate) enum CallerFailure {
    Setup(crate::bounded_output::identity_reservation::Refused),
    Legacy(Failure<std::convert::Infallible>),
    Identity(Vec<Diagnostic>),
}
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private identity-only caller migration; ordinary API remains legacy"
    )
)]
pub(crate) fn resolve_with_report(
    program: &Program,
    private: Option<source_verify::private_names::PrivateNames<'_>>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), CallerFailure> {
    resolve_with_report_observation(program, private, |_| {})
}
fn resolve_with_report_observation(
    program: &Program,
    private: Option<source_verify::private_names::PrivateNames<'_>>,
    observe: impl FnOnce(&crate::bounded_output::identity_reservation::IdentityReservation),
) -> Result<(ResolvedProgram, FunctionResolutionWork), CallerFailure> {
    let mut owner = crate::bounded_output::identity_reservation::IdentityReservation::acquire()
        .map_err(CallerFailure::Setup)?;
    observe(&owner);
    match resolve(program, private, |bytes| owner.reserve(bytes)) {
        Ok(value) => Ok(value),
        Err(Failure::Identity(_)) => Err(CallerFailure::Identity(owner.publish())),
        Err(Failure::Source(error)) => Err(CallerFailure::Legacy(Failure::Source(error))),
        Err(Failure::Semantic(error)) => Err(CallerFailure::Legacy(Failure::Semantic(error))),
    }
}

#[cfg(test)]
mod tests;
