//! Test-only funded twin of `owner`. Candidate order, the `validate_owner`
//! predicate and the first-duplicate ambiguity stop are the original's. Nothing
//! builds the discarded legacy diagnostics: a missing or ambiguous owner is the
//! plain `OwnerVerdict`, whose `diagnostic()` rebuilds the original text only
//! for a caller that actually needs it. Each candidate execution ID is paid on
//! the caller's payer before it is constructed: a monomorphic declaration ID is
//! a shared identity (0 payload bytes); a generic instance ID owns its full
//! identity text. A payer refusal is returned unchanged, never read as a
//! missing owner.
use super::FunctionExecutionId;
use crate::diagnostic::Diagnostic;
use crate::hir::funded::{self, FundedError, Reserve};
use crate::hir::ResolvedFunction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerVerdict {
    Missing,
    Ambiguous,
}

impl OwnerVerdict {
    pub(crate) fn diagnostic(self) -> Diagnostic {
        match self {
            Self::Ambiguous => Diagnostic::io(
                "SPX-H006",
                "lexical function execution owner is ambiguous",
            ),
            Self::Missing => Diagnostic::io(
                "SPX-H006",
                "unregistered lexical function requires an explicit authenticated execution context",
            ),
        }
    }
}

pub(crate) fn owner_funded<E>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<Result<FunctionExecutionId, OwnerVerdict>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut found = None;
    for declaration in program.functions() {
        funded::charge(reserve, 0)?;
        let candidate = FunctionExecutionId::Monomorphic(declaration.id.clone());
        if function.result_id.matches_result(&candidate)
            && function.body.id.matches(&candidate, "body")
            && found.replace(candidate).is_some()
        {
            return Ok(Err(OwnerVerdict::Ambiguous));
        }
    }
    for instance in program.function_instances() {
        funded::charge(reserve, instance.id.as_str().len())?;
        let candidate = FunctionExecutionId::Generic(instance.id.clone());
        if function.result_id.matches_result(&candidate)
            && function.body.id.matches(&candidate, "body")
            && found.replace(candidate).is_some()
        {
            return Ok(Err(OwnerVerdict::Ambiguous));
        }
    }
    Ok(found.ok_or(OwnerVerdict::Missing))
}

/// Funded `requires`: the scan is a fixed finite stack with no owning frontier,
/// so its real cursor layout is paid first. The only allocation reachable is
/// the original depth-overflow diagnostic text, paid at its full length before
/// the scan can build it (an upper bound; unused on the normal route).
pub(crate) fn requires_funded<E>(
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    const DEPTH_TEXT: &str = "lexical control scan exceeds admitted expression depth";
    funded::charge(
        reserve,
        std::mem::size_of::<[Option<super::ChildCursor<'_>>; crate::cache_codec::MAX_DEPTH + 1]>(),
    )?;
    funded::charge(reserve, DEPTH_TEXT.len())?;
    Ok(super::requires(function).unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funded_owner_matches_the_original_and_refuses_unchanged() {
        let checked = crate::check(
            "module owners; @id(\"main\") fn main()->i64 {0}",
            "owner-funded.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = &program.functions[0];
        let old = super::super::owner(&program, function).unwrap();
        let mut paid = 0usize;
        let mut pay = |bytes: usize| -> Result<(), &'static str> {
            paid += bytes;
            Ok(())
        };
        assert_eq!(owner_funded(&program, function, &mut pay), Ok(Ok(old)));
        let mut refuse = |_: usize| -> Result<(), &'static str> { Err("refused") };
        assert_eq!(
            owner_funded(&program, function, &mut refuse),
            Err(FundedError::Refused("refused"))
        );
        let mut pay = |_: usize| -> Result<(), &'static str> { Ok(()) };
        assert_eq!(requires_funded(function, &mut pay), Ok(false));
        assert_eq!(
            OwnerVerdict::Missing.diagnostic().message,
            "unregistered lexical function requires an explicit authenticated execution context"
        );
    }
}

mod work;
pub(crate) use work::{owner_funded_with_work, requires_funded_with_work};
