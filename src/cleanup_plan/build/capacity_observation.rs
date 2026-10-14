//! Component census telemetry, never a reservation or allocator/RSS measurement.
//! The overlap upper is the sum of this invocation's component census maxima;
//! shared identity aliases and legacy collection allowances can overcount it.
//! Existing text-length terms elsewhere are retained legacy model terms, so this
//! is not asserted to bound every physical allocator Layout.
use super::*;
use std::cell::Cell;
mod legacy;
mod tests;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Components {
    pub builder: usize,
    pub staged: usize,
    pub staged_max: usize,
    pub held_index: usize,
    pub phase_overlap_upper: usize,
    pub attached: usize,
    pub valid: bool,
}
impl Components {
    /// Compatibility sum of separate component maxima; not a simultaneous peak.
    pub fn upper(self) -> usize {
        self.builder
            .saturating_add(self.staged_max)
            .saturating_add(self.attached)
    }
}
thread_local! {
    static BUILDER_MAX: Cell<usize> = const { Cell::new(0) };
    static OBSERVATIONS_VALID: Cell<bool> = const { Cell::new(true) };
    static STANDALONE: Cell<usize> = const { Cell::new(0) };
    static PHASE_OVERLAP: Cell<usize> = const { Cell::new(0) };
    static OVERLAP: Cell<usize> = const { Cell::new(0) };
    static CURRENT: Cell<Option<Components>> = const { Cell::new(None) };
    static LAST: Cell<Components> = const { Cell::new(Components { builder: 0, staged: 0, staged_max: 0, held_index: 0, phase_overlap_upper: 0, attached: 0, valid: true }) };
}
pub(super) fn reset_lower_capacity_high_water() {
    STANDALONE.with(|c| c.set(0));
    BUILDER_MAX.with(|c| c.set(0));
    OBSERVATIONS_VALID.with(|c| c.set(true));
    OVERLAP.with(|c| c.set(0));
    PHASE_OVERLAP.with(|c| c.set(0));
    LAST.with(|c| {
        c.set(Components {
            valid: true,
            ..Components::default()
        })
    });
}
/// Maximum standalone builder component, including builders inside construction.
/// This retains the historical component model, not allocator Layout usage.
pub(super) fn standalone_builder_high_water() -> usize {
    BUILDER_MAX.with(Cell::get)
}
/// Maximum invocation-local sum of builder, staged and attached component maxima.
/// Separate component maxima need not occur simultaneously.
pub(super) fn construction_component_maxima_upper() -> Option<usize> {
    OBSERVATIONS_VALID
        .with(Cell::get)
        .then(|| OVERLAP.with(Cell::get))
}
/// Maximum sampled phase-paired component census. Builder samples already include the
/// held index; post-builder staging samples include the index separately.
/// Sampling and the legacy backing model do not establish allocator highwater.
pub(super) fn construction_phase_sampled_census() -> Option<usize> {
    OBSERVATIONS_VALID
        .with(Cell::get)
        .then(|| PHASE_OVERLAP.with(Cell::get))
}
/// Compatibility reader intentionally retains the historical saturated numeric
/// result; use the named Option accessors when observation validity matters.
pub(super) fn lower_capacity_high_water() -> usize {
    STANDALONE.with(Cell::get).max(OVERLAP.with(Cell::get))
}
pub(super) fn note_lower_capacity_high_water(bytes: usize) {
    BUILDER_MAX.with(|water| water.set(water.get().max(bytes)));
    CURRENT.with(|cell| {
        if let Some(mut components) = cell.get() {
            components.builder = components.builder.max(bytes);
            sample_phase(&mut components, bytes);
            cell.set(Some(components));
        } else {
            STANDALONE.with(|water| water.set(water.get().max(bytes)));
        }
    });
}
pub(super) struct Construction {
    previous: Option<Components>,
}
impl Construction {
    pub fn begin() -> Self {
        Self {
            previous: CURRENT.with(|c| {
                c.replace(Some(Components {
                    valid: true,
                    ..Components::default()
                }))
            }),
        }
    }
}
impl Drop for Construction {
    fn drop(&mut self) {
        let mut observed = CURRENT
            .with(|c| c.replace(self.previous))
            .expect("active construction census");
        if observed
            .builder
            .checked_add(observed.staged_max)
            .and_then(|bytes| bytes.checked_add(observed.attached))
            .is_none()
        {
            observed.valid = false;
        }
        OBSERVATIONS_VALID.with(|c| c.set(c.get() && observed.valid));
        LAST.with(|c| c.set(observed));
        OVERLAP.with(|c| c.set(c.get().max(observed.upper())));
        PHASE_OVERLAP.with(|c| c.set(c.get().max(observed.phase_overlap_upper)));
    }
}
fn update(f: impl FnOnce(&mut Components)) {
    CURRENT.with(|cell| {
        if let Some(mut components) = cell.get() {
            f(&mut components);
            cell.set(Some(components));
        }
    });
}
fn sample_phase(components: &mut Components, builder_or_held_index: usize) {
    match builder_or_held_index
        .checked_add(components.staged)
        .and_then(|bytes| bytes.checked_add(components.attached))
    {
        Some(bytes) => components.phase_overlap_upper = components.phase_overlap_upper.max(bytes),
        None => components.valid = false,
    }
}
pub(super) fn held_index(bytes: usize) {
    update(|c| {
        c.held_index = bytes;
        sample_phase(c, bytes);
    });
}
pub(super) fn attached(bytes: usize) {
    update(|c| c.attached = bytes);
}
pub(super) fn staged(plans: &Vec<CleanupPlan>) {
    let bytes = plans
        .capacity()
        .checked_mul(std::mem::size_of::<CleanupPlan>())
        .and_then(|backing| {
            plans.iter().try_fold(backing, |bytes, plan| {
                bytes.checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(plan)?,
                )
            })
        });
    update(|c| match bytes {
        Some(bytes) => {
            c.staged = bytes;
            c.staged_max = c.staged_max.max(bytes);
            let held = c.held_index;
            sample_phase(c, held);
        }
        None => c.valid = false,
    });
}
fn expression_backing(id: &ExpressionId) -> usize {
    match id.owned_allocation_bytes() {
        Some(bytes) => bytes,
        None => {
            update(|c| c.valid = false);
            0
        }
    }
}
pub(super) fn helper_backing(builder: &PlanBuilder<'_>) -> usize {
    let ordinary = if matches!(builder.byte_helpers, helper_index::HelperSites::Owned(_)) {
        builder.ordinary_index.map_or(0, |held| match held {
            helper_index::ordinary_view::OrdinaryView::Construction(index) => {
                index.owned_capacity_bytes()
            }
            helper_index::ordinary_view::OrdinaryView::Validation(frontier) => {
                frontier.owned_capacity_bytes()
            }
        })
    } else {
        0
    };
    builder
        .byte_helpers
        .owned_capacity_bytes()
        .saturating_add(ordinary)
}
pub(super) fn conditional_backing(state: &FlowState) -> usize {
    state.conditional_variants.capacity() * std::mem::size_of::<ConditionalFlowVariant>()
        + state
            .conditional_variants
            .iter()
            .map(|variant| {
                cleanup_place_owned_capacity(&variant.root)
                    + variant.variant.owned_allocation_bytes()
                    + variant.cases.capacity()
                        * std::mem::size_of::<(DeclarationId, Vec<LivenessFlagId>)>()
                    + variant
                        .cases
                        .iter()
                        .map(|(case, flags)| {
                            case.owned_allocation_bytes()
                                + flags.capacity() * std::mem::size_of::<LivenessFlagId>()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
}
pub(super) fn string_owner_moves(builder: &PlanBuilder<'_>) -> usize {
    builder.string_owner_moves.len()
        * (std::mem::size_of::<ExpressionId>() + std::mem::size_of::<BTreeSet<ExpressionId>>())
        + builder
            .string_owner_moves
            .iter()
            .map(expression_backing)
            .sum::<usize>()
}

pub(super) mod actual_attribution;
