//! Test-only funded replay diagnostic. `replay_error` formats the message into
//! one temporary String, wraps it in a second, and callers often format a third
//! before the call. This receiver takes the message as borrowed
//! `fmt::Arguments`, counts the exact bytes with an allocation-free writer,
//! debits that complete backing on the account, and only then builds the single
//! String that the Diagnostic retains (no temporary overlaps it). The
//! Diagnostic carries the original six fields and text byte for byte. The
//! production `replay_error` is untouched and remains the old oracle.
use super::{OracleLease, OracleRefusal};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedFunction;
use std::fmt::{self, Write};

/// Failure of the funded receiver. Every variant is inline: refusing never
/// allocates, so an exhausted account cannot be asked to pay for its own error.
#[derive(Debug)]
pub(in crate::cleanup_plan::replay) enum ReplayFailure {
    Diagnostic(Diagnostic),
    Account(OracleRefusal),
    Physical(&'static str),
}

impl ReplayFailure {
    /// Stable, allocation-free carrier: the account or physical failure stays
    /// visible in the text and is never turned into a semantic verdict.
    pub(in crate::cleanup_plan::replay) fn into_diagnostic(self) -> Diagnostic {
        match self {
            Self::Diagnostic(diagnostic) => diagnostic,
            Self::Account(_) => {
                Diagnostic::io_static("SPX-H006", "oracle account refused a replay diagnostic")
            }
            Self::Physical(reason) => Diagnostic::io_static("SPX-H006", reason),
        }
    }
}

/// Allocator or layout failure: recorded on the account (sticky even with
/// bytes left) and returned inline.
fn physical(lease: OracleLease, reason: &'static str) -> ReplayFailure {
    lease.fail_physical(reason);
    ReplayFailure::Physical(reason)
}

struct Count(usize);

impl fmt::Write for Count {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

pub(in crate::cleanup_plan::replay) fn replay_error_funded(
    lease: OracleLease,
    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    let mut count = Count(0);
    if write!(
        count,
        "cleanup plan for function `{}` failed independent replay: {}",
        function.id, message
    )
    .is_err()
    {
        return Err(physical(lease, "replay diagnostic length overflowed"));
    }
    let bytes = count.0;
    lease.charge(bytes).map_err(ReplayFailure::Account)?;
    let mut text = String::new();
    if text.try_reserve_exact(bytes).is_err() {
        return Err(physical(lease, "replay diagnostic allocation failed"));
    }
    if text.capacity() != bytes {
        return Err(physical(
            lease,
            "replay diagnostic capacity differs from its reservation",
        ));
    }
    if write!(
        text,
        "cleanup plan for function `{}` failed independent replay: {}",
        function.id, message
    )
    .is_err()
        || text.len() != bytes
    {
        return Err(physical(
            lease,
            "replay diagnostic text differs from its counted bytes",
        ));
    }
    Ok(Diagnostic::io("SPX-H006", text))
}
