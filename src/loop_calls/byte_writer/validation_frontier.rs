//! Validation-only demand frontier over one exact immutable program.
//! Inert ordinal cells never census or prove a future body. Construction and
//! final replay have independent owners and do not borrow this phase's memo.
use super::*;
use crate::cleanup_plan::renewal_profile::ordinary::{self, Admission, Directory, Family};
use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, FunctionExecutionId, ResolvedProgram};
use std::cell::{Cell, OnceCell, RefCell};
mod ordinary_receiver;
mod storage;
use storage::reserve;
#[cfg(test)]
mod tests;
struct Row<'p> {
    member: ordinary::CurrentMember<'p>,
    helpers: OnceCell<HelperBindings<'p>>,
    families: [OnceCell<Directory<'p>>; 4],
}
struct ProofState<'p> {
    census: Vec<resolved::CensusCursor<'p>>,
    proof: Vec<resolved::ProofCursor<'p>>,
    positives: Vec<&'p ResolvedFunction>,
    proof_refused: bool,
}
pub(crate) struct Frontier<'p> {
    program: &'p ResolvedProgram,
    row: OnceCell<Row<'p>>,
    ready: Cell<Option<usize>>,
    funded: Cell<bool>,
    next: Cell<usize>,
    possible: bool,
    state: RefCell<ProofState<'p>>,
    work: Cell<usize>,
    failed: Cell<bool>,
    probes: OnceCell<crate::hir::renewal_probes::phase::Probes<'p>>,
    #[cfg(test)]
    proof_count: Cell<usize>,
}
/// Only Frontier::current can construct this exact current member capability.
/// Its execution owner exists only on the old selective lexical branch.
pub(crate) struct Current<'f, 'p> {
    frontier: &'f Frontier<'p>,
    ordinal: usize,
    row: &'f Row<'p>,
    owner: Option<FunctionExecutionId>,
}
/// A builder may lend this receiver, but cannot implement or mint Current.
pub(crate) trait Ordinary<'a> {
    fn matches(&self, program: &ResolvedProgram, function: &ResolvedFunction) -> bool;
    fn has(&self, family: Family) -> bool;
    fn binding(&self, at: &ExpressionId) -> Option<&'a ResolvedBinding>;
    fn complete(&self) -> Result<(), Diagnostic>;
    #[cfg(test)]
    fn owned_capacity_bytes(&self) -> usize;
}
fn error() -> Diagnostic {
    resolved::census_error()
}
impl<'p> Frontier<'p> {
    pub(crate) fn new(program: &'p ResolvedProgram) -> Self {
        Self {
            program,
            row: OnceCell::new(),
            ready: Cell::new(None),
            funded: Cell::new(false),
            next: Cell::new(0),
            possible: resolved::potential(program),
            state: RefCell::new(ProofState {
                census: Vec::new(),
                proof: Vec::new(),
                positives: Vec::new(),
                proof_refused: false,
            }),
            work: Cell::new(0),
            failed: Cell::new(false),
            probes: OnceCell::new(),
            #[cfg(test)]
            proof_count: Cell::new(0),
        }
    }
    fn units(&self, units: usize) -> bool {
        if self.failed.get() {
            return false;
        }
        match self.work.get().checked_add(units) {
            Some(left) => {
                self.work.set(left);
                true
            }
            None => {
                self.failed.set(true);
                false
            }
        }
    }
    pub(crate) fn current(&self) -> Result<Current<'_, 'p>, Diagnostic> {
        if self.ready.get().is_some() {
            return Err(error());
        }
        let ordinal = self.next.get();
        let member = ordinary::current_member_at(self.program, ordinal).ok_or_else(error)?;
        let function = member.function();
        // Exact original selective owner order, before helper census or any
        // ordinary family. Nonlexical members gain no explicit ID check.
        let owner = if crate::hir::lexical_exit::requires(function)? {
            let owner = crate::hir::lexical_exit::owner(self.program, function)?;
            crate::hir::lexical_exit::validate_owner(function, &owner)?;
            if crate::hir::lexical_exit::requires(function)? {
                crate::hir::lexical_exit::validate_function(self.program, function, &owner)?;
            }
            Some(owner)
        } else {
            None
        };
        // No phase backing is forced before the old selective lexical probes.
        let row = self.row(member)?;
        Ok(Current {
            frontier: self,
            ordinal,
            row,
            owner,
        })
    }
    pub(crate) fn advance(&mut self) -> Result<(), Diagnostic> {
        // A consumed exact Current marks this ordinal. Rust requires every
        // builder/current borrow to end before this mutable receiver can run.
        if self.ready.get() != Some(self.next.get()) {
            return Err(error());
        }
        let next = self.next.get().checked_add(1).ok_or_else(error)?;
        let row = self.row.take().ok_or_else(error)?;
        self.next.set(next);
        self.ready.set(None);
        // Release only member facts. Physical debits and phase proof state
        // remain cumulative; no reservation or work allowance is refunded.
        drop(row);
        Ok(())
    }
    fn helpers<'f>(&'f self, row: &'f Row<'p>) -> Result<&'f HelperBindings<'p>, Diagnostic> {
        if row.helpers.get().is_none() {
            let sites = if self.possible {
                let function = row.member.function();
                let mut state = self.state.borrow_mut();
                if state.census.is_empty() {
                    // Both constructor stack and stable backing are real storage.
                    state.census = reserve(1)?;
                    state.census.push(resolved::CensusCursor::new(function)?);
                }
                let ProofState {
                    census,
                    proof,
                    positives,
                    proof_refused,
                } = &mut *state;
                resolved::checked_bindings_with(function, &mut census[0], |binding, value| {
                    let ResolvedExprKind::Call {
                        callee,
                        instance: None,
                        ..
                    } = &value.kind
                    else {
                        return Ok(false);
                    };
                    // Preserve the original first duplicate declaration winner.
                    let Some(index) = self
                        .program
                        .functions
                        .iter()
                        .position(|target| target.id == *callee)
                    else {
                        return Ok(false);
                    };
                    if !self.units(self.program.functions.len()) {
                        return Err(error());
                    }
                    let target = &self.program.functions[index];
                    let proved = if crate::bounded_output::active_refused() || *proof_refused {
                        false
                    } else if positives
                        .iter()
                        .any(|candidate| std::ptr::eq(*candidate, target))
                    {
                        true
                    } else {
                        #[cfg(test)]
                        self.proof_count.set(self.proof_count.get() + 1);
                        let mut proved = resolved::validation::target(self.program, target, proof);
                        *proof_refused |= crate::bounded_output::active_refused();
                        if proved {
                            proved = storage::remember(positives, target);
                            *proof_refused |= crate::bounded_output::active_refused();
                        }
                        proved
                    };
                    Ok(assignment_with_target(
                        self.program,
                        binding,
                        value,
                        target,
                        proved,
                    ))
                })?
            } else {
                HelperBindings::default()
            };
            row.helpers.set(sites).map_err(|_| error())?;
        }
        row.helpers.get().ok_or_else(error)
    }
    fn row(&self, member: ordinary::CurrentMember<'p>) -> Result<&Row<'p>, Diagnostic> {
        if !self.funded.get() {
            // Self includes the one inline row. No phase or row backing is
            // funded before the original selective lexical-owner probes.
            if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Self>()) {
                return Err(error());
            }
            self.funded.set(true);
        }
        if self.row.get().is_none() {
            // This is the literal current initializer stack, not a future-row
            // forecast or heap request. Row storage itself is inline in Self.
            if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Row<'p>>()) {
                return Err(error());
            }
            self.row
                .set(Row {
                    member,
                    helpers: OnceCell::new(),
                    families: std::array::from_fn(|_| OnceCell::new()),
                })
                .map_err(|_| error())?;
        }
        let row = self.row.get().ok_or_else(error)?;
        if !std::ptr::eq(row.member.function(), member.function()) {
            return Err(error());
        }
        Ok(row)
    }
    #[cfg(test)]
    fn owned_capacity_bytes(&self) -> usize {
        let state = self.state.borrow();
        let mut total = self
            .probes
            .get()
            .map_or(0, crate::hir::renewal_probes::phase::Probes::backing_bytes)
            + state.census.capacity() * std::mem::size_of::<resolved::CensusCursor<'p>>()
            + state.proof.capacity() * std::mem::size_of::<resolved::ProofCursor<'p>>()
            + state.positives.capacity() * std::mem::size_of::<&ResolvedFunction>();
        if let Some(row) = self.row.get() {
            total += row
                .helpers
                .get()
                .map_or(0, HelperBindings::owned_capacity_bytes)
                + row
                    .families
                    .iter()
                    .filter_map(OnceCell::get)
                    .map(Directory::backing_bytes)
                    .sum::<usize>();
        }
        total
    }
}
impl<'f, 'p> Current<'f, 'p> {
    fn valid(&self) -> bool {
        self.ordinal == self.frontier.next.get()
            && self.frontier.ready.get().is_none()
            && self
                .frontier
                .row
                .get()
                .is_some_and(|row| std::ptr::eq(row, self.row))
    }
    /// Consumes this capability; mutable advance cannot run while any other
    /// Current or builder still borrows the phase. No replayable receipt leaks.
    pub(crate) fn finish(self) -> Result<(), Diagnostic> {
        self.complete()?;
        self.frontier.ready.set(Some(self.ordinal));
        Ok(())
    }
    // Adversarial test tokens cannot be minted by a production caller.
    #[cfg(test)]
    pub(crate) fn with_ordinal_for_test(&self, ordinal: usize) -> Current<'_, 'p> {
        Current {
            frontier: self.frontier,
            row: self.row,
            ordinal,
            owner: None,
        }
    }
    pub(crate) fn function(&self) -> &'p ResolvedFunction {
        self.row.member.function()
    }
    pub(crate) fn owner(&self) -> Option<&FunctionExecutionId> {
        self.owner.as_ref()
    }
    pub(crate) fn sites(&self) -> Result<&HelperBindings<'p>, Diagnostic> {
        if !self.valid() {
            return Err(error());
        }
        self.frontier.helpers(self.row)
    }
}

#[cfg(test)]
pub(crate) fn test_bool_target<'a>(
    program: &'a ResolvedProgram,
    target: &'a ResolvedFunction,
    cursor: &mut Vec<resolved::ProofCursor<'a>>,
) -> bool {
    resolved::validation::target(program, target, cursor)
}
