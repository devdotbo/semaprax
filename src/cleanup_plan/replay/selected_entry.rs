//! A private primitive selected entry owns one cumulative reservation.
//! Independent replay reconstructs every fact from the immutable context; no
//! attached schema or certificate is an admission shortcut.
use super::*;

#[cfg(test)]
pub(crate) fn validate_retained_lexical_with_limit(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    bytes: usize,
) -> Result<(), Diagnostic> {
    validate_retained_lexical_with_reservation(program, function, PhysicalReservation::new(bytes))
        .map(|_| ())
}

#[cfg(test)]
pub(crate) fn validate_retained_lexical_with_reservation(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    physical: PhysicalReservation,
) -> Result<PhysicalReservation, Diagnostic> {
    let mut session = Session::new(program, function, physical)?;
    session.validate(function)?;
    session.finish()
}

/// Non-clone preparation owner: authentication and replay consume one ledger.
pub(crate) struct Preparation {
    budget: ReplayBudget,
    failed: bool,
}
impl Preparation {
    pub(crate) fn new(
        function: &ResolvedFunction,
        physical: PhysicalReservation,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            budget: ReplayBudget::from_reservation(function, physical)?,
            failed: false,
        })
    }
    #[cfg(test)]
    pub(crate) fn with_limits(
        function: &ResolvedFunction,
        work: usize,
        bytes: usize,
    ) -> Result<Self, Diagnostic> {
        if work > MAX_REPLAY_WORK_UNITS {
            return Err(replay_error(
                function,
                "test preparation work exceeds production ceiling",
            ));
        }
        let mut preparation = Self::new(function, PhysicalReservation::new(bytes))?;
        preparation.budget.remaining = work;
        Ok(preparation)
    }
    pub(crate) fn charge(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected preparation already failed",
            ));
        }
        let answer = self.budget.charge(function, units, phase);
        self.failed = answer.is_err();
        answer
    }
    pub(crate) fn reserve_array<T>(
        &mut self,
        function: &ResolvedFunction,
        count: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected preparation already failed",
            ));
        }
        let answer = self.budget.reserve_array::<T>(function, count, phase);
        self.failed = answer.is_err();
        answer
    }
    pub(crate) fn reserve_vec<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut Vec<T>,
        additional: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected preparation already failed",
            ));
        }
        let answer = self.budget.reserve_vec(function, values, additional, phase);
        self.failed = answer.is_err();
        answer
    }
    #[cfg(test)]
    pub(crate) fn remaining(&self) -> usize {
        self.budget.remaining
    }
    /// Constructors borrow both resources from this owner. They cannot copy
    /// work credits or replace the ledger when their retained backing grows.
    pub(crate) fn construct<T>(
        &mut self,
        function: &ResolvedFunction,
        apply: impl FnOnce(
            &mut PhysicalReservation,
            &mut dyn FnMut(usize) -> Result<(), Diagnostic>,
        ) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected preparation already failed",
            ));
        }
        let ReplayBudget {
            remaining,
            physical,
            ..
        } = &mut self.budget;
        let physical = physical
            .as_deref_mut()
            .ok_or_else(|| replay_error(function, "selected preparation lost backing ledger"))?;
        let mut debit = |units| {
            *remaining = remaining
                .checked_sub(units)
                .ok_or_else(|| replay_error(function, "selected constructor work exhausted"))?;
            Ok(())
        };
        let answer = apply(physical, &mut debit);
        self.failed = answer.is_err();
        answer
    }
    pub(crate) fn fail(&mut self) {
        self.failed = true;
    }
}

/// One held provider, borrowed callee inventory, and cumulative work ledger.
/// Each selected member still obtains a fresh function-bound renewal lease.
pub(crate) struct Session<'a> {
    indexed: selected_callees::Context<'a>,
    budget: ReplayBudget,
    anchor: &'a ResolvedFunction,
    failed: bool,
}
impl<'a> Session<'a> {
    pub(crate) fn new(
        program: &'a dyn ReplayContext,
        anchor: &'a ResolvedFunction,
        physical: PhysicalReservation,
    ) -> Result<Self, Diagnostic> {
        let budget = ReplayBudget::from_reservation(anchor, physical)?;
        Self::from_budget(program, anchor, budget)
    }

    pub(crate) fn from_preparation(
        program: &'a dyn ReplayContext,
        anchor: &'a ResolvedFunction,
        preparation: Preparation,
    ) -> Result<Self, Diagnostic> {
        if preparation.failed {
            return Err(replay_error(anchor, "selected preparation already failed"));
        }
        Self::from_budget(program, anchor, preparation.budget)
    }

    fn from_budget(
        program: &'a dyn ReplayContext,
        anchor: &'a ResolvedFunction,
        mut budget: ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        budget.reserve_array::<Self>(anchor, 1, "selected module replay session")?;
        let indexed = selected_callees::Context::new(program, anchor, &mut budget)?;
        Ok(Self {
            indexed,
            budget,
            anchor,
            failed: false,
        })
    }

    pub(crate) fn validate(&mut self, function: &ResolvedFunction) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected module replay already failed",
            ));
        }
        // Keep refusal sticky without publishing a diagnostic from Drop or
        // allowing another member to restart the spent work allowance.
        self.failed = true;
        require_entry(function)?;
        let context = selected_context::Context::new(&self.indexed, function, &mut self.budget)?;
        lexical_replay::validate_with_index(&context, function, &mut self.budget, None)?;
        self.failed = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn metrics(&self) -> (usize, usize) {
        (
            self.budget.remaining,
            self.budget
                .physical
                .as_ref()
                .expect("selected backing retained")
                .charged(),
        )
    }
    pub(crate) fn validate_scalar(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected module replay already failed",
            ));
        }
        self.failed = true;
        let context = selected_context::Context::new(&self.indexed, function, &mut self.budget)?;
        scalar_replay::validate(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }
    pub(crate) fn finish(mut self) -> Result<PhysicalReservation, Diagnostic> {
        if self.failed {
            return Err(replay_error(
                self.anchor,
                "selected module replay already failed",
            ));
        }
        // The function lease has dropped; the shared inventory drops with this
        // session. Returning the debit grants neither a work reset nor a refund.
        self.budget
            .physical
            .take()
            .map(|physical| *physical)
            .ok_or_else(|| {
                replay_error(self.anchor, "selected replay lost its physical reservation")
            })
    }
}

#[cfg(test)]
mod tests;

fn require_entry(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if !crate::hir::lexical_exit::requires(function)?
        || !matches!(
            function.cleanup_plan.schema,
            super::super::CLEANUP_PLAN_SCHEMA_V18 | super::super::CLEANUP_PLAN_SCHEMA_V19
        )
    {
        return Err(replay_error(
            function,
            "selected retained lexical replay requires an actual lexical function",
        ));
    }
    Ok(())
}
/// Whole-entry unit oracle uses the same constructors and proof owner as the
/// selected production entry, with independently supplied exact work/bytes.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn validate_with_limits(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
) -> Result<(usize, usize), Diagnostic> {
    require_entry(function)?;
    let mut budget = ReplayBudget::with_physical(function, bytes)?;
    budget.remaining = work;
    let mut session = Session::from_budget(program, function, budget)?;
    session.validate(function)?;
    let spent = work
        .checked_sub(session.budget.remaining)
        .expect("work never increases");
    let charged = session
        .budget
        .physical
        .as_ref()
        .expect("selected reservation retained")
        .charged();
    // Drop the whole budget and all constructors inside the caller's observer.
    drop(session);
    Ok((spent, charged))
}

#[cfg(test)]
mod borrowed;
