//! Exact traversal freeze receiver with caller-paid scratch/draft storage.
use super::*;

pub(in crate::loan_plan) fn inspect_counted<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    region: &'a ResolvedExpr,
    work: &mut WorkCounter,
) -> Result<Option<Proof<'a>>, Diagnostic> {
    let fail = || {
        error("record traversal length, index, nominal reads or whole-root freeze proof is invalid")
    };
    let Some(Prefix {
        source,
        owner,
        element,
        counter,
        length,
        row_index,
        authored,
    }) = prefix(region, |element| {
        crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || crate::hir::owned_leaf_collection::layout(program.declarations(), element)
                .is_some_and(|layout| layout.fields.is_some())
    })
    .map_err(|()| fail())?
    else {
        return Ok(None);
    };
    // Every root occurrence must be the exact child of an authenticated read
    // or an ordinary shared-borrow argument. Mutation, transfer, rebind and
    // aggregate storage are not permitted, even when all row fields are Copy.
    let mut allowed = cfg_directory::Set::new(work.allocation.ok_or_else(storage::marker)?)?;
    let mut pending = Vec::new();
    storage::push(&mut pending, authored, work.allocation)?;
    while let Some(expression) = pending.pop() {
        charge(work)?;
        match &expression.kind {
            ResolvedExprKind::VecFieldRead {
                element: read_element,
                field,
                bytes,
                args,
            } => {
                if let [vector, index] = args.as_slice() {
                    if matches!(&vector.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
                    {
                        if vector.ty != source.ty
                            || vector.ownership != source.ownership
                            || !matches!(&vector.kind, ResolvedExprKind::Place(place) if place == owner)
                            || read_element != element
                            || !place_is(index, row_index)
                            || crate::hir::vec_field::field(program.declarations(), element, field)
                                .and_then(|selected| selected.result_type(*bytes))
                                .as_ref()
                                != Some(&expression.ty)
                        {
                            return Err(fail());
                        }
                        cfg_directory::set_insert(
                            &mut allowed,
                            vector.id.clone(),
                            work.allocation,
                        )?;
                    }
                }
            }
            // Row Loan v1 names this exact region, bound and row index; the
            // freeze covers the whole call that receives the loan.
            ResolvedExprKind::RowLoan { loan, args } => {
                if let [vector, index] = args.as_slice() {
                    if matches!(&vector.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
                    {
                        if vector.ty != source.ty
                            || vector.ownership != source.ownership
                            || !matches!(&vector.kind, ResolvedExprKind::Place(place) if place == owner)
                            || loan.element != *element
                            || loan.traversal != region.id
                            || loan.bound != length.id
                            || !place_is(index, row_index)
                            || expression.ty != *element
                            || expression.ownership != OwnershipMode::Borrow
                        {
                            return Err(fail());
                        }
                        cfg_directory::set_insert(
                            &mut allowed,
                            vector.id.clone(),
                            work.allocation,
                        )?;
                    }
                }
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance: None,
                args,
            } if matches!(
                callee.as_str(),
                crate::vec_ops::GET_ID | crate::vec_ops::LEN_ID
            ) =>
            {
                if let Some(vector) = args.first() {
                    if matches!(&vector.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
                    {
                        if vector.ty != source.ty
                            || vector.ownership != source.ownership
                            || (type_arguments.len() != 1
                                || type_arguments.first() != Some(element))
                            || !matches!(&vector.kind, ResolvedExprKind::Place(place) if place == owner)
                            || (callee.as_str() == crate::vec_ops::GET_ID
                                && (args.len() != 2
                                    || !place_is(&args[1], row_index)
                                    || !crate::hir::copy_record_collection::admitted(
                                        program.declarations(),
                                        element,
                                    )))
                            || (callee.as_str() == crate::vec_ops::LEN_ID && args.len() != 1)
                        {
                            return Err(fail());
                        }
                        cfg_directory::set_insert(
                            &mut allowed,
                            vector.id.clone(),
                            work.allocation,
                        )?;
                    }
                }
            }
            ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } => {
                if matches!(statements.first(), Some(ResolvedStatement::Let { binding, .. }) if binding.name == "#for-borrow-length")
                {
                    return Err(fail());
                }
                if statements.iter().any(|statement| matches!(statement, ResolvedStatement::Assign { binding, .. } if binding.id == owner.root || binding.id == counter.id || binding.id == row_index.id || binding.id == length.id)) { return Err(fail()); }
            }
            _ => {}
        }
        for (_, argument) in
            call_arguments::borrowed_call_arguments_paid(program, expression, work.allocation)?
        {
            if matches!(&argument.kind, ResolvedExprKind::Place(place) if place == owner) {
                cfg_directory::set_insert(&mut allowed, argument.id.clone(), work.allocation)?;
            }
        }
        // Parent expressions are inspected before their children; an exact
        // allowed argument identity cannot authorize another root occurrence.
        if matches!(&expression.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
            && !allowed.contains(&expression.id)
        {
            return Err(fail());
        }
        paid_children::push_children(expression, &mut pending, work.allocation)?;
    }
    Ok(Some(Proof { source, region }))
}

pub(in crate::loan_plan) fn draft(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &ResolvedExpr,
    cfg: &Cfg<'_>,
    work: &mut WorkCounter,
) -> Result<Option<PaidDraft>, Diagnostic> {
    let Some(proof) = inspect_counted(program, expression, work)? else {
        return Ok(None);
    };
    let origin = storage::expression_place(proof.source, work.allocation)?
        .ok_or_else(|| error("record traversal source lacks its exact place"))?;
    Ok(Some(PaidDraft {
        site: proof.region.id.clone(),
        origin,
        origin_span: proof.source.span,
        parent_root: None,
        binding: None,
        start: cfg.node(proof.region, LoanPointPhase::Before)?,
        seeds: cfg_directory::singleton_set(
            cfg.node(proof.region, LoanPointPhase::After)?,
            work.allocation,
        )?,
        cause: LoanCause::TraversalFreeze,
    }))
}

/// Borrowed `Slice<u8>` traversal: the view's resolved owner is frozen from
/// before the complete lowered region until after it. The loan is independent
/// of the view's own liveness, which may end at its last read inside the body.
pub(in crate::loan_plan) fn draft_bytes(
    expression: &ResolvedExpr,
    cfg: &Cfg<'_>,
    aliases: &impl crate::loan_plan::directory_access::Aliases,
    work: &mut WorkCounter,
) -> Result<Option<PaidDraft>, Diagnostic> {
    let Some(region) = crate::hir::byte_traversal_region(expression)? else {
        return Ok(None);
    };
    charge(work)?;
    let place = storage::expression_place(region.view, work.allocation)?
        .ok_or_else(|| error("borrowed byte traversal view lacks its exact place"))?;
    Ok(Some(PaidDraft {
        site: expression.id.clone(),
        origin: aliases.resolve(place, work)?,
        origin_span: region.view.span,
        parent_root: None,
        binding: None,
        start: cfg.node(expression, LoanPointPhase::Before)?,
        seeds: cfg_directory::singleton_set(
            cfg.node(expression, LoanPointPhase::After)?,
            work.allocation,
        )?,
        cause: LoanCause::TraversalFreeze,
    }))
}

pub(in crate::loan_plan) fn inspect<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &'a ResolvedExpr,
    payer: &crate::bounded_output::cfg_reservation::CfgReservation,
) -> Result<Option<Proof<'a>>, Diagnostic> {
    let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    work.allocation = Some(payer);
    inspect_counted(program, expression, &mut work)
}
