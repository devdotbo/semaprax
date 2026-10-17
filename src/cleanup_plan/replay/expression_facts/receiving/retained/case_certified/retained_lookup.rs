//! Closed first-lookup continuation. No callbacks, ACTIVE switch or guard bank.
use super::directory::HeldPrefix;
use super::*;
use crate::cleanup_plan::replay::expression_facts::arena::{retained::Session, Arena};
use std::mem::size_of;

// Selection, checked acquisition sum, helper actual/formal transports and
// both helper Result receivers. Paid before any new owner/continuation forms.
pub(super) const SELECT_BYTES: usize = 4 * size_of::<bool>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<Option<usize>>()
    + 8 * size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<(
        &ResolvedFunction,
        &Arena<'_>,
        &mut ReplayBudget,
        &case_proof::StatusCases<'_>,
        &mut HeldPrefix<'_, '_>,
        &mut usize,
    )>();
const A: usize = SELECT_BYTES + Session::acquisition_bytes();
const S: usize = Session::OLD_SCRATCH;
const R: usize = Session::query_bytes();
// Conservative full-composition break-even; no success-count claim for an
// early semantic error. Preserve the complete first old scratch tariff too.
pub(super) const MIN_ROWS: usize = if S > R {
    (A + S) / (S - R) + 1
} else {
    usize::MAX
};

pub(super) fn complete_rows<'hir>(
    function: &'hir ResolvedFunction,
    arena: &Arena<'hir>,
    budget: &mut ReplayBudget,
    proof: &case_proof::StatusCases<'hir>,
    prefix_owner: &mut HeldPrefix<'_, 'hir>,
    ordinal: &mut usize,
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(
        function,
        S + Session::acquisition_bytes(),
        "retained expression-fact owner acquisition",
    )?;
    budget.charge(
        function,
        12,
        "retained expression-fact owner initialization",
    )?;
    let mut session = Session::new_prepaid(arena, function, budget);
    loop {
        let source = &function.cleanup_plan.status_sources[*ordinal];
        let Some(fact) = session.fact(&source.id.expression)? else {
            return Err(<Diagnostic as semantic_refusal::Error>::structural(
                function,
                semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
            ));
        };
        let budget = &mut *session.budget;
        match (&source.id.lane, &source.producer) {
            (
                StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                StatusProducer::PropagatedCall { callee },
            ) => {
                // The payload reference is a second simultaneous receiving
                // control; the eliminated lookup's result is not a credit.
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<&CallFact<'_>>(),
                    "retained status call receiving control",
                )?;
                budget.charge(function, 1, "retained status call receiving assignment")?;
                let Some(call) = fact else {
                    return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    ));
                };
                if call.callee.as_ref() != callee {
                    return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    ));
                }
            }
            (
                StatusLane::OperationFailure,
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                },
            ) => {
                // Complete independent discovery proved the exact held row.
                // No case value is read here and no second case directory exists.
                proof.member(function, source, *ordinal, budget)?;
                let _ = normalized_cases;
            }
            (
                StatusLane::AllocationFailure,
                StatusProducer::CheckedAllocation {
                    normalized_cases, ..
                },
            ) if function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
                && *normalized_cases
                    == [
                        crate::cleanup_plan::AllocationCase::PayloadRefused,
                        crate::cleanup_plan::AllocationCase::SizeOverflow,
                    ] => {}
            (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "status source lane does not match its producer",
                ));
            }
        }
        budget.charge(function, 1, "status case row ordinal increment")?;
        *ordinal += 1;
        if *ordinal == function.cleanup_plan.status_sources.len() {
            break;
        }
        let source = &function.cleanup_plan.status_sources[*ordinal];
        let budget = &mut *session.budget;
        if !prefix_owner.insert(function, &source.id, budget)? {
            return Err(<Diagnostic as semantic_refusal::Error>::structural(
                function,
                semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
            ));
        }
        // Retain the actual first lookup's borrowed payload, only through this
        // immutable source's producer match. Pay its complete receiving pointer
        // and one assignment step; no previous result/header credit is used.
        budget.reserve_bytes(
            function,
            std::mem::size_of::<&Option<CallFact<'_>>>(),
            "retained status fact receiving control",
        )?;
        budget.charge(function, 1, "retained status fact receiving assignment")?;
    }
    Ok(())
}
