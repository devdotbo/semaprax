//! Allocation frontier outside pure validation. No new quota or attached-plan
//! sizing authority. Active callers fund planner scratch and append through one
//! captured owner; upstream preflight and legacy semantic diagnostic text remain open.
use super::*;
use crate::hir::validation::{constructor_seed::ConstructorSeed, HirValidator};
use crate::hir::{function_directory::Directory, hir_error};
use crate::loan_plan::LoanPlan;
use crate::bounded_output::cfg_reservation::CfgReservation;

#[derive(Clone, Copy)]
enum Payer<'payer> {
    Captured(&'payer CfgReservation),
    ActiveBuilder,
    LegacyUnobserved,
}
impl Payer<'_> {
    fn current() -> Self {
        if crate::bounded_output::active_remaining().is_some() {
            Self::ActiveBuilder
        } else {
            Self::LegacyUnobserved
        }
    }
    fn reserve(self, bytes: usize) -> Result<(), Diagnostic> {
        if let Self::Captured(owner) = self {
            return owner.reserve(bytes).map_err(|_| refused());
        }
        if matches!(self, Self::ActiveBuilder) && !crate::bounded_output::reserve_active_required(bytes) {
            return Err(refused());
        }
        Ok(())
    }
    fn capacity(self, requested: usize, excess: usize) -> Diagnostic {
        if let Self::Captured(owner) = self {
            let _ = owner.capacity(requested, excess);
            return refused();
        }
        if matches!(self, Self::ActiveBuilder) {
            if excess != 0 {
                crate::bounded_output::reserve_active_required(excess);
            }
            crate::bounded_output::refuse_active_required(requested);
        }
        refused()
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io("SPX-H006", String::new())
}

fn backing<T>(items: usize, payer: Payer<'_>) -> Result<usize, Diagnostic> {
    std::alloc::Layout::array::<T>(items)
        .map(|layout| layout.size())
        .map_err(|_| payer.capacity(usize::MAX, 0))
}
fn ensure<T>(values: &mut Vec<T>, needed: usize, payer: Payer<'_>) -> Result<(), Diagnostic> {
    // Even spare-capacity and zero requests preserve the selected first refusal.
    payer.reserve(0)?;
    if needed <= values.capacity() {
        return Ok(());
    }
    let capacity = needed.max(
        values
            .capacity()
            .checked_mul(2)
            .ok_or_else(|| payer.capacity(usize::MAX, 0))?,
    );
    let bytes = backing::<T>(capacity, payer)?;
    payer.reserve(bytes)?;
    if values.try_reserve_exact(capacity - values.len()).is_err() {
        return Err(payer.capacity(bytes, 0));
    }
    if values.capacity() != capacity {
        let excess =
            backing::<T>(values.capacity().saturating_sub(capacity), payer).unwrap_or(usize::MAX);
        return Err(payer.capacity(bytes, excess));
    }
    Ok(())
}
fn count(current: usize, added: usize, payer: Payer<'_>) -> Result<usize, Diagnostic> {
    current
        .checked_add(added)
        .ok_or_else(|| payer.capacity(usize::MAX, 0))
}

pub(in crate::hir) fn validate<'a>(
    program: &'a ResolvedProgram,
    fused: Option<crate::hir::selected_fused_candidate::Selected<'a>>,
    directory: Option<&'a Directory<'a>>,
) -> Result<bool, Diagnostic> {
    validate_with_comparison(program, fused, directory).map(|(selected, _)| selected)
}

pub(in crate::hir) fn validate_with_comparison<'a>(
    program: &'a ResolvedProgram,
    fused: Option<crate::hir::selected_fused_candidate::Selected<'a>>,
    directory: Option<&Directory<'a>>,
) -> Result<(bool, Deferred<'a>), Diagnostic> {
    // Preserve ModuleIndex before the complete constructor preflight, and the
    // preflight before every independent canonical-plan rebuild/allocation.
    let observation = crate::hir::validation::owner_renewal::Observation::default();
    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(program)?;
    let seed = ConstructorSeed::authenticate(program, directory)?;
    let (storage, deferred) = prepare_with_comparison(&seed, program)?;
    let validator = HirValidator::with_storage(seed, &storage)?;
    let selected = crate::hir::validation::owner_renewal::validate_prepared(
        program,
        fused,
        &helpers,
        validator,
        &observation,
    )?;
    Ok((selected, deferred))
}

#[cfg(test)]
pub(in crate::hir) fn prepare<'a>(seed: &ConstructorSeed<'a>) -> Result<Storage<'a>, Diagnostic> {
    prepare_with_comparison(seed, seed.program()).map(|(storage, _)| storage)
}

fn prepare_with_comparison<'a>(
    seed: &ConstructorSeed<'_>,
    program: &'a ResolvedProgram,
) -> Result<(Storage<'a>, Deferred<'a>), Diagnostic> {
    if !std::ptr::eq(seed.program(), program) {
        return Err(hir_error(
            "canonical shared-loan constructor belongs to another program",
        ));
    }
    // Acquisition follows the unchanged ModuleIndex and ConstructorSeed
    // phases. One report/ledger is held across ALL plans and their append; a
    // storage-only test injection keeps its separate legacy payer below.
    let captured = if crate::bounded_output::active_remaining().is_some() {
        Some(CfgReservation::acquire().map_err(|_| refused())?)
    } else {
        None
    };
    let payer = captured.as_ref().map_or_else(Payer::current, Payer::Captured);
    let result = (|| {
        payer.reserve(0)?;
        let mut deferred = Deferred::new(program);
        let mut storage = Storage {
            program,
            ids: Vec::new(),
            liveness: Vec::new(),
            origins: Vec::new(),
        };
        for function in program.functions.iter().chain(program.function_instances.iter().map(|i| &i.function)) {
            let plan = match captured.as_ref() {
                Some(owner) => match crate::loan_plan::build_plan_with_reservation(program, function, owner) {
                    Ok(plan) => plan,
                    Err(crate::loan_plan::FundedError::Physical(_)) => return Err(refused()),
                    Err(crate::loan_plan::FundedError::Semantic(error)) => return Err(error),
                },
                None => crate::loan_plan::build_plan(program, function)?,
            };
            // Compare the COMPLETE plan before append consumes it. An
            // attached plan is never a sizing/admission source.
            deferred.compare(function, &plan);
            append(&mut storage, plan, payer)?;
        }
        Ok((storage, deferred))
    })();
    // Only this exact affine owner can mark physical failure. Never infer
    // payment from H006 text, and never reserve/render after semantic error.
    if let Some(owner) = captured {
        if owner.first().is_some() { return Err(owner.diagnostic()); }
    }
    result
}

fn append(storage: &mut Storage<'_>, plan: LoanPlan, payer: Payer<'_>) -> Result<(), Diagnostic> {
    let base = storage.origins.len();
    let mut local = Vec::new();
    ensure(&mut local, plan.loans.len(), payer)?;
    for (offset, loan) in plan.loans.iter().enumerate() {
        local.push((
            loan.id,
            base.checked_add(offset)
                .ok_or_else(|| hir_error("canonical shared-loan origin directory overflows"))?,
        ));
    }
    local.sort_unstable();
    let origin = |id: &LoanId| {
        // Complete original collect semantics: the final local occurrence wins.
        let end = local.partition_point(|entry| entry.0 <= *id);
        end.checked_sub(1)
            .and_then(|i| local.get(i))
            .filter(|entry| entry.0 == *id)
            .map(|entry| entry.1)
    };
    let mut live_count = 0usize;
    for endpoint in &plan.endpoints {
        let live = match endpoint.point.phase {
            LoanPointPhase::Before => &endpoint.live_before,
            LoanPointPhase::After => &endpoint.live_after,
        };
        live_count = count(live_count, live.len(), payer)?;
    }
    let mut live_probe = Vec::new();
    ensure(&mut live_probe, live_count, payer)?;
    for endpoint in &plan.endpoints {
        let live = match endpoint.point.phase {
            LoanPointPhase::Before => &endpoint.live_before,
            LoanPointPhase::After => &endpoint.live_after,
        };
        for loan in live {
            let source = live_probe.len();
            live_probe.push((
                (
                    endpoint.point.expression.clone(),
                    endpoint.point.phase,
                    *loan,
                ),
                origin(loan),
                source,
            ));
        }
    }
    // This private ordering indexes first source occurrences, not canonical
    // endpoint/loan vectors or error selection. It requires no sort scratch.
    live_probe.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
    let mut source = 0usize;
    for endpoint in &plan.endpoints {
        let live = match endpoint.point.phase {
            LoanPointPhase::Before => &endpoint.live_before,
            LoanPointPhase::After => &endpoint.live_after,
        };
        for loan in live {
            origin(loan)
                .ok_or_else(|| hir_error("canonical shared-loan endpoint names an unknown loan"))?;
            let key = (
                endpoint.point.expression.clone(),
                endpoint.point.phase,
                *loan,
            );
            let first = live_probe.partition_point(|entry| entry.0 < key);
            if storage.live(&key).is_some() || live_probe[first].2 < source {
                return Err(hir_error("canonical shared-loan liveness is duplicated"));
            }
            source += 1;
        }
    }
    let mut id_probe = Vec::new();
    ensure(&mut id_probe, plan.loans.len(), payer)?;
    id_probe.extend(
        plan.loans
            .iter()
            .enumerate()
            .map(|(source, loan)| ((loan.site.clone(), loan.cause.clone()), loan.id, source)),
    );
    id_probe.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
    for (source, loan) in plan.loans.iter().enumerate() {
        let key = (loan.site.clone(), loan.cause.clone());
        let first = id_probe.partition_point(|entry| entry.0 < key);
        if loan.start.phase != LoanPointPhase::Before
            || storage.id(&key).is_some()
            || id_probe[first].2 < source
        {
            return Err(hir_error(
                "canonical shared-loan identity is ambiguous or malformed",
            ));
        }
    }
    // No partial plan directory is published. Growth prepays full replacement
    // capacities while old backing is still live, never a delta or refund.
    let id_needed = count(storage.ids.len(), id_probe.len(), payer)?;
    let live_needed = count(storage.liveness.len(), live_probe.len(), payer)?;
    let origin_needed = count(storage.origins.len(), plan.loans.len(), payer)?;
    ensure(&mut storage.ids, id_needed, payer)?;
    ensure(&mut storage.liveness, live_needed, payer)?;
    ensure(&mut storage.origins, origin_needed, payer)?;
    storage
        .ids
        .extend(id_probe.into_iter().map(|(key, id, _)| (key, id)));
    storage.liveness.extend(
        live_probe
            .into_iter()
            .map(|(key, ordinal, _)| (key, ordinal.expect("checked every source endpoint"))),
    );
    storage
        .origins
        .extend(plan.loans.into_iter().map(|loan| loan.origin));
    storage.ids.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    storage.liveness.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    Ok(())
}

#[cfg(test)]
pub(super) fn capacity_failure(requested: usize, excess: usize) -> Diagnostic {
    Payer::current().capacity(requested, excess)
}

// Raw plan injection exists only for independent receiver/error-priority tests.
// Production acquisition requires the unconstructible current ConstructorSeed.
#[cfg(test)]
pub(super) fn from_plans<'a>(
    program: &'a ResolvedProgram,
    plans: Vec<LoanPlan>,
) -> Result<Storage<'a>, Diagnostic> {
    let mut storage = Storage {
        program,
        ids: Vec::new(),
        liveness: Vec::new(),
        origins: Vec::new(),
    };
    let payer = Payer::current();
    payer.reserve(0)?;
    for plan in plans {
        append(&mut storage, plan, payer)?;
    }
    Ok(storage)
}
