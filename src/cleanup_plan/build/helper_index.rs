//! Immutable construction-phase helper authority. Canonical plans are staged
//! together, then published only after the borrowed index has been dropped.
use super::*;
use crate::hir::ResolvedBinding;
use crate::loop_calls::byte_writer::{HelperBindings, ModuleIndex};

#[derive(Clone)]
pub(super) enum HelperSites<'a> {
    Owned(HelperBindings<'a>),
    Indexed {
        sites: &'a HelperBindings<'a>,
        #[cfg(test)]
        index: &'a ModuleIndex<'a>,
    },
}
impl<'a> HelperSites<'a> {
    fn is_empty(&self) -> bool {
        match self {
            Self::Owned(sites) => sites.is_empty(),
            Self::Indexed { sites, .. } => sites.is_empty(),
        }
    }
    fn get(&self, at: &ExpressionId) -> Option<&'a ResolvedBinding> {
        match self {
            Self::Owned(sites) => sites.get(at),
            Self::Indexed { sites, .. } => sites.get(at),
        }
    }
    #[cfg(test)]
    pub(super) fn owned_capacity_bytes(&self) -> usize {
        match self {
            Self::Owned(sites) => sites.owned_capacity_bytes(),
            Self::Indexed { index, .. } => index.owned_capacity_bytes(),
        }
    }
}
impl<'a> PlanBuilder<'a> {
    pub(super) fn new(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
    ) -> Result<Self, Diagnostic> {
        let sites = crate::loop_calls::byte_writer::checked_bindings(program, function)?;
        Self::with_helper_sites(program, function, HelperSites::Owned(sites))
    }
    pub(super) fn with_index(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        index: &'a ModuleIndex<'a>,
    ) -> Result<Self, Diagnostic> {
        if !index.matches_program(program) {
            return Err(plan_error(
                "construction helper index belongs to another typed program",
            ));
        }
        let sites = index.function_sites(function).ok_or_else(|| {
            plan_error("construction helper index has no exact function authority")
        })?;
        Self::with_helper_sites(
            program,
            function,
            HelperSites::Indexed {
                sites,
                #[cfg(test)]
                index,
            },
        )
    }
    pub(super) fn renewal_binding(&self, at: &ExpressionId) -> Option<&'a ResolvedBinding> {
        // Canonical transition anchors own copies of source identities. Membership
        // is derived once from the exact held function, preserving legacy key and
        // profile priority, never from attached cleanup transitions.
        self.byte_helpers
            .get(at)
            .or_else(|| crate::string_ops::replacement::binding(self.function, at))
            .or_else(|| crate::byte_ops::same_owner_set_binding(self.function, at))
            .or_else(|| crate::hir::vec_loop_renewal::binding_in(self.program, self.function, at))
            .or_else(|| crate::hir::iterator_loop::renewal_binding(self.program, self.function, at))
    }
    fn with_helper_sites(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        byte_helpers: helper_index::HelperSites<'a>,
    ) -> Result<Self, Diagnostic> {
        let has_byte_helpers = !byte_helpers.is_empty();
        let mut storage_to_slot = BTreeMap::new();
        let mut inventory_storage = BTreeMap::new();
        let mut slots = Vec::with_capacity(function.cleanup.slots.len());
        for inventory_slot in &function.cleanup.slots {
            let storage = match &inventory_slot.origin {
                crate::cleanup::CleanupStorageOrigin::Parameter { value, .. }
                | crate::cleanup::CleanupStorageOrigin::Binding { value }
                | crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value } => {
                    if matches!(
                        &inventory_slot.origin,
                        crate::cleanup::CleanupStorageOrigin::ProvisionalResult { .. }
                    ) {
                        StorageId::ProvisionalResult
                    } else {
                        StorageId::Value(value.clone())
                    }
                }
                crate::cleanup::CleanupStorageOrigin::Temporary { expression } => {
                    StorageId::Temporary(expression.clone())
                }
            };
            let id = CleanupSlotId(inventory_slot.id.0);
            if storage_to_slot.insert(storage.clone(), id).is_some()
                || inventory_storage
                    .insert(inventory_slot.id, storage.clone())
                    .is_some()
            {
                return Err(plan_error("cleanup inventory has conflicting storage"));
            }
            slots.push(CleanupSlot {
                id,
                storage,
                ty: inventory_slot.ty.clone(),
                storage_index: inventory_slot.discovery_index,
                field_liveness_shape: inventory_slot.shape.clone(),
            });
        }
        let mut leaves = BTreeMap::new();
        for flag in &function.cleanup.flags {
            let storage = inventory_storage
                .get(&flag.place.storage)
                .cloned()
                .ok_or_else(|| plan_error("cleanup flag references unknown inventory storage"))?;
            if leaves
                .insert(
                    flag.id,
                    LeafMetadata {
                        place: CleanupPlace {
                            storage,
                            projections: flag.place.projections.clone(),
                        },
                        lifecycle: flag.lifecycle.clone(),
                    },
                )
                .is_some()
            {
                return Err(plan_error("cleanup inventory repeats a liveness flag"));
            }
        }
        let next_flag = u32::try_from(leaves.len())
            .map_err(|_| plan_error("too many cleanup liveness flags"))?;
        let root = CleanupRegionId(0);
        let entry = BlockId(0);
        let mut builder = Self {
            string_appends: crate::stdin_stream_ops::owner_reopens(function),
            string_owner_moves: crate::literal_format::moving_string_operands(function),
            string_condition_reads: crate::string_ops::conditions::function_reads(function),
            program,
            function,
            byte_helpers,
            slots,
            storage_to_slot,
            inventory_storage,
            leaves,
            next_flag,
            status_sources: Vec::new(),
            blocks: vec![OpenBlock {
                id: entry,
                region: root,
                transitions: Vec::new(),
                terminator: None,
            }],
            edges: Vec::new(),
            regions: vec![CleanupRegion {
                id: root,
                parent: None,
                slots: Vec::new(),
                normal_scope_end: UNRESOLVED_EXIT,
            }],
            exits: Vec::new(),
            entry_state: CleanupEntryState {
                live_owned_parameters: Vec::new(),
                conditional_owned_parameters: Vec::new(),
            },
            initial_state: FlowState {
                renewals: BTreeMap::new(),
                live_order: Vec::new(),
                conditional_variants: Vec::new(),
            },
            pending_try_residuals: Vec::new(),
            lexical: None,
            schema: if has_byte_helpers {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
            } else if crate::byte_ops::requires_same_owner_set(function) {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17
            } else if crate::string_ops::replacement::requires(function) {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16
            } else if crate::hir::vec_loop_renewal::requires_in(program, function) {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15
            } else if crate::iterator_ops::function_uses_owned_iterator(function)
                || crate::iterator_ops::function_uses_record_iterator_in(
                    &program.declarations,
                    function,
                )
            {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13
            } else if crate::hir::iterator_loop::function_requires_renewal(function)
                || crate::hir::iterator_loop::function_requires_record_renewal(program, function)
            {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V12
            } else if crate::hir::iterator_loop::function_contains(function) {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11
            } else {
                schema::initial(&function.cleanup)?
            },
        };
        builder.seed_entry(root)?;
        Ok(builder)
    }
}

pub(crate) fn build_program_plans(
    program: &ResolvedProgram,
) -> Result<Vec<CleanupPlan>, Diagnostic> {
    if crate::bounded_output::active_refused() {
        return Err(plan_error(
            "cleanup construction budget was already refused",
        ));
    }
    // Delayed publication keeps the old attached inventories alive while every
    // replacement is staged. Charge this additional overlap before any new
    // index/staging/builder allocation. Attached metadata grants no authority.
    let attached = attached_capacity::program(program)?;
    if !crate::bounded_output::reserve_active_required(attached) {
        return Err(plan_error(
            "attached cleanup overlap exceeds admitted scratch",
        ));
    }
    #[cfg(test)]
    CONSTRUCTION_STARTED.with(|started| started.set(true));
    let index = ModuleIndex::new(program)?;
    let count = program
        .functions
        .len()
        .checked_add(program.function_instances.len())
        .ok_or_else(|| plan_error("cleanup plan staging capacity overflowed"))?;
    let bytes = count
        .checked_mul(std::mem::size_of::<CleanupPlan>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<CleanupPlan>>()))
        .ok_or_else(|| plan_error("cleanup plan staging capacity overflowed"))?;
    if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(plan_error("cleanup plan staging exceeds admitted scratch"));
    }
    let mut plans = Vec::new();
    plans
        .try_reserve_exact(count)
        .map_err(|_| plan_error("cleanup plan staging allocation failed"))?;
    if plans.capacity() != count {
        return Err(plan_error(
            "cleanup plan staging backing exceeds reserved capacity",
        ));
    }
    for function in &program.functions {
        let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
        plans.push(lexical_control::build_with_index(
            program, function, &execution, &index,
        )?);
    }
    for instance in &program.function_instances {
        let execution = crate::hir::FunctionExecutionId::Generic(instance.id.clone());
        plans.push(lexical_control::build_with_index(
            program,
            &instance.function,
            &execution,
            &index,
        )?);
    }
    // Intrinsic parameter identities use infallible bounded formatting. A
    // refused format returns an empty diagnostic carrier and marks the ledger;
    // it must never turn into successful staged-plan publication.
    if crate::bounded_output::active_refused() {
        return Err(plan_error("cleanup construction exceeded admitted scratch"));
    }
    #[cfg(test)]
    {
        // A conservative upper bound on the overlap: the largest in-progress
        // builder (including the held index) plus every staged plan backing.
        // Count full physical capacities, rather than legacy receipt lengths.
        let staged = plans
            .iter()
            .try_fold(
                plans.capacity() * std::mem::size_of::<CleanupPlan>(),
                |bytes, plan| {
                    bytes.checked_add(
                        crate::private_capacity_contract::cleanup_plan_owned_capacity(plan)?,
                    )
                },
            )
            .expect("staged physical capacity census fits usize");
        note_lower_capacity_high_water(lower_capacity_high_water() + staged + attached);
    }
    Ok(plans)
}

/// All physical staging completes before any attached plan is replaced. A
/// failed construction leaves the input inventory unchanged; it grants no
/// authority to an old or invalid attached plan.
pub(crate) fn attach_program_plans(program: &mut ResolvedProgram) -> Result<(), Diagnostic> {
    let plans = build_program_plans(program)?;
    let mut plans = plans.into_iter();
    for function in &mut program.functions {
        function.cleanup_plan = plans.next().expect("complete staged function inventory");
    }
    for instance in &mut program.function_instances {
        instance.function.cleanup_plan = plans.next().expect("complete staged instance inventory");
    }
    debug_assert!(plans.next().is_none());
    Ok(())
}

#[cfg(test)]
thread_local! {
    static CONSTRUCTION_STARTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
mod attached_capacity;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn attached_capacity_for_measurement(
    program: &ResolvedProgram,
) -> Result<usize, Diagnostic> {
    attached_capacity::program(program)
}
