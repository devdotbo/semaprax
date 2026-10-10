//! Independent reconstruction of the complete record-traversal freeze region.
//! A compiler spelling is a discovery marker only; every structural fact is
//! checked again before it can create loan authority.
use super::*;
use crate::hir::ResolvedBinding;

pub(crate) struct Proof<'a> {
    pub(crate) source: &'a ResolvedExpr,
    pub(crate) region: &'a ResolvedExpr,
}

fn place_is(expression: &ResolvedExpr, binding: &ResolvedBinding) -> bool {
    expression.ty == binding.ty
        && expression.ownership == binding.ownership
        && matches!(&expression.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty())
}
fn usize_value(expression: &ResolvedExpr, expected: usize) -> bool {
    expression.ty == ResolvedType::Usize
        && expression.ownership == OwnershipMode::Value
        && matches!(expression.kind, ResolvedExprKind::Usize(value) if value == expected as u64)
}
fn scalar(binding: &ResolvedBinding) -> bool {
    binding.ty == ResolvedType::Usize && binding.ownership == OwnershipMode::Value
}

pub(crate) fn inspect<'a>(
    program: &ResolvedProgram,
    region: &'a ResolvedExpr,
) -> Result<Option<Proof<'a>>, Diagnostic> {
    inspect_counted(
        program,
        region,
        &mut WorkCounter::new(MAX_LOAN_PLAN_WORK_V1),
    )
}

fn inspect_counted<'a>(
    program: &ResolvedProgram,
    region: &'a ResolvedExpr,
    work: &mut WorkCounter,
) -> Result<Option<Proof<'a>>, Diagnostic> {
    let ResolvedExprKind::Block { statements, tail } = &region.kind else {
        return Ok(None);
    };
    let Some(ResolvedStatement::Let {
        binding: marker, ..
    }) = statements.first()
    else {
        return Ok(None);
    };
    if marker.name != "#for-borrow-length" {
        return Ok(None);
    }
    let fail = || {
        error("record traversal length, index, nominal reads or whole-root freeze proof is invalid")
    };
    let [ResolvedStatement::Let {
        binding: length,
        mutable: false,
        value: length_value,
        ..
    }, ResolvedStatement::Let {
        binding: counter,
        mutable: true,
        value: zero,
        ..
    }, (ResolvedStatement::While {
        condition, body, ..
    }
    | ResolvedStatement::LexicalWhile {
        condition, body, ..
    })] = statements.as_slice()
    else {
        return Err(fail());
    };
    if !scalar(length)
        || !scalar(counter)
        || !usize_value(zero, 0)
        || !usize_value(tail, 0)
        || region.ty != ResolvedType::Usize
        || region.ownership != OwnershipMode::Value
    {
        return Err(fail());
    }
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &length_value.kind
    else {
        return Err(fail());
    };
    let [element] = type_arguments.as_slice() else {
        return Err(fail());
    };
    let [source] = args.as_slice() else {
        return Err(fail());
    };
    let ResolvedExprKind::Place(owner) = &source.kind else {
        return Err(fail());
    };
    if callee.as_str() != crate::vec_ops::LEN_ID
        || !owner.projections.is_empty()
        || source.ty != crate::vec_ops::resolved_vec(element.clone())
        || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || !(crate::hir::copy_record_collection::admitted(&program.declarations, element)
            || crate::hir::owned_leaf_collection::layout(&program.declarations, element)
                .is_some_and(|layout| layout.fields.is_some()))
        || length_value.ty != ResolvedType::Usize
        || length_value.ownership != OwnershipMode::Value
    {
        return Err(fail());
    }
    let ResolvedExprKind::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return Err(fail());
    };
    if !place_is(left, counter) || !place_is(right, length) || condition.ty != ResolvedType::Bool {
        return Err(fail());
    }
    let ResolvedExprKind::Block {
        statements: iteration,
        tail: iteration_tail,
    } = &body.kind
    else {
        return Err(fail());
    };
    let [ResolvedStatement::Let {
        binding: row_index,
        mutable: false,
        value: index_snapshot,
        ..
    }, ResolvedStatement::Let {
        binding: discarded,
        mutable: false,
        value: authored,
        ..
    }, ResolvedStatement::Assign {
        binding: incremented,
        field: None,
        value: increment,
        ..
    }] = iteration.as_slice()
    else {
        return Err(fail());
    };
    if !scalar(row_index)
        || !place_is(index_snapshot, counter)
        || incremented != counter
        || discarded.ty != authored.ty
        || discarded.ownership != authored.ownership
        || !usize_value(iteration_tail, 0)
    {
        return Err(fail());
    }
    let ResolvedExprKind::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &increment.kind
    else {
        return Err(fail());
    };
    if !place_is(left, counter) || !usize_value(right, 1) {
        return Err(fail());
    }
    // Every root occurrence must be the exact child of an authenticated read
    // or an ordinary shared-borrow argument. Mutation, transfer, rebind and
    // aggregate storage are not permitted, even when all row fields are Copy.
    let mut allowed = BTreeSet::new();
    let mut pending = vec![authored];
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
                            || crate::hir::vec_field::field(&program.declarations, element, field)
                                .and_then(|selected| selected.result_type(*bytes))
                                != Some(expression.ty.clone())
                        {
                            return Err(fail());
                        }
                        allowed.insert(vector.id.clone());
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
                                        &program.declarations,
                                        element,
                                    )))
                            || (callee.as_str() == crate::vec_ops::LEN_ID && args.len() != 1)
                        {
                            return Err(fail());
                        }
                        allowed.insert(vector.id.clone());
                    }
                }
            }
            ResolvedExprKind::Block { statements, .. } => {
                if matches!(statements.first(), Some(ResolvedStatement::Let { binding, .. }) if binding.name == "#for-borrow-length")
                {
                    return Err(fail());
                }
                if statements.iter().any(|statement| matches!(statement, ResolvedStatement::Assign { binding, .. } if binding.id == owner.root || binding.id == counter.id || binding.id == row_index.id || binding.id == length.id)) { return Err(fail()); }
            }
            _ => {}
        }
        for (_, argument) in borrowed_call_arguments(program, expression) {
            if matches!(&argument.kind, ResolvedExprKind::Place(place) if place == owner) {
                allowed.insert(argument.id.clone());
            }
        }
        // Parent expressions are inspected before their children; an exact
        // allowed argument identity cannot authorize another root occurrence.
        if matches!(&expression.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
            && !allowed.contains(&expression.id)
        {
            return Err(fail());
        }
        push_children(expression, &mut pending);
    }
    Ok(Some(Proof { source, region }))
}

pub(super) fn draft(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    cfg: &Cfg<'_>,
    work: &mut WorkCounter,
) -> Result<Option<CfgDraft>, Diagnostic> {
    let Some(proof) = inspect_counted(program, expression, work)? else {
        return Ok(None);
    };
    let origin = expression_place(proof.source)
        .ok_or_else(|| error("record traversal source lacks its exact place"))?;
    Ok(Some(CfgDraft {
        site: proof.region.id.clone(),
        origin,
        origin_span: proof.source.span,
        parent_root: None,
        binding: None,
        start: cfg.node(proof.region, LoanPointPhase::Before)?,
        seeds: [cfg.node(proof.region, LoanPointPhase::After)?]
            .into_iter()
            .collect(),
        cause: LoanCause::TraversalFreeze,
    }))
}

#[cfg(test)]
mod tests;
