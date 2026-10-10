//! Independent replay driver for cleanup-plan v18 lexical-profile functions.
//!
//! The order is deliberate. The HIR lexical contour is replayed first, so no
//! certificate is read before its meaning is independently known. Structural
//! identity, region, status and call-commit validation then reuses the legacy
//! validators unchanged, certificates are authenticated against both the HIR
//! facts and the plan, and only then do the all-path ownership replay and the
//! typed control skeleton run over the acyclic proof CFG. A failed or stale
//! certificate never reaches a backend.
use super::*;
use crate::ast::LexicalExitKind;

pub(super) fn validate_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let owner = crate::hir::lexical_exit::owner(program, function)?;
    crate::hir::lexical_exit::validate_function(program, function, &owner)?;
    let held_facts = if let Some(physical) = budget.physical.as_deref_mut() {
        let mut held = None;
        lexical_census::validate_size_budget_with_expression_count(program, function, || {
            let facts = expression_facts::expression_facts_with_reservation(function, physical)?;
            let count = facts.len();
            held = Some(facts);
            Ok(count)
        })?;
        held
    } else {
        lexical_census::validate_size_budget(program, function)?;
        None
    };
    budget.charge(
        function,
        plan_structure_units(plan),
        "structural validation",
    )?;

    let expression_facts = match held_facts {
        Some(facts) => facts,
        None => expression_facts(function)?,
    };
    validate_inventory_coverage(program, function)?;
    validate_required_status_sources(program, function)?;
    let storage = validate_slots(function)?;
    let leaves = collect_leaves(function)?;
    validate_status_sources(function, &expression_facts)?;
    validate_regions(function, &storage)?;
    validate_entry(function, &storage, &leaves)?;
    validate_blocks_and_edges(
        program,
        function,
        &expression_facts,
        &storage,
        &leaves,
        helpers,
    )?;

    let facts = lexical_hir::collect(function, budget)?;
    let return_operands = facts
        .exits
        .iter()
        .filter(|exit| exit.kind == LexicalExitKind::Return)
        .filter_map(|exit| exit.value.map(|operand| operand.id.clone()))
        .collect::<BTreeSet<_>>();
    validate_exits(program, function, &storage, &leaves, &return_operands)?;
    let auth = lexical_cert::authenticate(program, function, &owner, &facts, budget)?;
    validate_reference_coverage(function)?;
    validate_reachable_acyclic_cfg(function)?;
    // Lexical functions always replay every path: no summary shortcut applies.
    validate_path_states(
        program,
        function,
        &storage,
        &leaves,
        budget,
        Some(&auth),
        helpers,
    )?;
    validate_typed_control_skeleton_with_index(program, function, budget, helpers)
}
