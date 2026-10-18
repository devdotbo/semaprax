//! Selected structural-control receiving closure. Original general APIs remain.
use super::*;
use crate::cleanup_plan::replay::expression_facts::errors::EngineError as Error;
use crate::cleanup_plan::replay::selected_context::child::Permit;
// The closure capture and its new typed Result are admitted before the
// closure is formed. Macro expansion adds no forwarding helper activation.
macro_rules! control_refusal {
    ($budget:ident,$function:ident,$permit:ident,$value:ty,$kind:expr) => {{
        $budget.reserve_array::<(&Permit<'_>, Result<$value, Error>)>(
            $function,
            1,
            "selected control closure receiving",
        )?;
        $budget.charge($function, 2, "selected control closure receiving")?;
        || $permit.control(Reason::Finite($kind))
    }};
}
pub(in crate::cleanup_plan::replay) mod reason;
use reason::{Kind, Reason};
mod blocks;
mod calls;
mod cfg;
mod entry;
mod exits;
mod helpers;
mod status;
pub(in crate::cleanup_plan::replay) use blocks::validate_blocks_and_edges_paid;
pub(in crate::cleanup_plan::replay) use cfg::validate_reachable_acyclic_cfg;
pub(in crate::cleanup_plan::replay) use entry::validate_entry;
pub(in crate::cleanup_plan::replay) use exits::validate_exits;
pub(in crate::cleanup_plan::replay) use status::validate_core as validate_status;

#[cfg(test)]
mod tests;
