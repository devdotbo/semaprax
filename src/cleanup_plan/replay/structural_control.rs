//! Selected structural validation borrows immutable plan facts and reserves
//! every retained scratch constructor before allocation. Legacy routes stay exact.
use super::*;
mod blocks;
#[cfg(test)]
pub(in crate::cleanup_plan::replay) mod borrowed_ids;
mod calls;
mod cfg;
pub(super) use blocks::validate_blocks_and_edges;
mod entry;
mod exits;
pub(super) use cfg::validate_reachable_acyclic_cfg;
pub(super) use entry::validate_entry;
pub(super) use exits::validate_exits;

fn insert_copy<T: Copy + Ord>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    set: &mut BTreeSet<T>,
    value: T,
    phase: &str,
) -> Result<bool, Diagnostic> {
    if !set.contains(&value) {
        budget.reserve_btree_node::<T, ()>(function, phase)?;
    }
    Ok(set.insert(value))
}

#[cfg(test)]
mod tests;
