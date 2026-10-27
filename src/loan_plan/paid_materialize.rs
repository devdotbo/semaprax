//! Same canonical output order; actual result/temporary/replacement payment.
use super::*;

// Inline the successful append: no intermediate owned Diagnostic Results.
// The actual immutable final Box fields and all prior IDs remain complete.
macro_rules! append_endpoint_id {
    ($ids:expr, $id:expr, $payer:expr) => {{
        let ids: &mut Box<[LoanId]> = $ids;
        let id = $id;
        let payer = $payer;
        let count = match ids.len().checked_add(1) {
            Some(count) => count,
            None => {
                payer.arithmetic();
                return Err(storage::marker());
            }
        };
        match payer.array::<LoanId>(count) {
            Ok(()) => (),
            Err(_) => return Err(storage::marker()),
        }
        let requested = match count.checked_mul(std::mem::size_of::<LoanId>()) {
            Some(bytes) => bytes,
            None => {
                payer.arithmetic();
                return Err(storage::marker());
            }
        };
        let mut values = Vec::new();
        if values.try_reserve_exact(count).is_err() {
            payer.allocation(requested);
            return Err(storage::marker());
        }
        if values.capacity() != count {
            let excess = values
                .capacity()
                .saturating_sub(count)
                .saturating_mul(std::mem::size_of::<LoanId>());
            let _ = payer.capacity(requested, excess);
            return Err(storage::marker());
        }
        values.extend_from_slice(ids);
        values.push(id);
        *ids = values.into_boxed_slice();
    }};
}

pub(super) fn materialize<S>(
    cfg: Cfg<'_>,
    drafts: Vec<CfgDraft<S>>,
    parents: Vec<Option<LoanId>>,
    edge_live: Vec<Vec<LoanId>>,
    termination_edges: Vec<Vec<u16>>,
    payer: &crate::bounded_output::cfg_reservation::CfgReservation,
) -> Result<LoanPlan, Diagnostic> {
    // The draft count is exact. Result collection need not retain spare
    // geometric capacity in the authoritative physical loan inventory.
    payer
        .reserve(std::mem::size_of::<LoanPlan>())
        .map_err(|_| storage::marker())?;
    let mut loans = cfg_directory::vector(drafts.len(), payer)?;
    for (index, draft) in drafts.iter().enumerate() {
        let end_edges = storage::clone_vec(&termination_edges[index], Some(payer))?;
        if end_edges.is_empty() {
            return Err(error("loan has no bounded CFG termination edge"));
        }
        let mut ends = cfg_directory::vector(end_edges.len(), payer)?;
        ends.extend(
            end_edges
                .iter()
                .map(|edge| cfg.points[cfg.edges[*edge as usize].1 as usize].clone()),
        );
        ends.dedup();
        loans.push(Loan {
            id: LoanId(index as u16),
            site: draft.site.clone(),
            origin: storage::clone_place(&draft.origin, Some(payer))?,
            parent: parents[index],
            start: cfg.points[draft.start as usize].clone(),
            ends,
            end_edges,
            cause: draft.cause.clone(),
        });
    }
    let mut endpoints = cfg_directory::vector(cfg.points.len(), payer)?;
    for (node, point) in cfg.points.iter().enumerate() {
        // Both producers append LoanId in increasing loan-index order. Merge
        // borrowed edge rows; never sort or repair canonical plan vectors.
        let live_before = ordered_union::merge(
            cfg.predecessors(node)
                .iter()
                .filter_map(|from| cfg.edges.binary_search(&(*from, node as u16)).ok())
                .map(|edge| edge_live[edge].as_slice()),
            payer,
        )?;
        let live_after = ordered_union::merge(
            cfg.successor_edges(node)
                .map(|edge| cfg.edges[edge as usize].1)
                .filter_map(|to| cfg.edges.binary_search(&(node as u16, to)).ok())
                .map(|edge| edge_live[edge].as_slice()),
            payer,
        )?;
        endpoints.push(LoanEndpoint {
            point: point.clone(),
            live_before: compact_ids::from_vec_paid(live_before, payer)?,
            starts: Box::new([]),
            kills: Box::new([]),
            live_after: compact_ids::from_vec_paid(live_after, payer)?,
        });
    }
    // Publish the same dense endpoints first, then append IDs in the original
    // loan/termination-edge order. No temporary per-point Vec directory exists.
    payer
        .reserve(ENDPOINT_LOOP_CONTROLS)
        .map_err(|_| storage::marker())?;
    for loan in &loans {
        payer
            .reserve(ENDPOINT_APPEND_CONTROLS)
            .map_err(|_| storage::marker())?;
        append_endpoint_id!(
            &mut endpoints[*cfg.nodes.get(&loan.start).expect("start point retained") as usize]
                .starts,
            loan.id,
            payer
        );
        for edge in &loan.end_edges {
            payer
                .reserve(ENDPOINT_APPEND_CONTROLS)
                .map_err(|_| storage::marker())?;
            let to = cfg.edges[*edge as usize].1;
            append_endpoint_id!(&mut endpoints[to as usize].kills, loan.id, payer);
        }
    }
    let mut edges = cfg_directory::vector(cfg.edges.len(), payer)?;
    for ((from, to), live) in cfg.edges.iter().zip(edge_live) {
        edges.push(LoanEdge {
            from: *from,
            to: *to,
            live: compact_ids::from_vec_paid(live, payer)?,
        });
    }
    let schema = if cfg
        .expressions
        .iter()
        .any(|e| matches!(e.kind, ResolvedExprKind::LexicalExit { .. }))
    {
        LOAN_PLAN_SCHEMA_V3
    } else if loans
        .iter()
        .any(|loan| loan.cause == LoanCause::TraversalFreeze)
    {
        LOAN_PLAN_SCHEMA_V2
    } else {
        LOAN_PLAN_SCHEMA_V1
    };
    Ok(LoanPlan {
        schema,
        loans,
        endpoints,
        edges,
    })
}

// Actual additional borrowed loop controls. No owning IR/result storage bank.
const ENDPOINT_LOOP_CONTROLS: usize = 2 * std::mem::size_of::<std::slice::Iter<'static, Loan>>()
    + 4 * std::mem::size_of::<&Loan>()
    + 2 * std::mem::size_of::<std::slice::Iter<'static, u16>>()
    + 4 * std::mem::size_of::<&u16>();
const ENDPOINT_APPEND_CONTROLS: usize = 4 * std::mem::size_of::<&mut Box<[LoanId]>>()
    + 4 * std::mem::size_of::<&crate::bounded_output::cfg_reservation::CfgReservation>()
    + 4 * std::mem::size_of::<LoanId>()
    + 4 * std::mem::size_of::<Vec<LoanId>>()
    + 2 * std::mem::size_of::<Result<(), crate::bounded_output::cfg_reservation::Refused>>()
    + 2 * std::mem::size_of::<Result<(), std::collections::TryReserveError>>()
    + 4 * std::mem::size_of::<Box<[LoanId]>>()
    + 12 * std::mem::size_of::<usize>()
    + 4 * std::mem::size_of::<Option<usize>>()
    + 4 * std::mem::size_of::<&[LoanId]>()
    + 2 * std::mem::size_of::<Option<&u16>>()
    + 4 * std::mem::size_of::<bool>();

#[cfg(test)]
mod before;
mod ordered_union;
#[cfg(test)]
mod tests;
