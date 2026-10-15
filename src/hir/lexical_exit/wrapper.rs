//! Complete source-For wrapper replay; spelling alone supplies no authority.
use super::validate_latch;
use crate::diagnostic::Diagnostic;
use crate::hir::*;
pub(crate) struct SourceForWrapper<'a> {
    pub(crate) authored_body: &'a ResolvedExpr,
    /// A consuming traversal's hidden Step must settle before leaving this region.
    pub(crate) iterator_step: Option<&'a ResolvedBinding>,
}
fn fail() -> Diagnostic {
    Diagnostic::io(
        "SPX-H006",
        "lexical source-for wrapper has inconsistent seed, carrier, counter or latch",
    )
}
fn zero(value: &ResolvedExpr) -> bool {
    value.ownership == OwnershipMode::Value
        && matches!(
            value.kind,
            ResolvedExprKind::Int(0) | ResolvedExprKind::Usize(0)
        )
}
fn place(value: &ResolvedExpr, binding: &ResolvedBinding) -> bool {
    value.ty == binding.ty
        && value.ownership == binding.ownership
        && matches!(&value.kind,ResolvedExprKind::Place(p) if p.root==binding.id && p.projections.is_empty())
}
pub(crate) fn source_for_wrapper<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    region: &'a ResolvedExpr,
) -> Result<Option<SourceForWrapper<'a>>, Diagnostic> {
    source_for_wrapper_with(program, region, &mut |program, region| {
        crate::loan_plan::traversal::inspect(program, region).map(|proof| proof.is_some())
    })
}

pub(crate) fn source_for_wrapper_with<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    region: &'a ResolvedExpr,
    inspect: &mut dyn FnMut(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedExpr,
    ) -> Result<bool, Diagnostic>,
) -> Result<Option<SourceForWrapper<'a>>, Diagnostic> {
    source_for_wrapper_with_bytes(program, region, None, inspect)
}

pub(crate) fn source_for_wrapper_with_bytes<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    region: &'a ResolvedExpr,
    byte_census: Option<(&crate::hir::ByteTraversalCensus<'a>, &'a ResolvedFunction)>,
    inspect: &mut dyn FnMut(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedExpr,
    ) -> Result<bool, Diagnostic>,
) -> Result<Option<SourceForWrapper<'a>>, Diagnostic> {
    let ResolvedExprKind::Block { statements, tail } = &region.kind else {
        return Ok(None);
    };
    let Some(ResolvedStatement::LexicalWhile {
        condition,
        body,
        latch,
        ..
    }) = statements.last()
    else {
        return Ok(None);
    };
    if !matches!(
        latch,
        LoopLatchKind::SourceForStep { .. } | LoopLatchKind::OwnedIteratorStep { .. }
    ) {
        return Ok(None);
    }
    validate_latch(condition, body, latch)?;
    if !zero(tail) || region.ty != tail.ty || region.ownership != OwnershipMode::Value {
        return Err(fail());
    }
    if let LoopLatchKind::OwnedIteratorStep { .. } = latch {
        let proof = crate::hir::iterator_loop::recognize(condition, body).ok_or_else(fail)?;
        let [ResolvedStatement::Let {
            binding: step,
            mutable: true,
            value: seed,
            ..
        }, ResolvedStatement::LexicalWhile { .. }] = statements.as_slice()
        else {
            return Err(fail());
        };
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance: None,
            args,
        } = &seed.kind
        else {
            return Err(fail());
        };
        let [element] = type_arguments.as_slice() else {
            return Err(fail());
        };
        let [source] = args.as_slice() else {
            return Err(fail());
        };
        if step != proof.step
            || callee.as_str() != crate::iterator_ops::NEXT_ID
            || seed.ty != step.ty
            || seed.ownership != OwnershipMode::Own
            || source.ownership != OwnershipMode::Own
            || !nominal_matches(&source.ty, crate::iterator_ops::ITER_ID, element)
            || !nominal_matches(&step.ty, crate::iterator_ops::STEP_ID, element)
        {
            return Err(fail());
        }
        return Ok(Some(SourceForWrapper {
            authored_body: proof.authored_body,
            iterator_step: Some(step),
        }));
    }
    if let LoopLatchKind::SourceForStep {
        kind: SourceForKind::BorrowBytes,
        ..
    } = latch
    {
        let proof = if let Some((census, function)) = byte_census {
            census.region(program, function, region)?
        } else {
            crate::hir::validation::byte_traversal::region(region)?
        }
        .ok_or_else(fail)?;
        return Ok(Some(SourceForWrapper {
            authored_body: proof.authored_body,
            iterator_step: None,
        }));
    }
    let [ResolvedStatement::Let {
        binding: length,
        mutable: false,
        value: count,
        ..
    }, ResolvedStatement::Let {
        binding: counter,
        mutable: true,
        value: init,
        ..
    }, ResolvedStatement::LexicalWhile { .. }] = statements.as_slice()
    else {
        return Err(fail());
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &count.kind
    else {
        return Err(fail());
    };
    let [element] = type_arguments.as_slice() else {
        return Err(fail());
    };
    let [source] = args.as_slice() else {
        return Err(fail());
    };
    let ResolvedExprKind::Binary { right, .. } = &condition.kind else {
        return Err(fail());
    };
    if length.ty != ResolvedType::Usize
        || length.ownership != OwnershipMode::Value
        || counter.ty != ResolvedType::Usize
        || counter.ownership != OwnershipMode::Value
        || !place(right, length)
        || !zero(init)
        || init.ty != ResolvedType::Usize
        || count.ty != length.ty
        || count.ownership != OwnershipMode::Value
        || callee.as_str() != crate::vec_ops::LEN_ID
        || !crate::vec_ops::resolved_vec_matches(&source.ty, element)
        || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || !matches!(&source.kind,ResolvedExprKind::Place(p) if p.projections.is_empty())
    {
        return Err(fail());
    }
    let ResolvedExprKind::Block {
        statements: body_statements,
        ..
    } = &body.kind
    else {
        return Err(fail());
    };
    let ResolvedStatement::Let { value: read, .. } = &body_statements[0] else {
        return Err(fail());
    };
    let ResolvedStatement::Let {
        value: authored_body,
        ..
    } = &body_statements[1]
    else {
        return Err(fail());
    };
    match latch {
        LoopLatchKind::SourceForStep {
            kind: SourceForKind::Copy,
            ..
        } => {
            let ResolvedExprKind::Call {
                type_arguments,
                args,
                ..
            } = &read.kind
            else {
                return Err(fail());
            };
            if type_arguments.as_slice() != std::slice::from_ref(element)
                || args.first() != Some(source)
            {
                // Identity differs at each source site: compare exact place/type/mode, not expression ID.
                let Some(owner) = args.first() else {
                    return Err(fail());
                };
                if type_arguments.as_slice() != std::slice::from_ref(element)
                    || owner.ty != source.ty
                    || owner.ownership != source.ownership
                    || !matches!((&owner.kind,&source.kind),(ResolvedExprKind::Place(a),ResolvedExprKind::Place(b)) if a==b)
                {
                    return Err(fail());
                }
            }
        }
        LoopLatchKind::SourceForStep {
            kind: SourceForKind::BorrowRecord,
            ..
        } => {
            if !inspect(program, region)? {
                return Err(fail());
            }
        }
        _ => return Err(fail()),
    }
    Ok(Some(SourceForWrapper {
        authored_body,
        iterator_step: None,
    }))
}

fn nominal_matches(ty: &ResolvedType, id: &str, element: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == id && arguments.as_slice() == std::slice::from_ref(element))
}
