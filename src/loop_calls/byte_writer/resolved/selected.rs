//! Selected replay reservations replace the ordinary enclosing debit.
//! Primitive parameter admission avoids allocating nominal layout/type keys.
use super::*;
use crate::cleanup_plan::ReplayPhysicalReservation;

pub(crate) fn checked_bindings_with_reservation<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
) -> Result<HelperBindings<'a>, crate::diagnostic::Diagnostic> {
    if !potential(program) {
        return Ok(HelperBindings::default());
    }
    physical
        .reserve(std::mem::size_of::<CensusCursor<'_>>())
        .map_err(|_| census_error())?;
    let mut census = CensusCursor {
        frames: [(&function.body, 0, 0); CENSUS_DEPTH],
    };
    let mut proof: Option<ProofCursor<'a>> = None;
    checked_bindings_with_storage(
        function,
        &mut census,
        physical,
        |binding, value, physical| {
            let ResolvedExprKind::Call {
                callee,
                instance: None,
                ..
            } = &value.kind
            else {
                return Ok(false);
            };
            let Some(target) = program.resolve_call_target(callee, None) else {
                return Ok(false);
            };
            if target.return_type != ResolvedType::Bytes
                || target
                    .params
                    .iter()
                    .filter(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
                    .count()
                    != 1
                || !target
                    .params
                    .iter()
                    .all(|p| primitive_param(p.ownership, &p.ty))
            {
                return Ok(false);
            }
            if proof.is_none() {
                physical
                    .reserve(std::mem::size_of::<Option<ProofCursor<'_>>>())
                    .map_err(|_| census_error())?;
                proof = Some(ProofCursor {
                    frames: [None; DEPTH],
                });
            }
            let proved = nonallocating_with_cursor(
                target,
                |id, instance| program.resolve_call_target(id, instance),
                |_, mode, ty| primitive_param(mode, ty),
                |_, ty| crate::hir::is_scalar_resolved_type(ty),
                program
                    .functions()
                    .len()
                    .saturating_add(program.function_instances().len()),
                proof.as_mut().ok_or_else(census_error)?,
            );
            Ok(assignment_with_target(
                program, binding, value, target, proved,
            ))
        },
        |bytes, physical| physical.reserve(bytes).is_ok(),
    )
}
fn primitive_param(mode: OwnershipMode, ty: &ResolvedType) -> bool {
    match mode {
        OwnershipMode::Own => *ty == ResolvedType::Bytes,
        OwnershipMode::Value => crate::hir::is_scalar_resolved_type(ty),
        OwnershipMode::Borrow => matches!(
            ty,
            ResolvedType::Bytes | ResolvedType::String | ResolvedType::Str | ResolvedType::SliceU8
        ),
        OwnershipMode::Shared => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
    #[test]
    fn selected_helper_proof_reserves_exact_backing_without_double_enclosing_debit() {
        let checked = crate::check(SOURCE, "physical-helper.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let main = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let legacy = checked_bindings(&program, main).unwrap();
        let ((selected, charged), refused, enclosing) =
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                let mut physical = ReplayPhysicalReservation::new(usize::MAX);
                let sites =
                    checked_bindings_with_reservation(&program, main, &mut physical).unwrap();
                (sites, physical.charged())
            });
        assert!(!refused);
        assert_eq!(
            charged, enclosing,
            "one enclosing debit per selected reservation"
        );
        assert_eq!(
            selected
                .into_iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>(),
            legacy
                .into_iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>()
        );
        let mut exact = ReplayPhysicalReservation::new(charged);
        assert_eq!(
            checked_bindings_with_reservation(&program, main, &mut exact)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(exact.charged(), charged);
        let mut short = ReplayPhysicalReservation::new(charged - 1);
        assert_eq!(
            checked_bindings_with_reservation(&program, main, &mut short)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
        assert!(
            short.reserve(0).is_err(),
            "the first refusal remains sticky"
        );
    }
    #[test]
    fn wide_helper_cursor_retains_late_constructor_refusal() {
        let mut source = String::from(
            "module cursor.helper; @id(\"cursor.fill\") fn fill(output:own Bytes)->Bytes {",
        );
        for index in 0..32 {
            source.push_str(&format!("let value_{index}={index};"));
        }
        source.push_str("bytes_set(output,0usize,65u8)} @id(\"app.main\") fn main()->i64 {0}");
        let checked = crate::check(&source, "helper-proof-cursor.spx").unwrap();
        let mut program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "cursor.fill")
            .unwrap();
        assert!(super::super::nonallocating(&program, function));
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "cursor.fill")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("helper body remains a block")
        };
        let crate::hir::ResolvedStatement::Let { value, .. } = statements.last_mut().unwrap()
        else {
            panic!("last copied scalar remains a let")
        };
        value.kind = ResolvedExprKind::String("late allocation".to_owned());
        value.ty = ResolvedType::String;
        value.ownership = OwnershipMode::Own;
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "cursor.fill")
            .unwrap();
        assert!(!super::super::nonallocating(&program, function));
    }

    #[test]
    fn absent_helper_signature_has_no_selected_cursor_or_backing_charge() {
        let source = "module old; @id(\"old.main\") fn main()->i64 {42}";
        let checked = crate::check(source, "no-helper.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut physical = ReplayPhysicalReservation::new(0);
        let sites =
            checked_bindings_with_reservation(&program, &program.functions[0], &mut physical)
                .unwrap();
        assert!(sites.is_empty());
        assert_eq!(physical.charged(), 0);
        assert!(physical.reserve(0).is_ok());
    }
}
