//! Capacity rule for the explicit `FusedConstructionV1` allocation strategy.
//!
//! A fused construction charges one backing-producing event and its whole
//! `rows * width` extent, with no refund. That charge joins the unchanged
//! per-path event and cumulative payload limits. This module only checks the
//! flow it is given. Count, capacity, destination, fresh-row and no-escape
//! authority must come from independent source and HIR derivation, and the
//! executor must reserve its own lease metadata. Neither is minted here.

use super::{
    allocation_error, flow_node_bound, invariant, require_identity, AllocationStrategy,
    AllocationSummary, CapacityError, CapacityFlow, MAX_FLOW_NODES,
};

/// Explicit row ceiling of this versioned construction strategy. Source/HIR
/// and each executor independently prove that their actual record and owner
/// layouts fit the existing limits; this ceiling grants no layout admission.
pub(crate) const MAX_FUSED_ROWS: u32 = 4_096;

/// Structural gate, run before the ordinary node walk visits the children.
/// `nodes` is that walk's count so far, including this fused node. The body
/// scan continues the same count under the same `MAX_FLOW_NODES` bound: it
/// refuses only when the ordinary walk would also exceed it.
pub(super) fn validate(
    function: &str,
    strategy: AllocationStrategy,
    producer: &str,
    rows: u32,
    width: u64,
    body: &CapacityFlow,
    nodes: usize,
) -> Result<(), CapacityError> {
    if strategy != AllocationStrategy::FusedConstructionV1 {
        return Err(invariant(
            Some(function),
            "fused construction flow requires the explicit FusedConstructionV1 allocation strategy",
        ));
    }
    require_identity(producer, Some(function), "fused construction producer")?;
    if rows == 0 || width == 0 {
        return Err(invariant(
            Some(function),
            "fused construction needs positive rows and width",
        ));
    }
    extent(function, rows, width)?;
    let mut found = 0usize;
    // Every push is admitted first, so `visited + pending.len()` never exceeds
    // the bound and neither the counter nor the stack can outgrow it.
    let mut visited = nodes;
    admit(function, visited, 0, 1)?;
    let mut pending = vec![(body, true)];
    while let Some((flow, unconditional)) = pending.pop() {
        visited += 1;
        match flow {
            CapacityFlow::BytesCopy {
                site,
                conservative_payload_bytes,
            } if site == producer => {
                found += 1;
                if !unconditional || *conservative_payload_bytes != width {
                    return Err(producer_mismatch(function));
                }
            }
            CapacityFlow::FusedConstruction { .. } => {
                return Err(invariant(
                    Some(function),
                    "nested fused construction is closed",
                ));
            }
            CapacityFlow::Sequence(children) => {
                admit(function, visited, pending.len(), children.len())?;
                pending.extend(children.iter().map(|child| (child, unconditional)));
            }
            CapacityFlow::Alternative(children) => {
                admit(function, visited, pending.len(), children.len())?;
                pending.extend(children.iter().map(|child| (child, false)));
            }
            CapacityFlow::Loop { condition, body }
            | CapacityFlow::FiniteOwnedLoop {
                condition, body, ..
            } => {
                admit(function, visited, pending.len(), 2)?;
                pending.push((condition.as_ref(), false));
                pending.push((body.as_ref(), false));
            }
            _ => {}
        }
    }
    if found != 1 {
        return Err(producer_mismatch(function));
    }
    Ok(())
}

/// Replace the folded condition/body summaries with the single backing charge.
pub(super) fn charge(
    function: &str,
    rows: u32,
    width: u64,
    condition: &AllocationSummary,
    body: &AllocationSummary,
) -> Result<AllocationSummary, CapacityError> {
    if condition.sites != 0 || condition.stdin_sites != 0 || condition.fused != 0 {
        return Err(allocation_error(
            function,
            "fused construction condition reaches an allocation",
        ));
    }
    if body.sites != 1 || body.stdin_sites != 0 || body.fused != 0 || body.payload_bytes != width {
        return Err(allocation_error(
            function,
            "fused construction body allocates outside its single producer",
        ));
    }
    Ok(AllocationSummary {
        sites: 1,
        stdin_sites: 0,
        payload_bytes: extent(function, rows, width)?,
        fused: 1,
    })
}

fn extent(function: &str, rows: u32, width: u64) -> Result<u64, CapacityError> {
    if rows > MAX_FUSED_ROWS {
        return Err(allocation_error(
            function,
            format!("fused construction reaches {rows} rows; limit is {MAX_FUSED_ROWS}"),
        ));
    }
    let extent = u64::from(rows).checked_mul(width).ok_or_else(|| {
        allocation_error(function, "fused construction backing extent overflowed")
    })?;
    if extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES {
        return Err(allocation_error(
            function,
            format!(
                "fused construction backing extent {extent} exceeds the per-value limit {}",
                crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
            ),
        ));
    }
    Ok(extent)
}

/// Refuse before growing the scan past the ordinary walk's node bound.
fn admit(function: &str, visited: usize, pending: usize, more: usize) -> Result<(), CapacityError> {
    let total = visited
        .checked_add(pending)
        .and_then(|total| total.checked_add(more));
    if total.is_none_or(|total| total > MAX_FLOW_NODES) {
        return Err(flow_node_bound(function));
    }
    Ok(())
}

fn producer_mismatch(function: &str) -> CapacityError {
    invariant(
        Some(function),
        "fused construction producer is not the exact unconditional body producer",
    )
}
