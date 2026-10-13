//! Selected replay reservations replace the ordinary enclosing debit.
//! Primitive parameter admission avoids allocating nominal layout/type keys.
use super::*;
use crate::cleanup_plan::ReplayPhysicalReservation;

#[cfg(test)]
pub(crate) fn checked_bindings_with_reservation<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
) -> Result<HelperBindings<'a>, crate::diagnostic::Diagnostic> {
    checked_bindings_with_reservation_and_work(program, function, physical, &mut |_| Ok(()))
}
pub(crate) fn checked_bindings_with_reservation_and_work<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    debit: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<HelperBindings<'a>, crate::diagnostic::Diagnostic> {
    let mut possible = false;
    for ordinal in 0..program.function_count() {
        debit(1)?;
        let member = program.function_at(ordinal).ok_or_else(census_error)?;
        if member.return_type == ResolvedType::Bytes {
            for parameter in &member.params {
                debit(1)?;
                if parameter.ownership == OwnershipMode::Own && parameter.ty == ResolvedType::Bytes
                {
                    possible = true;
                    break;
                }
            }
        }
        if possible {
            break;
        }
    }
    if !possible {
        return Ok(HelperBindings::default());
    }
    physical
        .reserve(std::mem::size_of::<CensusCursor<'_>>())
        .map_err(|_| census_error())?;
    let mut census = CensusCursor {
        frames: [(&function.body, 0, 0); CENSUS_DEPTH],
    };
    let mut proof: Option<ProofCursor<'a>> = None;
    let mut storage = (physical, debit);
    checked_bindings_with_storage(
        function,
        &mut census,
        &mut storage,
        |binding, value, storage| {
            let (physical, debit) = storage;
            debit(program.function_count())?;
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
            let proved = nonallocating_with_cursor_and_work(
                target,
                |id, instance| program.resolve_call_target(id, instance),
                |_, mode, ty| primitive_param(mode, ty),
                |_, ty| crate::hir::is_scalar_resolved_type(ty),
                program
                    .function_count()
                    .saturating_add(program.function_instances().len()),
                proof.as_mut().ok_or_else(census_error)?,
                *debit,
            )?;
            Ok(assignment_with_target(
                program, binding, value, target, proved,
            ))
        },
        |bytes, storage| storage.0.reserve(bytes).is_ok(),
        |units, storage| (storage.1)(units),
        |_| {},
        census_error,
    )
}
/// Guarded receiving proof for a complete nonowning helper profile. The
/// original potential short circuit, body census, local work and depth limits
/// still run. This receiver cannot mint a writer site or accept an owning Bytes
/// assignment; a caller must independently authenticate its entire shape.
#[cfg(test)]
pub(crate) fn checked_absent_with_work_reservation<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    debit: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<HelperBindings<'a>, crate::diagnostic::Diagnostic> {
    // Mirror potential()'s evaluated return-type and parameter probes exactly.
    // No declaration names, cloned type keys or alternate lookup universe.
    let mut possible = false;
    for member in program.functions() {
        debit(1)?;
        if member.return_type == ResolvedType::Bytes {
            for parameter in &member.params {
                debit(1)?;
                if parameter.ownership == OwnershipMode::Own && parameter.ty == ResolvedType::Bytes
                {
                    possible = true;
                    break;
                }
            }
        }
        if possible {
            break;
        }
    }
    if !possible {
        return Ok(HelperBindings::default());
    }
    physical
        .reserve(std::mem::size_of::<CensusCursor<'_>>())
        .map_err(|_| census_error())?;
    let mut cursor = CensusCursor {
        frames: [(&function.body, 0, 0); CENSUS_DEPTH],
    };
    checked_bindings_with_storage(
        function,
        &mut cursor,
        physical,
        |binding, _, _| {
            if binding.ty == ResolvedType::Bytes && binding.ownership == OwnershipMode::Own {
                return Err(census_error());
            }
            Ok(false)
        },
        |bytes, physical| physical.reserve(bytes).is_ok(),
        |units, _| debit(units),
        |_| {},
        census_error,
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
            "module cursor.helper; @id(\"cursor.fill\") fn fill(output:own Bytes)->Bytes requires 0usize<byte_len(bytes_as_slice(output)) {",
        );
        for index in 0..32 {
            source.push_str(&format!("let value_{index}={index};"));
        }
        source.push_str("let mut next=output; next=bytes_set(next,0usize,65u8); next} @id(\"app.main\") fn main()->i64 {0}");
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
        let crate::hir::ResolvedStatement::Let { value, .. } = statements.iter_mut().find(|statement| matches!(statement, crate::hir::ResolvedStatement::Let { binding, .. } if binding.name == "value_31")).unwrap()
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
    fn copy_census_program(width: usize) -> crate::hir::ResolvedProgram {
        let statements = (0..width)
            .map(|index| format!("let n{index}=0;"))
            .collect::<String>();
        let source = format!(
            "module census; @id(\"probe\") fn probe()->i64 {{{statements}0}} \
             @id(\"writer\") fn writer(value:own Bytes)->Bytes {{value}} \
             @id(\"main\") fn main()->i64 {{0}}"
        );
        crate::hir::resolve(&crate::check(&source, "copy-writer-census.spx").unwrap()).unwrap()
    }
    fn prepared_census(
        program: &crate::hir::ResolvedProgram,
        calls: usize,
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), &'static str>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let answer = (|| {
                let function = &program.functions[0];
                let mut preparation = crate::cleanup_plan::SelectedReplayPreparation::with_limits(
                    function, work, bytes,
                )?;
                for _ in 0..calls {
                    // The typed constructor debit is Copy; this legacy census
                    // owner keeps its Diagnostic contract through a test-only
                    // explicit rendering of the refused debit.
                    let result = preparation.construct(function, |physical, debit| {
                        let mut legacy = |units| {
                            debit(units).map_err(|refused| refused.into_legacy_diagnostic())
                        };
                        checked_absent_with_work_reservation(
                            program,
                            function,
                            physical,
                            &mut legacy,
                        )
                        .map_err(|_| {
                            crate::cleanup_plan::SelectedReplayRefused::constructor(
                                "writer census refused",
                            )
                        })
                    });
                    match result {
                        Ok(sites) => assert!(sites.is_empty()),
                        Err(refused) => {
                            let remaining = preparation.remaining();
                            let sticky = preparation.charge(function, 1, "late census retry");
                            assert_eq!(sticky.unwrap_err().code, "SPX-H006");
                            assert_eq!(preparation.remaining(), remaining);
                            return Err(refused.into_legacy_diagnostic());
                        }
                    }
                }
                let charged = preparation
                    .construct(function, |physical, _| Ok(physical.charged()))
                    .map_err(|refused| refused.into_legacy_diagnostic())?;
                Ok::<_, crate::diagnostic::Diagnostic>((work - preparation.remaining(), charged))
            })();
            match answer {
                Ok(receipt) => Ok(receipt),
                Err(diagnostic) => {
                    assert_eq!(diagnostic.code, "SPX-H006");
                    drop(diagnostic);
                    Err("SPX-H006")
                }
            }
        })
    }
    #[test]
    fn nonowning_writer_absence_preserves_exact_census_probes_and_old_verdict() {
        for width in [0, 1, 32, 512] {
            let program = copy_census_program(width);
            let legacy = checked_bindings(&program, &program.functions[0]).unwrap();
            assert!(legacy.is_empty());
            // Three potential probes (probe, writer, its Own Bytes parameter),
            // 2N+3 cursor frames, and N structural statement probes. This is
            // independent of the callback's measured successful debit count.
            let expected = 3 * width + 6;
            let (wide, allocation) = prepared_census(&program, 1, expected, 1 << 20);
            let (spent, bytes) = wide.unwrap();
            assert_eq!(spent, expected);
            assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
            let (exact, allocation) = prepared_census(&program, 1, expected, bytes);
            assert_eq!(exact.unwrap(), (spent, bytes));
            assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
            for (work, backing) in [(expected - 1, bytes), (expected, bytes - 1)] {
                let (short, allocation) = prepared_census(&program, 1, work, backing);
                assert_eq!(short, Err("SPX-H006"));
                assert!(!allocation.invalid && allocation.live == 0);
            }
        }
    }
    #[test]
    fn later_writer_absence_census_spends_one_preparation_and_failure_stays_sticky() {
        let program = copy_census_program(64);
        let expected = 2 * (3 * 64 + 6);
        let (wide, allocation) = prepared_census(&program, 2, expected, 1 << 20);
        let (spent, bytes) = wide.unwrap();
        assert_eq!(spent, expected);
        assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
        let (exact, allocation) = prepared_census(&program, 2, expected, bytes);
        assert_eq!(exact.unwrap(), (spent, bytes));
        assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
        for (work, backing) in [(expected - 1, bytes), (expected, bytes - 1)] {
            let (short, allocation) = prepared_census(&program, 2, work, backing);
            assert_eq!(short, Err("SPX-H006"));
            assert!(!allocation.invalid && allocation.live == 0);
        }
    }
}
