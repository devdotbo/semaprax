//! Test-only explicit invocation authority for the oracle payer. Two RAII
//! tokens, both thread-local and neither Clone nor Send:
//!
//! * `OptIn` is held by exactly the opted controls next to their account scope;
//!   it only names the lease the live hook may use. Without it every hook and
//!   constructor stays ordinary.
//! * `Invocation` binds that lease to the exact LIVE program and function
//!   addresses of one comparison. It owns no clone of either. A second entry,
//!   an expired lease, or a different subject fails closed; nothing falls back
//!   to the production ACTIVE ledger.
use super::{OracleLease, OracleRefusal};
use std::cell::Cell;
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum InvocationRefusal {
    /// An invocation or opt-in is already active on this thread.
    Nested,
    /// The lease no longer names a live account scope.
    Expired(OracleRefusal),
    /// An invocation exists but names another program or function.
    StaleSubject,
    /// A token was released out of order or twice.
    Unbalanced,
}

#[derive(Clone, Copy)]
struct Subject {
    serial: u64,
    /// Addresses only; compared for identity, never dereferenced.
    program: usize,
    function: usize,
    lease: OracleLease,
}

thread_local! {
    static OPTED: Cell<Option<(u64, OracleLease)>> = const { Cell::new(None) };
    static CURRENT: Cell<Option<Subject>> = const { Cell::new(None) };
    static SERIAL: Cell<u64> = const { Cell::new(0) };
}

fn next_serial() -> Result<u64, InvocationRefusal> {
    SERIAL.with(|serial| {
        let next = serial
            .get()
            .checked_add(1)
            .ok_or(InvocationRefusal::Nested)?;
        serial.set(next);
        Ok(next)
    })
}

fn live(lease: OracleLease) -> Result<(), InvocationRefusal> {
    lease
        .receipt()
        .map(|_| ())
        .map_err(InvocationRefusal::Expired)
}

#[must_use]
pub(in crate::cleanup_plan::replay) struct OptIn {
    serial: u64,
    _thread: PhantomData<*const ()>,
}

/// Opt the current thread's live hook into `lease`. Nested opt-ins refuse.
pub(in crate::cleanup_plan::replay) fn opt_in(
    lease: OracleLease,
) -> Result<OptIn, InvocationRefusal> {
    live(lease)?;
    if OPTED.with(|opted| opted.get().is_some()) {
        return Err(InvocationRefusal::Nested);
    }
    let serial = next_serial()?;
    OPTED.with(|opted| opted.set(Some((serial, lease))));
    Ok(OptIn {
        serial,
        _thread: PhantomData,
    })
}

impl Drop for OptIn {
    fn drop(&mut self) {
        OPTED.with(|opted| {
            if opted.get().is_some_and(|(serial, _)| serial == self.serial) {
                opted.set(None);
            }
        });
    }
}

/// The opted lease, if any. A lease whose scope ended is refused, not skipped.
pub(in crate::cleanup_plan::replay) fn opted() -> Result<Option<OracleLease>, InvocationRefusal> {
    match OPTED.with(Cell::get) {
        None => Ok(None),
        Some((_, lease)) => live(lease).map(|()| Some(lease)),
    }
}

#[must_use]
pub(in crate::cleanup_plan::replay) struct Invocation<'subject> {
    serial: u64,
    _thread: PhantomData<*const ()>,
    _subject: PhantomData<(
        &'subject dyn crate::cleanup::ReplayContext,
        &'subject crate::hir::ResolvedFunction,
    )>,
}

/// Bind `lease` to this exact live subject. The caller holds the references for
/// the whole guard lifetime; the token stores only their identities.
pub(in crate::cleanup_plan::replay) fn enter<'subject>(
    program: &'subject dyn crate::cleanup::ReplayContext,
    function: &'subject crate::hir::ResolvedFunction,
    lease: OracleLease,
) -> Result<Invocation<'subject>, InvocationRefusal> {
    live(lease)?;
    if CURRENT.with(|current| current.get().is_some()) {
        return Err(InvocationRefusal::Nested);
    }
    let serial = next_serial()?;
    CURRENT.with(|current| {
        current.set(Some(Subject {
            serial,
            program: std::ptr::from_ref(program).cast::<()>() as usize,
            function: std::ptr::from_ref(function) as usize,
            lease,
        }))
    });
    Ok(Invocation {
        serial,
        _thread: PhantomData,
        _subject: PhantomData,
    })
}

impl Invocation<'_> {
    /// Explicit release; an out-of-order release leaves the other token alone.
    pub(in crate::cleanup_plan::replay) fn exit(self) -> Result<(), InvocationRefusal> {
        let held = CURRENT
            .with(Cell::get)
            .is_some_and(|s| s.serial == self.serial);
        drop(self);
        if held {
            Ok(())
        } else {
            Err(InvocationRefusal::Unbalanced)
        }
    }
}

impl Drop for Invocation<'_> {
    /// Runs during unwind too, restoring "no invocation".
    fn drop(&mut self) {
        CURRENT.with(|current| {
            if current.get().is_some_and(|s| s.serial == self.serial) {
                current.set(None);
            }
        });
    }
}

/// Authority for the diagnostic bridge, which sees only the function. The held
/// invocation must name exactly this live function address (the program was
/// matched at `enter`); another function, or an expired lease, is refused and
/// never falls back to the ACTIVE ledger. `Ok(None)`: no invocation.
pub(in crate::cleanup_plan::replay) fn authority_for_function(
    function: &crate::hir::ResolvedFunction,
) -> Result<Option<OracleLease>, InvocationRefusal> {
    let Some(subject) = CURRENT.with(Cell::get) else {
        return Ok(None);
    };
    live(subject.lease)?;
    if subject.function != std::ptr::from_ref(function) as usize {
        return Err(InvocationRefusal::StaleSubject);
    }
    Ok(Some(subject.lease))
}

/// Authority for exactly this program/function. `Ok(None)`: no invocation, the
/// caller is ordinary. An invocation naming any other subject, including a
/// forwarding wrapper around the same data, is `StaleSubject`.
pub(in crate::cleanup_plan::replay) fn authority(
    program: &dyn crate::cleanup::ReplayContext,
    function: &crate::hir::ResolvedFunction,
) -> Result<Option<OracleLease>, InvocationRefusal> {
    let Some(subject) = CURRENT.with(Cell::get) else {
        return Ok(None);
    };
    live(subject.lease)?;
    if subject.program != std::ptr::from_ref(program).cast::<()>() as usize
        || subject.function != std::ptr::from_ref(function) as usize
    {
        return Err(InvocationRefusal::StaleSubject);
    }
    Ok(Some(subject.lease))
}
