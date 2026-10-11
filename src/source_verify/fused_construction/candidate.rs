//! Private, explicit `FusedConstructionV1` source candidate route.
//!
//! The route takes one parsed `Program` and nothing else: no reader, path,
//! library trust or ambient authority. It independently derives the single
//! selected [`SourceProof`] from that exact AST, authenticates it, and runs
//! every ordinary source pass with the held selection. The selection changes
//! only its own loop: the exact producer call is admitted in the while body,
//! and the loop's capacity flow becomes a `FusedConstruction` node at the
//! proof's actual producer site, analyzed under `FusedConstructionV1`. No row
//! count is rewritten, no check is skipped and no limit is raised.
//!
//! Success is a [`ProvisionalFusedSource`], which is always
//! [`CandidateStatus::UnvalidatedUntilHirJoin`]. It is not a checked program
//! and grants no permission to emit, cache, graph or publish anything. Three
//! steps must follow before any validated program or artifact exists: an
//! independent typed-HIR derivation, an exact join with this held source
//! proof, and full selected HIR validation. Then each of the three executors
//! must close the lease domain. The default `check` path never reaches this
//! module.
use super::{derive, Derivation, SourceProof};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;

/// The only status a source candidate can have.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateStatus {
    UnvalidatedUntilHirJoin,
}

impl CandidateStatus {
    pub(crate) const fn describe(self) -> &'static str {
        match self {
            Self::UnvalidatedUntilHirJoin => {
                "fused construction candidate UNVALIDATED until independent typed-HIR \
                 derivation, exact source join and full selected HIR validation"
            }
        }
    }
}

/// Why the private route produced no candidate.
#[derive(Debug)]
pub(crate) enum CandidateRefusal {
    /// No function derives a selected proof. Legacy counts (32 or fewer)
    /// belong to the default route.
    NoSelection,
    /// More than one function derives a selected proof.
    Ambiguous,
    /// The derived proof failed to re-authenticate against the program.
    NotHeld,
    /// An ordinary source pass refused under the held selection.
    Source(Vec<Diagnostic>),
}

/// A source-checked selection that is still unvalidated. It holds the proof
/// borrowed from the exact program it checked and exposes nothing else.
pub(crate) struct ProvisionalFusedSource<'p> {
    proof: SourceProof<'p>,
    status: CandidateStatus,
}

impl<'p> ProvisionalFusedSource<'p> {
    pub(crate) fn proof(&self) -> &SourceProof<'p> {
        &self.proof
    }
    pub(crate) fn status(&self) -> CandidateStatus {
        self.status
    }
}

/// Run the private candidate route over `program`.
pub(crate) fn check_candidate(
    program: &Program,
) -> Result<ProvisionalFusedSource<'_>, CandidateRefusal> {
    let mut selected = None;
    for function in &program.functions {
        if let Ok(Derivation::Selected(proof)) = derive(program, function) {
            if selected.replace(proof).is_some() {
                return Err(CandidateRefusal::Ambiguous);
            }
        }
    }
    let proof = selected.ok_or(CandidateRefusal::NoSelection)?;
    if !proof.authenticate(program) {
        return Err(CandidateRefusal::NotHeld);
    }
    let diagnostics = crate::source_verify::declaration::verify_selected(program, Some(&proof), None);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(CandidateRefusal::Source(diagnostics));
    }
    Ok(ProvisionalFusedSource {
        proof,
        status: CandidateStatus::UnvalidatedUntilHirJoin,
    })
}

#[cfg(test)]
mod tests;
