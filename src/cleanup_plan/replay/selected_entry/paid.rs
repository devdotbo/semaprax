//! Caller-owned paid outward report for one selected replay ledger.
//!
//! The actual caller acquires this affine carrier on the same ledger before
//! preparation: one Diagnostic slot and the exact bytes of one fixed outward
//! form are debited before either is allocated. Setup, authentication,
//! constructor and Session refusals transfer that backing; publication
//! allocates nothing. Steps on the qualified route refuse with the Copy
//! `Refused`, never an owned Diagnostic or Vec. A refused carrier returns the
//! exact failed ledger and a Copy reason. Only `Unpaid::into_unfunded`, a
//! named boundary outside the qualified contract, allocates an outward report.
use super::*;
use physical_budget::{SetupFailure, SetupReason};

/// One fixed outward failure form chosen by the actual caller. Its literal
/// message bytes are the exact text payment; no function id or formatted
/// detail is published through a paid carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReportForm {
    code: &'static str,
    message: &'static str,
}
impl ReportForm {
    pub(crate) const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
    #[cfg(test)]
    pub(crate) fn code(self) -> &'static str {
        self.code
    }
    #[cfg(test)]
    pub(crate) fn message(self) -> &'static str {
        self.message
    }
    /// Exact carrier request: the Diagnostic slot plus the literal text.
    #[cfg(test)]
    pub(crate) fn paid_bytes(self) -> usize {
        std::mem::size_of::<Diagnostic>() + self.message.len()
    }
    /// Compatibility boundary only. This allocates an unfunded owned report
    /// and is never a paid or zero-allocation route.
    pub(crate) fn legacy(self) -> Vec<Diagnostic> {
        vec![Diagnostic::io(self.code, self.message)]
    }
}

/// Why a qualified-route step refused. Copy; it owns no heap backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Cause {
    /// Current source, namespace, signature or closure disagreement.
    Mismatch,
    /// A work or physical debit refused. Its first paid budget Diagnostic
    /// (the prepaid refusal text, moved once) stays in the failed Preparation.
    Budget,
    /// A declaration constructor refused; `detail` is its fixed literal.
    Constructor,
    /// The held preparation had already failed or lost its ledger.
    Failed,
    /// Initial preparation setup refused; `detail` is the `SetupReason` text.
    Setup,
    /// Session construction or member replay refused. Its own Diagnostic is
    /// moved into the failed Preparation as the first receipt.
    Session,
}

/// Inline qualified-route refusal. It is Copy and heap-free, and has no
/// implicit conversion to a Diagnostic or Vec. The failed ledger and any
/// first paid receipt stay with their owners; tests observe them there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "cause and detail are read by owning observations only"
    )
)]
pub(crate) struct Refused {
    cause: Cause,
    detail: &'static str,
    stage: Option<&'static str>,
}
impl Refused {
    pub(crate) const MISMATCH: Self =
        Self::new(Cause::Mismatch, "retained source authentication disagrees");
    pub(crate) const FAILED: Self = Self::new(Cause::Failed, "selected preparation already failed");
    const LEDGER: Self = Self::new(Cause::Failed, "selected preparation lost backing ledger");
    const SESSION: Self = Self::new(Cause::Session, "selected module replay refused");
    const fn new(cause: Cause, detail: &'static str) -> Self {
        Self {
            cause,
            detail,
            stage: None,
        }
    }
    pub(crate) const fn constructor(detail: &'static str) -> Self {
        Self::new(Cause::Constructor, detail)
    }
    const fn budget(phase: &'static str) -> Self {
        Self::new(Cause::Budget, phase)
    }
    /// Name the first refusing boundary. An earlier, more specific stage is
    /// never replaced; recording it copies one static pointer.
    pub(crate) fn at(mut self, stage: &'static str) -> Self {
        if self.stage.is_none() {
            self.stage = Some(stage);
        }
        self
    }
    #[cfg(test)]
    pub(crate) fn cause(self) -> Cause {
        self.cause
    }
    #[cfg(test)]
    pub(crate) fn detail(self) -> &'static str {
        self.detail
    }
    #[cfg(test)]
    pub(crate) fn stage(self) -> Option<&'static str> {
        self.stage
    }
    /// Test-only legacy rendering for owners that still return a Diagnostic.
    /// It allocates an unfunded value and is unreachable from production.
    #[cfg(test)]
    pub(crate) fn into_legacy_diagnostic(self) -> Diagnostic {
        Diagnostic::io("SPX-H006", self.detail)
    }
}

impl Preparation {
    /// Retain the first paid refusal Diagnostic by move; a later one is
    /// dropped (a spent selected budget returns only an empty, unallocated
    /// String). The owner is failed and the inline reason is returned.
    fn record(&mut self, error: Diagnostic, refused: Refused) -> Refused {
        self.failed = true;
        if self.receipt.is_none() {
            self.receipt = Some(error);
        }
        refused
    }

    pub(crate) fn try_charge(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &'static str,
    ) -> Result<(), Refused> {
        if self.failed {
            return Err(Refused::FAILED);
        }
        match self.budget.charge(function, units, phase) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.record(error, Refused::budget(phase))),
        }
    }

    pub(crate) fn try_reserve_array<T>(
        &mut self,
        function: &ResolvedFunction,
        count: usize,
        phase: &'static str,
    ) -> Result<(), Refused> {
        if self.failed {
            return Err(Refused::FAILED);
        }
        match self.budget.reserve_array::<T>(function, count, phase) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.record(error, Refused::budget(phase))),
        }
    }

    pub(crate) fn try_reserve_vec<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut Vec<T>,
        additional: usize,
        phase: &'static str,
    ) -> Result<(), Refused> {
        if self.failed {
            return Err(Refused::FAILED);
        }
        match self.budget.reserve_vec(function, values, additional, phase) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.record(error, Refused::budget(phase))),
        }
    }

    /// Constructors borrow both resources from this owner. They cannot copy
    /// work credits or replace the ledger when their retained backing grows.
    /// Work exhaustion spends the budget's prepaid refusal text as the first
    /// receipt; a constructor's own refusal is its Copy literal. A value
    /// returned after an ignored exhausted debit is dropped (fail closed).
    pub(crate) fn construct<T>(
        &mut self,
        function: &ResolvedFunction,
        apply: impl FnOnce(
            &mut PhysicalReservation,
            &mut dyn FnMut(usize) -> Result<(), Refused>,
        ) -> Result<T, Refused>,
    ) -> Result<T, Refused> {
        const PHASE: &str = "selected constructor work";
        if self.failed {
            return Err(Refused::FAILED);
        }
        let mut exhausted = false;
        let answer = {
            let ReplayBudget {
                remaining,
                physical,
                ..
            } = &mut self.budget;
            match physical.as_deref_mut() {
                None => None,
                Some(physical) => {
                    let mut debit = |units: usize| match remaining.checked_sub(units) {
                        Some(left) => {
                            *remaining = left;
                            Ok(())
                        }
                        None => {
                            exhausted = true;
                            Err(Refused::budget(PHASE))
                        }
                    };
                    Some(apply(physical, &mut debit))
                }
            }
        };
        match answer {
            None => {
                self.failed = true;
                Err(Refused::LEDGER)
            }
            Some(answer) if exhausted => {
                drop(answer);
                let error = self.budget.budget_refusal(
                    function,
                    "cleanup replay work budget exhausted during ",
                    PHASE,
                );
                Err(self.record(error, Refused::budget(PHASE)))
            }
            Some(Ok(value)) => Ok(value),
            Some(Err(refused)) => {
                self.failed = true;
                Err(refused)
            }
        }
    }

    /// The first retained paid refusal, for owning observations only.
    #[cfg(test)]
    pub(crate) fn receipt(&self) -> Option<&Diagnostic> {
        self.receipt.as_ref()
    }
}

/// Affine paid carrier. Its backing is already charged on the selected
/// ledger; dropping it on success frees that backing without a refund.
pub(crate) struct PaidReport {
    form: ReportForm,
    diagnostics: Vec<Diagnostic>,
    message: String,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "each owner is held for its Drop and owning observations"
    )
)]
enum Ledger {
    /// Initial preparation setup refused after the carrier was paid.
    Setup(SetupFailure),
    /// Refused before a preparation existed; the ledger is rejected.
    Unprepared(PhysicalReservation),
    /// Authentication, constructor or Session refusal; preparation failed.
    Prepared(Preparation),
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the ledger and inline reason travel with the report"
    )
)]
enum Refusal {
    Unpaid {
        failure: SetupFailure,
        form: ReportForm,
    },
    Paid {
        report: Vec<Diagnostic>,
        form: ReportForm,
        ledger: Ledger,
        refused: Refused,
    },
}

/// Sealed selected replay refusal. Only this module constructs it, so no
/// caller can forge a paid report. Neither variant can restore admission:
/// every retained ledger is rejected and every retained preparation failed.
pub(crate) struct PaidRefusal(Refusal);

/// Typed unpaid outward boundary. No paid carrier exists for the requested
/// form: the carrier was refused, was paid for a different form, or the view
/// refusal was not a selected replay. It holds only Copy facts and cannot
/// claim funding. Only `into_unfunded` allocates, outside the qualified route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the unpaid reason is read by owning observations only"
    )
)]
pub(crate) struct Unpaid {
    form: ReportForm,
    reason: UnpaidReason,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the refused setup reason is read by owning observations only"
    )
)]
pub(crate) enum UnpaidReason {
    /// The carrier debit or allocation was refused on the selected ledger.
    Carrier(SetupReason),
    /// A carrier paid for another form; it is dropped, never relabelled.
    FormMismatch,
    /// A non-replay refusal already owned its legacy Vec.
    Legacy,
}
impl Unpaid {
    pub(crate) const fn legacy(form: ReportForm) -> Self {
        Self {
            form,
            reason: UnpaidReason::Legacy,
        }
    }
    #[cfg(test)]
    pub(crate) fn reason(self) -> UnpaidReason {
        self.reason
    }
    /// Named compatibility boundary: allocates the form's literal, unfunded.
    pub(crate) fn into_unfunded(self) -> Vec<Diagnostic> {
        self.form.legacy()
    }
}

impl PaidReport {
    /// Debit the exact Diagnostic slot and form text, then allocate both.
    /// Refusal returns the same rejected ledger and an inline Copy reason.
    pub(crate) fn try_acquire(
        mut physical: PhysicalReservation,
        form: ReportForm,
    ) -> Result<(Self, PhysicalReservation), PaidRefusal> {
        let unpaid = |physical, reason| {
            PaidRefusal(Refusal::Unpaid {
                failure: SetupFailure::refused(physical, reason),
                form,
            })
        };
        if physical.reserve_array::<Diagnostic>(1).is_err() {
            return Err(unpaid(physical, SetupReason::ReportCarrier));
        }
        if physical.reserve(form.message.len()).is_err() {
            return Err(unpaid(physical, SetupReason::ReportText));
        }
        let diagnostics = Vec::<Diagnostic>::with_capacity(1);
        let message = String::with_capacity(form.message.len());
        if diagnostics.capacity() != 1 || message.capacity() != form.message.len() {
            drop((diagnostics, message));
            return Err(unpaid(physical, SetupReason::ReportCapacity));
        }
        Ok((
            Self {
                form,
                diagnostics,
                message,
            },
            physical,
        ))
    }

    /// Acquired Diagnostic slot and text backing, for pointer controls only.
    #[cfg(test)]
    pub(crate) fn backing(&self) -> (*const Diagnostic, *const u8) {
        (self.diagnostics.as_ptr(), self.message.as_ptr())
    }

    /// Exact-capacity writes into the acquired backing; no reallocation.
    fn publish(self, ledger: Ledger, refused: Refused) -> PaidRefusal {
        let Self {
            form,
            mut diagnostics,
            mut message,
        } = self;
        message.push_str(form.message);
        diagnostics.push(Diagnostic::io(form.code, message));
        PaidRefusal(Refusal::Paid {
            report: diagnostics,
            form,
            ledger,
            refused,
        })
    }

    /// Run the unchanged neutral setup on the ledger that paid this carrier.
    pub(crate) fn prepare(
        self,
        function: &ResolvedFunction,
        physical: PhysicalReservation,
    ) -> Result<(Preparation, Self), PaidRefusal> {
        match Preparation::try_new(function, physical) {
            Ok(preparation) => Ok((preparation, self)),
            Err(failure) => {
                let refused = Refused::new(Cause::Setup, failure.reason_message());
                Err(self.publish(Ledger::Setup(failure), refused))
            }
        }
    }

    pub(crate) fn refuse_unprepared(self, mut physical: PhysicalReservation) -> PaidRefusal {
        physical.reject();
        self.publish(Ledger::Unprepared(physical), Refused::MISMATCH)
    }

    /// Transfer on an inline authentication, budget or constructor refusal.
    /// The held preparation is failed and its ledger rejected; the first
    /// receipt (ledger and any retained paid Diagnostic) stays.
    pub(crate) fn refuse(self, mut preparation: Preparation, refused: Refused) -> PaidRefusal {
        preparation.failed = true;
        if let Some(physical) = preparation.budget.physical.as_deref_mut() {
            physical.reject();
        }
        self.publish(Ledger::Prepared(preparation), refused)
    }

    /// Transfer on a Session construction or member refusal. The Session's
    /// own Diagnostic is moved into the failed owner as its first receipt
    /// unless an earlier one is held; it is never cloned, formatted or
    /// published. The outward report stays this carrier's fixed literal.
    pub(crate) fn refuse_session(
        self,
        mut preparation: Preparation,
        error: Diagnostic,
    ) -> PaidRefusal {
        let refused = preparation.record(error, Refused::SESSION);
        self.refuse(preparation, refused)
    }
}

impl Session<'_> {
    /// Recover the cumulative owner after a Session refusal. The result is
    /// failed: it can be observed and dropped but never replayed again.
    pub(crate) fn into_failed_preparation(self) -> Preparation {
        Preparation {
            budget: self.budget,
            failed: true,
            receipt: None,
        }
    }

    /// Paid twin of `finish`. Success drops the unused carrier and returns
    /// the same boxed ledger, with no refund or reset; refusal retains the
    /// exact ledger owner and never allocates `finish`'s legacy Diagnostic.
    pub(crate) fn finish_paid(
        mut self,
        report: PaidReport,
    ) -> Result<Box<PhysicalReservation>, PaidRefusal> {
        if self.failed {
            return Err(report.refuse(self.into_failed_preparation(), Refused::FAILED));
        }
        match self.budget.physical.take() {
            Some(physical) => {
                drop(report);
                Ok(physical)
            }
            None => Err(report.refuse(self.into_failed_preparation(), Refused::LEDGER)),
        }
    }
}

impl PaidRefusal {
    #[cfg(test)]
    pub(crate) fn is_paid(&self) -> bool {
        matches!(self.0, Refusal::Paid { .. })
    }

    /// A paid carrier for exactly `expected` moves out without allocation.
    /// Anything else fails closed to the typed unpaid boundary: the ledger,
    /// and any carrier paid for a different form, are dropped here.
    pub(crate) fn into_report(self, expected: ReportForm) -> Result<Vec<Diagnostic>, Unpaid> {
        match self.0 {
            Refusal::Paid { report, form, .. } if form == expected => Ok(report),
            Refusal::Paid { .. } => Err(Unpaid {
                form: expected,
                reason: UnpaidReason::FormMismatch,
            }),
            Refusal::Unpaid { failure, .. } => {
                let (physical, reason) = failure.into_parts();
                drop(physical);
                Err(Unpaid {
                    form: expected,
                    reason: UnpaidReason::Carrier(reason),
                })
            }
        }
    }

    /// Pass-through form: the carrier's own form.
    pub(crate) fn into_published(self) -> Result<Vec<Diagnostic>, Unpaid> {
        let form = match &self.0 {
            Refusal::Paid { form, .. } | Refusal::Unpaid { form, .. } => *form,
        };
        self.into_report(form)
    }

    #[cfg(test)]
    pub(crate) fn report(&self) -> Option<&Vec<Diagnostic>> {
        match &self.0 {
            Refusal::Paid { report, .. } => Some(report),
            Refusal::Unpaid { .. } => None,
        }
    }

    /// The inline reason transferred with a paid carrier.
    #[cfg(test)]
    pub(crate) fn refused(&self) -> Option<Refused> {
        match &self.0 {
            Refusal::Paid { refused, .. } => Some(*refused),
            Refusal::Unpaid { .. } => None,
        }
    }

    /// The first paid refusal Diagnostic retained with the failed owner.
    #[cfg(test)]
    pub(crate) fn receipt(&self) -> Option<&Diagnostic> {
        match &self.0 {
            Refusal::Paid {
                ledger: Ledger::Prepared(preparation),
                ..
            } => preparation.receipt.as_ref(),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn setup_reason(&self) -> Option<SetupReason> {
        match &self.0 {
            Refusal::Unpaid { failure, .. }
            | Refusal::Paid {
                ledger: Ledger::Setup(failure),
                ..
            } => Some(failure.reason()),
            Refusal::Paid { .. } => None,
        }
    }

    /// The exact retained ledger, for owning observations only.
    #[cfg(test)]
    pub(crate) fn ledger_mut(&mut self) -> Option<&mut PhysicalReservation> {
        match &mut self.0 {
            Refusal::Unpaid { failure, .. }
            | Refusal::Paid {
                ledger: Ledger::Setup(failure),
                ..
            } => Some(failure.ledger_mut()),
            Refusal::Paid {
                ledger: Ledger::Unprepared(physical),
                ..
            } => Some(physical),
            Refusal::Paid {
                ledger: Ledger::Prepared(preparation),
                ..
            } => preparation.budget.physical.as_deref_mut(),
        }
    }

    /// The retained failed preparation, for no-restoration controls only.
    #[cfg(test)]
    pub(crate) fn into_preparation(self) -> Option<Preparation> {
        match self.0 {
            Refusal::Paid {
                ledger: Ledger::Prepared(preparation),
                ..
            } => Some(preparation),
            _ => None,
        }
    }
}
