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
    pub attached: usize,
    pub valid: bool,
}
impl Components {
    pub fn upper(self) -> usize {
        self.builder
            .saturating_add(self.staged)
            .saturating_add(self.attached)
    }
}
thread_local! {
    static STANDALONE: Cell<usize> = const { Cell::new(0) };
    static OVERLAP: Cell<usize> = const { Cell::new(0) };
    static CURRENT: Cell<Option<Components>> = const { Cell::new(None) };
    static LAST: Cell<Components> = const { Cell::new(Components { builder: 0, staged: 0, attached: 0, valid: true }) };
}
pub(super) fn reset_lower_capacity_high_water() {
    STANDALONE.with(|c| c.set(0));
    OVERLAP.with(|c| c.set(0));
    LAST.with(|c| {
        c.set(Components {
            valid: true,
            ..Components::default()
        })
    });
}
pub(super) fn lower_capacity_high_water() -> usize {
    STANDALONE.with(Cell::get).max(OVERLAP.with(Cell::get))
}
pub(super) fn note_lower_capacity_high_water(bytes: usize) {
    CURRENT.with(|cell| {
        if let Some(mut components) = cell.get() {
            components.builder = components.builder.max(bytes);
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
        let observed = CURRENT
            .with(|c| c.replace(self.previous))
            .expect("active construction census");
        LAST.with(|c| c.set(observed));
        OVERLAP.with(|c| c.set(c.get().max(observed.upper())));
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
pub(super) fn attached(bytes: usize) {
    update(|c| c.attached = bytes);
}
pub(super) fn staged(plans: &Vec<CleanupPlan>) {
    let bytes = plans.iter().try_fold(
        plans.capacity() * std::mem::size_of::<CleanupPlan>(),
        |bytes, plan| {
            bytes.checked_add(crate::private_capacity_contract::cleanup_plan_owned_capacity(plan)?)
        },
    );
    update(|c| match bytes {
        Some(bytes) => c.staged = bytes,
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
