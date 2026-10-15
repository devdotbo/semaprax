//! Resolved parameter signatures carried by iterative cleanup call frames.
//!
//! Authored and generic targets are read-only program inventory and stay
//! borrowed. Compiler intrinsic signatures are synthesized per call and stay
//! owned, so their construction checks remain with the intrinsic resolvers.
use std::borrow::Cow;
use std::fmt::Display;

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, FunctionInstanceId, ResolvedParam, ResolvedProgram};

use super::plan_error;

pub(super) type CallParams<'a> = Cow<'a, [ResolvedParam]>;

/// Borrow the exact parameter inventory of an authored or generic target,
/// refusing an unknown target or an argument count that differs from it.
pub(super) fn authored<'p>(
    program: &'p ResolvedProgram,
    callee: &DeclarationId,
    instance: Option<&FunctionInstanceId>,
    arity: usize,
    call: impl Display,
) -> Result<CallParams<'p>, Diagnostic> {
    let target = program
        .resolve_call_target(callee, instance)
        .ok_or_else(|| plan_error(format!("unknown cleanup call target `{callee}`")))?;
    if target.params.len() != arity {
        return Err(plan_error(format!(
            "cleanup call `{call}` has inconsistent arity"
        )));
    }
    Ok(Cow::Borrowed(&target.params[..]))
}

/// Heap bytes a call frame owns for its signature: nothing when borrowed.
#[cfg(test)]
pub(super) fn owned_capacity(params: &CallParams<'_>) -> usize {
    match params {
        Cow::Borrowed(_) => 0,
        Cow::Owned(owned) => {
            owned.capacity() * std::mem::size_of::<ResolvedParam>()
                + owned
                    .iter()
                    .map(super::resolved_param_owned_capacity)
                    .sum::<usize>()
        }
    }
}
