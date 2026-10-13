//! Typed byte-capacity projection under an explicit `FusedConstructionV1`
//! selection.
//!
//! The selection is a [`SelectedFlow`] taken from a source/HIR `Joined` over
//! this exact program. The ordinary walk is unchanged except at the one loop
//! whose condition and body are the joined nodes, by pointer identity. That
//! loop projects as `CapacityFlow::FusedConstruction`: its producer is the
//! joined `core.bytes.zeroed` expression identity, which is the walk's own
//! `BytesCopy` site name, and its condition and body flows are the ones
//! actually projected. Every other loop keeps its Legacy flow. The program is
//! then analyzed under `FusedConstructionV1` against the unchanged 32-event
//! and 2 MiB per-path limits. The walk must match the joined loop exactly
//! once, and the selected function must carry exactly one fused backing. A
//! selection from a foreign, decoded or stale program matches no loop here
//! and refuses.
use super::*;
use crate::hir::fused_construction::join::SelectedFlow;
use crate::hir::selected_fused_candidate::Selected;

impl<'a> ValueFactIndex<'a> {
    fn selected(program: &'a ResolvedProgram, flow: SelectedFlow<'a>) -> Self {
        Self {
            selected: Some(flow),
            ..Self::new(program)
        }
    }
}

/// Pointer-only discovery of the joined loop during the walk.
pub(super) fn selects(
    facts: &ValueFactIndex<'_>,
    condition: &ResolvedExpr,
    body: &ResolvedExpr,
) -> bool {
    let matched = facts.selected.is_some_and(|flow| {
        std::ptr::eq(flow.condition, condition) && std::ptr::eq(flow.body, body)
    });
    if matched {
        facts.matched.set(facts.matched.get().saturating_add(1));
    }
    matched
}

/// The selected loop's flow over its actually projected condition and body.
pub(super) fn flow(
    facts: &ValueFactIndex<'_>,
    condition: crate::byte_data_capacity::CapacityFlow,
    body: crate::byte_data_capacity::CapacityFlow,
) -> Result<crate::byte_data_capacity::CapacityFlow, Diagnostic> {
    let flow = facts
        .selected
        .ok_or_else(|| hir_error("fused construction frame has no held selection"))?;
    Ok(crate::byte_data_capacity::CapacityFlow::FusedConstruction {
        producer: flow.producer.to_owned(),
        rows: flow.rows,
        width: flow.width,
        condition: Box::new(condition),
        body: Box::new(body),
    })
}

/// The actually projected selected inputs, for capacity-limit regression
/// tests that alter the flow after projection.
#[cfg(test)]
pub(crate) fn selected_inputs_for_test<'a>(
    program: &'a ResolvedProgram,
    selected: Selected<'a>,
) -> Result<Vec<crate::byte_data_capacity::FunctionCapacityInput>, Diagnostic> {
    let facts = ValueFactIndex::selected(program, selected.flow());
    let inputs = byte_data_capacity_inputs_with(program, &facts)?;
    assert_eq!(facts.matched.get(), 1, "the joined loop projects once");
    Ok(inputs)
}

/// Selected analysis for one joined loop of `program`. A [`Selected`] can
/// only be built from a `Joined`, so its rows, width and producer are the
/// joined facts rather than caller-supplied numbers.
pub(crate) fn analyze_selected_byte_data_capacity<'a>(
    program: &'a ResolvedProgram,
    selected: Selected<'a>,
) -> Result<crate::byte_data_capacity::ProgramCapacitySummary, Diagnostic> {
    if !selected.held_by(program) {
        return Err(hir_error(
            "selected fused construction is not held by this program",
        ));
    }
    let function = selected.function();
    let facts = ValueFactIndex::selected(program, selected.flow());
    let inputs = byte_data_capacity_inputs_with(program, &facts)?;
    if facts.matched.get() != 1 {
        return Err(hir_error(
            "selected fused construction loop is not exactly one loop of this program",
        ));
    }
    let summary = crate::byte_data_capacity::analyze_with_strategy(
        &inputs,
        crate::byte_data_capacity::AllocationStrategy::FusedConstructionV1,
    )
    .map_err(|error| {
        let primary = Diagnostic::io(error.diagnostic.code(), error.to_string());
        match error.witness_help {
            Some(help) => primary.with_help(help),
            None => primary,
        }
    })?;
    if summary
        .function(function)
        .is_none_or(|summary| summary.fused_backing_events != 1)
    {
        return Err(hir_error(
            "selected fused construction did not charge exactly one fused backing",
        ));
    }
    Ok(summary)
}
