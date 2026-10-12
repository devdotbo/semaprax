//! Complete pre-change paid native inventory and match-parent receivers.
use super::*;

pub(super) fn inventory_aliases_paid(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    aliases: &mut BTreeMap<ValueId, Place>,
    payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
) -> Result<(), Diagnostic> {
    let ResolvedExprKind::Match {
        mode: crate::hir::ResolvedMatchMode::Borrow,
        scrutinee,
        arms,
    } = &expression.kind
    else {
        return Ok(());
    };
    let Some(origin) = storage::expression_place(scrutinee, payer)? else {
        return Ok(());
    };
    for arm in arms {
        let ResolvedMatchPattern::Variant { case, fields, .. } = &arm.pattern else {
            continue;
        };
        for field in fields {
            let ResolvedType::Nominal {
                declaration,
                arguments,
            } = &field.binding.ty
            else {
                continue;
            };
            if field.binding.ownership != OwnershipMode::Borrow || !arguments.is_empty()
                || !program.interfaces.iter().flat_map(|interface| &interface.imports).any(|import|
                    matches!(&import.result.kind, crate::hir::ResolvedImportResultKind::BorrowedStr { resource } if resource == declaration)) { continue; }
            let mut place = storage::clone_place(&origin, payer)?;
            storage::push(
                &mut place.projections,
                PlaceProjection::VariantField {
                    case: case.clone(),
                    field: field.field.clone(),
                },
                payer,
            )?;
            storage::map_insert(aliases, field.binding.id.clone(), place, payer)?;
        }
    }
    Ok(())
}

pub(super) fn match_parents_paid(
    expressions: &[&ResolvedExpr],
    drafts: &[CfgDraft],
    payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
) -> Result<BTreeMap<ValueId, LoanId>, Diagnostic> {
    let mut sites = BTreeMap::new();
    for (index, draft) in drafts.iter().enumerate() {
        storage::map_insert(
            &mut sites,
            (draft.site.clone(), draft.cause.clone()),
            LoanId(index as u16),
            payer,
        )?;
    }
    let mut parents = BTreeMap::new();
    for expression in expressions {
        let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            arms,
            ..
        } = &expression.kind
        else {
            continue;
        };
        for (arm_index, arm) in arms.iter().enumerate() {
            let ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern else {
                continue;
            };
            let Some(id) = sites.get(&(
                expression.id.clone(),
                LoanCause::MatchBorrow {
                    arm: arm_index as u16,
                },
            )) else {
                continue;
            };
            for field in fields {
                if field.binding.ownership == OwnershipMode::Borrow {
                    storage::map_insert(&mut parents, field.binding.id.clone(), *id, payer)?;
                }
            }
        }
    }
    Ok(parents)
}
