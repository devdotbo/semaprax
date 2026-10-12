//! Independent canonical replay retains each rebuilt origin once. Endpoint
//! ordinals are private access data, never source-plan or validation authority.
//! Caller-owned validation storage must fund the directory; this pure receiver
//! neither creates a physical ledger nor changes the conservative forecast.
use super::*;

#[derive(Clone)]
pub(super) struct Inventory {
    pub(super) ids: BTreeMap<(ExpressionId, LoanCause), LoanId>,
    pub(super) liveness: BTreeMap<(ExpressionId, LoanPointPhase, LoanId), usize>,
    pub(super) origins: Vec<Place>,
}

impl Inventory {
    pub(super) fn rebuild(program: &ResolvedProgram) -> Result<Self, Diagnostic> {
        let mut inventory = Self {
            ids: BTreeMap::new(),
            liveness: BTreeMap::new(),
            origins: Vec::new(),
        };
        for function in program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ) {
            // Reconstruct independently from the exact current body. Attached
            // plans remain checked by the ordinary validator, not trusted here.
            inventory.append(crate::loan_plan::build_plan(program, function)?)?;
        }
        Ok(inventory)
    }

    fn append(&mut self, plan: crate::loan_plan::LoanPlan) -> Result<(), Diagnostic> {
        let base = self.origins.len();
        let origins = plan
            .loans
            .iter()
            .enumerate()
            .map(|(offset, loan)| {
                base.checked_add(offset)
                    .map(|ordinal| (loan.id, ordinal))
                    .ok_or_else(|| hir_error("canonical shared-loan origin directory overflows"))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        for endpoint in &plan.endpoints {
            let live = match endpoint.point.phase {
                LoanPointPhase::Before => &endpoint.live_before,
                LoanPointPhase::After => &endpoint.live_after,
            };
            for loan in live {
                let ordinal = origins.get(loan).copied().ok_or_else(|| {
                    hir_error("canonical shared-loan endpoint names an unknown loan")
                })?;
                if self
                    .liveness
                    .insert(
                        (
                            endpoint.point.expression.clone(),
                            endpoint.point.phase,
                            *loan,
                        ),
                        ordinal,
                    )
                    .is_some()
                {
                    return Err(hir_error("canonical shared-loan liveness is duplicated"));
                }
            }
        }
        for loan in plan.loans {
            if loan.start.phase != LoanPointPhase::Before
                || self.ids.insert((loan.site, loan.cause), loan.id).is_some()
            {
                return Err(hir_error(
                    "canonical shared-loan identity is ambiguous or malformed",
                ));
            }
            // Move the exact independently rebuilt Place, including every
            // projection and text holder; no endpoint clones its payload.
            self.origins.push(loan.origin);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

// The ordinary compatibility constructor retains the saved owned directories.
// Production caller storage lends only immutable exact-subject lookup metadata.
#[derive(Clone)]
// The compatibility branch preserves its existing inline owned inventory;
// boxing it would add an unpriced allocation to old constructors.
#[allow(clippy::large_enum_variant)]
pub(super) enum Lookup<'a> {
    Legacy(Inventory),
    Borrowed(&'a super::super::canonical_loan_storage::Storage<'a>),
}
impl<'a> Lookup<'a> {
    pub(super) fn id(&self, key: &(ExpressionId, LoanCause)) -> Option<&LoanId> {
        match self {
            Self::Legacy(i) => i.ids.get(key),
            Self::Borrowed(i) => i.id(key),
        }
    }
    pub(super) fn live(&self, key: &(ExpressionId, LoanPointPhase, LoanId)) -> Option<&usize> {
        match self {
            Self::Legacy(i) => i.liveness.get(key),
            Self::Borrowed(i) => i.live(key),
        }
    }
    pub(super) fn origin(&self, index: usize) -> Option<&Place> {
        match self {
            Self::Legacy(i) => i.origins.get(index),
            Self::Borrowed(i) => i.origin(index),
        }
    }
    pub(super) fn shared(&self) -> Option<&'a super::super::canonical_loan_storage::Storage<'a>> {
        match self {
            Self::Legacy(_) => None,
            Self::Borrowed(i) => Some(*i),
        }
    }
}
