//! Selected scalar schema after the unchanged complete profile/owner checks.
//! Original general schema remains independently executable. Dynamic type-fact
//! errors are same-payer funded owned moves, not semantic-error provenance casts.
use super::*;
use crate::cleanup_plan::replay::expression_facts::errors::EngineError;
use crate::cleanup_plan::replay::selected_context::{child::Permit, family::Reason};
pub(in crate::cleanup_plan::replay) fn selected(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<&'static str, EngineError> {
    let _permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<&'static str>(function, budget)
    )?;
    // Actual arguments, summary/choice locals and the funded Diagnostic child
    // receiving controls are charged before entering any selected child.
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a mut ReplayBudget,
        Result<&'static str, EngineError>,
        paid_type_facts::inventory_summary::Summary,
        bool,
        bool,
        bool,
        Result<paid_type_facts::inventory_summary::Summary, Diagnostic>,
        Result<bool, Diagnostic>,
        Diagnostic,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected schema receiving")?;
    budget.charge(function, 11, "selected schema receiving")?;
    // Independently replayed lexical exits select the lexical certificate
    // profile regardless of any attached inventory or plan.
    let helper_sites = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &crate::hir::ResolvedFunction,
            &mut crate::cleanup_plan::replay::ReplayBudget,
            Result<bool, crate::cleanup_plan::replay::expression_facts::errors::EngineError>
        ),
        4,
        "selected schema helper call receiving",
        helper_sites(program, function, budget)
    )?;
    if helper_sites {
        return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19);
    }
    if lexical_cert::requires_lexical(function) {
        return Ok(CLEANUP_PLAN_SCHEMA_V18);
    }
    let inventory = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &crate::hir::ResolvedFunction,
            &mut crate::cleanup_plan::replay::ReplayBudget,
            Result<paid_type_facts::inventory_summary::Summary, Diagnostic>
        ),
        4,
        "selected summary call receiving",
        paid_type_facts::inventory_summary::collect(program, function, budget)
    )?;
    let has_nested_owned_bytes = inventory.has_nested_owned_bytes;
    let has_nested_record_destructure = record_destructure::function_contains(program, function);
    let has_nested_record_update = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &crate::hir::ResolvedFunction,
            &mut crate::cleanup_plan::replay::ReplayBudget,
            Result<bool, Diagnostic>
        ),
        4,
        "selected update call receiving",
        record_destructure::update::selected_contains_paid(program, function, budget)
    )?;
    Ok(if crate::byte_ops::requires_same_owner_set(function) {
        CLEANUP_PLAN_SCHEMA_V17
    } else if crate::string_ops::replacement::requires(function) {
        CLEANUP_PLAN_SCHEMA_V16
    } else if crate::hir::vec_loop_renewal::requires_in(program, function) {
        CLEANUP_PLAN_SCHEMA_V15
    } else if crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &crate::hir::ResolvedFunction,
            &mut crate::cleanup_plan::replay::ReplayBudget,
            Result<bool, crate::cleanup_plan::replay::expression_facts::errors::EngineError>
        ),
        4,
        "selected owner admission call receiving",
        owner_admission(program, function, budget)
    )? {
        CLEANUP_PLAN_SCHEMA_V14
    } else if crate::iterator_ops::function_uses_owned_iterator(function)
        || crate::iterator_ops::function_uses_record_iterator_in(program.declarations(), function)
    {
        CLEANUP_PLAN_SCHEMA_V13
    } else if crate::hir::iterator_loop::function_requires_renewal(function)
        || crate::hir::iterator_loop::function_requires_record_renewal(program, function)
    {
        CLEANUP_PLAN_SCHEMA_V12
    } else if crate::hir::iterator_loop::function_contains(function) {
        CLEANUP_PLAN_SCHEMA_V11
    } else if inventory.has_iterator {
        CLEANUP_PLAN_SCHEMA_V10
    } else if has_nested_record_update {
        CLEANUP_PLAN_SCHEMA_V9
    } else if has_nested_record_destructure {
        CLEANUP_PLAN_SCHEMA_V8
    } else if has_nested_owned_bytes {
        CLEANUP_PLAN_SCHEMA_V7
    } else if inventory.has_variant
        || function
            .requires
            .iter()
            .any(expression_has_explicit_variant_match)
        || function
            .ensures
            .iter()
            .any(expression_has_explicit_variant_match)
        || expression_has_explicit_variant_match(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V6
    } else if function
        .requires
        .iter()
        .any(expression_has_explicit_record_match)
        || function
            .ensures
            .iter()
            .any(expression_has_explicit_record_match)
        || expression_has_explicit_record_match(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V5
    } else if function.requires.iter().any(expression_has_byte_range)
        || function.ensures.iter().any(expression_has_byte_range)
        || expression_has_byte_range(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V4
    } else if function.requires.iter().any(expression_has_option_try)
        || function.ensures.iter().any(expression_has_option_try)
        || expression_has_option_try(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V3
    } else {
        CLEANUP_PLAN_SCHEMA_V2
    })
}

fn helper_sites<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, EngineError> {
    use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
    use crate::loop_calls::byte_writer::HelperBindings;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<HelperBindings<'a>>(function, budget)
    )?;
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a mut ReplayBudget,
        Result<HelperBindings<'a>, Refusal>,
        Result<bool, EngineError>,
        HelperBindings<'a>,
        &'a mut usize,
        &'a mut crate::cleanup_plan::ReplayPhysicalReservation,
        bool,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected schema helper receiving")?;
    budget.charge(function, 9, "selected schema helper receiving")?;
    // Preserve the ordinary API's exact local potential predicate. Imported
    // directory members cannot newly select schema 19 through this typed twin.
    let possible = program.functions().iter().any(|member| {
        member.return_type == ResolvedType::Bytes
            && member
                .params
                .iter()
                .any(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
    });
    if !possible {
        return Ok(false);
    }
    budget.reserve_array::<(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedFunction,
        &mut crate::cleanup_plan::ReplayPhysicalReservation,
        &mut dyn FnMut(usize) -> Result<(), Refusal>,
        &mut usize,
        Result<HelperBindings<'_>, Refusal>,
    )>(function, 1, "selected raw helper call receiving")?;
    budget.charge(function, 6, "selected raw helper call receiving")?;
    let result = {
        let ReplayBudget {
            remaining,
            physical,
            ..
        } = budget;
        let physical = match physical.as_deref_mut() {
            Some(physical) => physical,
            None => return Err(permit.refuse(Refusal::LostPhysical)),
        };
        crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work_inline(
            program,
            function,
            physical,
            &mut |units| match remaining.checked_sub(units) {
                Some(next) => {
                    *remaining = next;
                    Ok(())
                }
                None => {
                    *remaining = 0;
                    Err(Refusal::RenewalWork)
                }
            },
        )
    };
    match result {
        Ok(bindings) => Ok(!bindings.is_empty()),
        Err(reason) => Err(permit.refuse(reason)),
    }
}

fn owner_admission(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, EngineError> {
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<bool>(function, budget)
    )?;
    budget.reserve_array::<[Option<(&ResolvedExpr, usize)>; 514]>(
        function,
        1,
        "selected owner admission stack",
    )?;
    budget.reserve_array::<(usize, Option<(&ResolvedExpr, usize)>, &ResolvedExpr, usize)>(
        function,
        1,
        "selected owner admission receiving",
    )?;
    budget.charge(function, 5, "selected owner admission receiving")?;
    for root in function
        .requires
        .iter()
        .chain(&function.ensures)
        .chain(std::iter::once(&function.body))
    {
        let mut stack = [None; 514];
        stack[0] = Some((root, 0usize));
        let mut len = 1usize;
        while len != 0 {
            len -= 1;
            let (expression, next) = stack[len].take().expect("owner admission frame retained");
            if next == 0 && super::super::owner_admission::required(program, expression) {
                return Ok(true);
            }
            if let Some(child) = replay_expression_child(expression, next) {
                if len + 2 > stack.len() {
                    return Err(permit.family(Reason::OwnerAdmissionDepth));
                }
                stack[len] = Some((expression, next + 1));
                stack[len + 1] = Some((child, 0));
                len += 2;
            }
        }
    }
    Ok(false)
}
