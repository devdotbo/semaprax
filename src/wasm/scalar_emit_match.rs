//! Scalar match/guard/lazy-operand emission retains exact lexical depth.
use super::*;

/// Refutable Match v1 core-lane lowering. The scrutinee evaluates once into
/// its dedicated staging local; every non-final arm nests one reject block
/// whose `br_if 0` falls through to the following arms, and a selected arm's
/// value branches out of the result-carrying `$done` block. A guard is an
/// ordinary emitted bool expression short-circuited after the pattern test,
/// so it evaluates exactly once per reached matching arm.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_scalar_refutable_match(
    output: &mut impl ByteOutput,
    expr: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[crate::hir::ResolvedMatchArm],
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout<'_>,
    result: Option<(u32, &str)>,
    control: &mut scalar_control::Control<'_>,
) -> Result<(), Diagnostic> {
    let scratch = layout
        .match_scratch
        .get(expr.id.as_str())
        .copied()
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-W108",
                format!("missing WebAssembly local layout for match `{}`", expr.id),
            )
        })?;
    // One evaluation: emit the scrutinee, then store it in the staging local.
    scalar_emit::emit_with_control(
        output,
        scrutinee,
        value_indexes,
        function_indexes,
        layout,
        result,
        control,
    )?;
    output.push(0x21);
    write_u32(output, scratch);

    let result_type = if expr.ty == ResolvedType::Never {
        0x40
    } else {
        wasm_type(&expr.ty)?
    };
    // block $done (result T)
    output.push(0x02);
    output.push(result_type);
    control.depth += 1;
    for (index, arm) in arms.iter().enumerate() {
        let final_arm = index + 1 == arms.len();
        if !final_arm {
            // block $reject (void)
            output.extend_bytes(&[0x02, 0x40]);
            control.depth += 1;
            emit_pattern_test(output, scratch, &scrutinee.ty, &arm.pattern)?;
            output.push(0x45); // i32.eqz
            output.extend_bytes(&[0x0d, 0x00]); // br_if 0 -> next arm
        }
        if let Some(guard) = &arm.guard {
            debug_assert!(matches!(guard.ty, ResolvedType::Bool));
            scalar_emit::emit_with_control(
                output,
                guard.as_ref(),
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
            output.push(0x45); // i32.eqz: false guard falls through
            output.extend_bytes(&[0x0d, 0x00]); // br_if 0 -> next arm
        }
        scalar_emit::emit_with_control(
            output,
            &arm.value,
            value_indexes,
            function_indexes,
            layout,
            result,
            control,
        )?;
        if !final_arm {
            // Selected: exit $done carrying the arm value. Labels from here
            // are [reject_i(0), $done(1)] because earlier reject blocks were
            // closed; the trailing catch-all falls out of $done naturally.
            output.extend_bytes(&[0x0c, 0x01]); // br 1
            output.push(0x0b); // end reject block
            control.depth -= 1;
        }
    }
    output.push(0x0b); // end $done
    control.depth -= 1;
    Ok(())
}

/// Pushes an i32 truth value for one scalar pattern: `local.get` then an
/// equality chain over the literal alternatives joined with `i32.or`.
fn emit_pattern_test(
    output: &mut impl ByteOutput,
    scratch: u32,
    scrutinee_ty: &ResolvedType,
    pattern: &crate::hir::ResolvedMatchPattern,
) -> Result<(), Diagnostic> {
    let alternatives: &[crate::hir::PatternValue] = match pattern {
        crate::hir::ResolvedMatchPattern::Literal(value) => std::slice::from_ref(value),
        crate::hir::ResolvedMatchPattern::Or(alternatives) => {
            let mut flattened = Vec::with_capacity(alternatives.len());
            for alternative in alternatives {
                match alternative {
                    crate::hir::ResolvedMatchPattern::Literal(value) => {
                        flattened.push(*value);
                    }
                    _ => {
                        return Err(Diagnostic::io(
                            "SPX-M105",
                            "or-pattern alternatives must be literals",
                        ))
                    }
                }
            }
            return emit_alternative_tests(output, scratch, scrutinee_ty, &flattened);
        }
        // Refutable Match v1: an irrefutable pattern with a guard tests as
        // constant true; the guard decides. Unguarded irrefutable arms only
        // occur as the trailing catch-all, which emits no test.
        crate::hir::ResolvedMatchPattern::Wildcard
        | crate::hir::ResolvedMatchPattern::Binding(_) => {
            output.extend_bytes(&[0x41, 0x01]); // i32.const 1
            return Ok(());
        }
        crate::hir::ResolvedMatchPattern::Variant { .. }
        | crate::hir::ResolvedMatchPattern::Record { .. } => {
            return Err(Diagnostic::io(
                "SPX-W110",
                "aggregate arm reached scalar match lowering",
            ))
        }
    };
    emit_alternative_tests(output, scratch, scrutinee_ty, alternatives)
}

fn emit_alternative_tests(
    output: &mut impl ByteOutput,
    scratch: u32,
    scrutinee_ty: &ResolvedType,
    alternatives: &[crate::hir::PatternValue],
) -> Result<(), Diagnostic> {
    for (position, value) in alternatives.iter().enumerate() {
        output.push(0x20); // local.get scratch
        write_u32(output, scratch);
        match (scrutinee_ty, value) {
            (ResolvedType::I64, crate::hir::PatternValue::Int(inner)) => {
                output.push(0x42); // i64.const
                write_i64(output, *inner);
                output.push(0x51); // i64.eq -> i32
            }
            (ResolvedType::I32, crate::hir::PatternValue::Int32(inner)) => {
                output.push(0x41); // i32.const
                write_i64(output, i64::from(*inner));
                output.push(0x46); // i32.eq
            }
            (ResolvedType::U8, crate::hir::PatternValue::Uint8(inner)) => {
                output.push(0x41);
                write_i64(output, i64::from(*inner));
                output.push(0x46);
            }
            (ResolvedType::Usize, crate::hir::PatternValue::Usize(inner)) => {
                // `usize` rides the i64 valtype, matching the literal
                // expression encoding and the aggregate lane's own test.
                output.push(0x42); // i64.const
                write_i64(output, *inner as i64);
                output.push(0x51); // i64.eq -> i32
            }
            (ResolvedType::Char, crate::hir::PatternValue::Char(inner)) => {
                output.push(0x41);
                write_i64(output, i64::from(*inner));
                output.push(0x46);
            }
            (ResolvedType::Bool, crate::hir::PatternValue::Bool(inner)) => {
                output.push(0x41);
                write_i64(output, i64::from(*inner));
                output.push(0x46);
            }
            _ => {
                return Err(Diagnostic::io(
                    "SPX-T255",
                    format!(
                        "literal pattern disagrees with its `{}` scrutinee",
                        scrutinee_ty.identity_key()
                    ),
                ));
            }
        }
        // Both equality flags are now stacked; join them for alternatives
        // after the first.
        if position != 0 {
            output.push(0x72); // i32.or
        }
    }
    Ok(())
}

// Match the scalar emitter's explicit lowering inputs; the shared mutable
// control depth must remain visible while only the selected operand executes.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_short_circuit(
    output: &mut impl ByteOutput,
    op: BinaryOp,
    right: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
    control: &mut scalar_control::Control<'_>,
) -> Result<(), Diagnostic> {
    output.push(0x04);
    output.push(I32);
    control.depth += 1;
    if op == BinaryOp::And {
        scalar_emit::emit_with_control(
            output,
            right,
            value_indexes,
            function_indexes,
            layout,
            result,
            control,
        )?;
        output.push(0x05);
        output.extend_bytes(&[0x41, 0x00]);
    } else {
        output.extend_bytes(&[0x41, 0x01]);
        output.push(0x05);
        scalar_emit::emit_with_control(
            output,
            right,
            value_indexes,
            function_indexes,
            layout,
            result,
            control,
        )?;
    }
    output.push(0x0b);
    control.depth -= 1;
    Ok(())
}
