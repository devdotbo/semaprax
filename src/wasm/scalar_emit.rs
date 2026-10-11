//! Scalar expressions with explicit lexical label state; no bottom storage.
use super::scalar_emit_match::{emit_scalar_refutable_match, emit_short_circuit};
use super::*;

pub(super) fn emit_expr(
    output: &mut impl ByteOutput,
    expr: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
) -> Result<(), Diagnostic> {
    emit_with_control(
        output,
        expr,
        value_indexes,
        function_indexes,
        layout,
        result,
        &mut scalar_control::Control::legacy(),
    )
}
pub(super) fn emit_with_control(
    output: &mut impl ByteOutput,
    expr: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
    control: &mut super::scalar_control::Control<'_>,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        } => {
            return control.emit_exit(
                output,
                expr,
                *kind,
                target,
                value.as_deref(),
                value_indexes,
                function_indexes,
                layout,
                result,
            );
        }
        ResolvedExprKind::Int(value) => {
            output.push(0x42);
            write_i64(output, *value);
        }
        ResolvedExprKind::Int32(value) => {
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::Char(value) => {
            // Chars ride the i32 valtype; scalar values are below 2^31 so the
            // signed LEB128 encoding is exact.
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::Uint8(value) => {
            // u8 values ride the i32 valtype with the same exact encoding.
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::Usize(value) => {
            output.push(0x42);
            write_i64(output, *value as i64);
        }
        ResolvedExprKind::Float32(bits) => {
            output.push(0x43);
            output.extend_bytes(&bits.to_le_bytes());
        }
        ResolvedExprKind::Float64(bits) => {
            output.push(0x44);
            output.extend_bytes(&bits.to_le_bytes());
        }
        ResolvedExprKind::Bool(value) => {
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::VecConstructor { .. } => {
            return Err(Diagnostic::io(
                "SPX-W116",
                "Vec constructors require the aggregate collection profile",
            ))
        }
        ResolvedExprKind::BorrowedUtf8Literal(_) => {
            return Err(Diagnostic::io(
                "SPX-W116",
                "static borrowed UTF-8 requires the aggregate memory profile",
            ));
        }
        ResolvedExprKind::String(value) => {
            // A string value is an abstract host handle riding the i64 lane;
            // the handle is freshly allocated for this evaluation.
            let Some(table) = layout.string_data else {
                return Err(Diagnostic::io(
                    "SPX-W116",
                    "string literal reached lowering without string admission",
                ));
            };
            let Some(&offset) = table.offsets.get(value) else {
                return Err(Diagnostic::io(
                    "SPX-W116",
                    "string literal has no data-segment offset",
                ));
            };
            output.push(0x41); // i32.const offset
            write_u32(output, offset);
            output.push(0x41); // i32.const len
            write_i64(output, value.len() as i64);
            call_import(output, STRING_IMPORT_BASE_NEW);
        }
        ResolvedExprKind::Place(place) => {
            if !place.projections.is_empty() {
                return Err(Diagnostic::io(
                    "SPX-W110",
                    "aggregate place projections are not supported by the Wasm core backend",
                ));
            }
            let index = value_indexes.get(&place.root).copied().or_else(|| {
                result.and_then(|(index, result_id)| {
                    (place.root.as_str() == result_id).then_some(index)
                })
            });
            let index = index.ok_or_else(|| {
                Diagnostic::io(
                    "SPX-W103",
                    format!("unknown value identity `{}`", place.root),
                )
            })?;
            output.push(0x20);
            write_u32(output, index);
            // Every read of an owned string place clones the handle so the
            // source place keeps its unique owner.
            if matches!(expr.ty, ResolvedType::String) {
                call_import(output, STRING_IMPORT_BASE_CLONE);
            }
        }
        ResolvedExprKind::Closure { .. } => {
            return Err(Diagnostic::io(
                "SPX-W104",
                "capturing closure requires the Wasm closure profile",
            ))
        }
        ResolvedExprKind::FunctionReference { target } => {
            output.push(0x41);
            write_i32(
                output,
                i32::try_from(*layout.function_tables.get(target).ok_or_else(|| {
                    Diagnostic::io(
                        "SPX-W104",
                        "function reference has no WebAssembly table slot",
                    )
                })?)
                .map_err(|_| {
                    Diagnostic::io("SPX-W104", "function table index exceeds signed i32")
                })?,
            );
        }
        ResolvedExprKind::Invoke { callable, args } => {
            hir::function_value::validate_invocation(expr)?;
            let scratch = *layout
                .function_scratch
                .get(expr.id.as_str())
                .ok_or_else(|| {
                    Diagnostic::io(
                        "SPX-W108",
                        "function invocation has no WebAssembly scratch local",
                    )
                })?;
            emit_with_control(
                output,
                callable,
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
            local_set(output, scratch);
            for arg in args {
                emit_with_control(
                    output,
                    arg,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
            }
            local_get(output, scratch);
            output.push(0x11);
            write_u32(
                output,
                *layout
                    .function_types
                    .get(&callable.ty.identity_key())
                    .ok_or_else(|| {
                        Diagnostic::io(
                            "SPX-W104",
                            "function invocation has no WebAssembly signature type",
                        )
                    })?,
            );
            output.push(0x00);
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } => {
            if instance.is_none() {
                if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                    let helpers = layout.text_intrinsics.ok_or_else(|| {
                        Diagnostic::io(
                            "SPX-W119",
                            "borrowed `str` operation reached a non-text Wasm profile",
                        )
                    })?;
                    for arg in args {
                        emit_with_control(
                            output,
                            arg,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                    }
                    match op {
                        crate::str_ops::StrOp::ByteAt => {
                            return Err(Diagnostic::io(
                                "SPX-W119",
                                "borrowed byte indexing requires the aggregate Wasm lane",
                            ))
                        }
                        crate::str_ops::StrOp::LenBytes => {
                            output.push(0x42); // i64.const 32
                            write_i64(output, 32);
                            output.push(0x88); // i64.shr_u
                        }
                        crate::str_ops::StrOp::IsEmpty => {
                            output.push(0x42);
                            write_i64(output, 32);
                            output.push(0x88); // i64.shr_u
                            output.push(0x50); // i64.eqz
                        }
                        crate::str_ops::StrOp::StartsWith => {
                            output.push(0x10);
                            write_u32(output, helpers.starts_with);
                        }
                        crate::str_ops::StrOp::Contains => {
                            output.push(0x10);
                            write_u32(output, helpers.contains);
                        }
                    }
                    return Ok(());
                }
                if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                    if op.is_integer_conversion() {
                        emit_with_control(
                            output,
                            &args[0],
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                        return numeric_conversions::emit_scalar(output, op, layout);
                    }
                    if op.is_wasm_refused() {
                        return Err(crate::string_ops::text_toolkit_wasm_refusal(op));
                    }
                    // Compiler-owned string operations lower through their
                    // dedicated host imports; borrowed reads leave the input
                    // handle owned by the caller and concatenation hands both
                    // input handles to the host shim.
                    for arg in args {
                        emit_with_control(
                            output,
                            arg,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                    }
                    match op {
                        crate::string_ops::StringOp::I64FromU8 => output.push(0xad), // i64.extend_i32_u
                        crate::string_ops::StringOp::Len => {
                            call_import(output, STRING_OPS_IMPORT_BASE_LEN);
                        }
                        crate::string_ops::StringOp::IsEmpty => {
                            call_import(output, STRING_OPS_IMPORT_BASE_LEN);
                            output.push(0x50); // i64.eqz keeps bool results exact
                        }
                        crate::string_ops::StringOp::Concat => {
                            call_import(output, STRING_OPS_IMPORT_BASE_CONCAT);
                        }
                        crate::string_ops::StringOp::StartsWith => {
                            call_import(output, layout.string_ops_v2_base);
                        }
                        crate::string_ops::StringOp::Contains => {
                            call_import(output, layout.string_ops_v2_base + 1);
                        }
                        crate::string_ops::StringOp::LenChars => {
                            call_import(output, layout.string_ops_v2_base + 2);
                        }
                        crate::string_ops::StringOp::FromChar => {
                            call_import(output, layout.string_ops_v2_base + 3);
                        }
                        crate::string_ops::StringOp::FromI64 => {
                            call_import(output, layout.string_numeric_text_base);
                        }
                        crate::string_ops::StringOp::FromUsize => {
                            call_import(output, layout.string_numeric_text_base + 1);
                        }
                        _ => unreachable!("Text Toolkit v1 was refused above"),
                    }
                    return Ok(());
                }
            }
            for arg in args {
                emit_with_control(
                    output,
                    arg,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
            }
            output.push(0x10);
            let execution = instance.as_ref().map_or_else(
                || FunctionExecutionId::Monomorphic(callee.clone()),
                |instance| FunctionExecutionId::Generic(instance.clone()),
            );
            write_u32(
                output,
                *function_indexes.get(&execution).ok_or_else(|| {
                    Diagnostic::io("SPX-W104", format!("unknown function identity `{callee}`"))
                })?,
            );
        }
        ResolvedExprKind::NativeRustImportCall(_) => {
            return Err(Diagnostic::io(
                "SPX-W114",
                "Native Rust imports are unavailable for WebAssembly targets",
            ));
        }
        ResolvedExprKind::HostCommandCall(_) => {
            return Err(Diagnostic::io(
                "SPX-W114",
                "command I/O operations require the Language Command I/O v1 WebAssembly lane",
            ));
        }
        ResolvedExprKind::Unary { op, value } => match op {
            UnaryOp::Neg => {
                if value.ty == ResolvedType::I32 {
                    emit_with_control(
                        output,
                        value,
                        value_indexes,
                        function_indexes,
                        layout,
                        result,
                        control,
                    )?;
                    let [wide, _] = layout.wide_scratch;
                    output.push(0xac);
                    local_set(output, wide);
                    local_get(output, wide);
                    output.push(0xa7);
                    output.push(0x41);
                    write_i64(output, i32::MIN as i64);
                    output.push(0x46);
                    emit_arithmetic_failure_if(output, aggregate::STATUS_NEG_OVERFLOW);
                    output.push(0x42);
                    write_i64(output, 0);
                    local_get(output, wide);
                    output.push(0x7d);
                    output.push(0xa7);
                    return Ok(());
                }
                emit_with_control(
                    output,
                    value,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
                if matches!(value.ty, ResolvedType::F32 | ResolvedType::F64) {
                    // IEEE-754 negation is total; no failure import is used.
                    output.push(match value.ty {
                        ResolvedType::F32 => 0x8c,
                        _ => 0x9a,
                    });
                } else {
                    output.push(0x10);
                    write_u32(output, 5);
                }
            }
            UnaryOp::Not => {
                emit_with_control(
                    output,
                    value,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
                output.push(0x45);
            }
        },
        ResolvedExprKind::Binary { op, left, right } => {
            if left.ty == ResolvedType::Str && matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                let helper = layout
                    .text_intrinsics
                    .and_then(|indexes| indexes.equality)
                    .ok_or_else(|| {
                        Diagnostic::io("SPX-W119", "Str equality reached a non-text Wasm profile")
                    })?;
                if right.ty != ResolvedType::Str || expr.ty != ResolvedType::Bool {
                    return Err(Diagnostic::io("SPX-W119", "Str equality types disagree"));
                }
                emit_with_control(
                    output,
                    left,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
                emit_with_control(
                    output,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
                output.push(0x10);
                write_u32(output, helper);
                if *op == BinaryOp::Ne {
                    output.push(0x45);
                }
                return Ok(());
            }
            if matches!(left.ty, ResolvedType::String) {
                // Owned strings compare by UTF-8 contents through the host
                // shim; every other operator over strings is ill-typed.
                if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                    return Err(Diagnostic::io(
                        "SPX-W116",
                        "string operands only support equality comparison",
                    ));
                }
                emit_with_control(
                    output,
                    left,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
                emit_with_control(
                    output,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
                call_import(output, STRING_IMPORT_BASE_EQ);
                if *op == BinaryOp::Ne {
                    output.push(0x45); // i32.eqz keeps bool results exact
                }
                return Ok(());
            }
            if matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            ) && left.ty == ResolvedType::I32
            {
                emit_i32_checked_binary(
                    output,
                    *op,
                    left,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                return Ok(());
            }
            emit_with_control(
                output,
                left,
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                emit_short_circuit(
                    output,
                    *op,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                )?;
                return Ok(());
            }
            emit_with_control(
                output,
                right,
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
            if matches!(left.ty, ResolvedType::F32 | ResolvedType::F64) {
                // IEEE-754 arithmetic is total and never selects a failure
                // status, so floats use native opcodes instead of the checked
                // scalar imports.
                let wide = matches!(left.ty, ResolvedType::F64);
                output.push(match (op, wide) {
                    (BinaryOp::Add, true) => 0xa0,
                    (BinaryOp::Sub, true) => 0xa1,
                    (BinaryOp::Mul, true) => 0xa2,
                    (BinaryOp::Div, true) => 0xa3,
                    (BinaryOp::Add, false) => 0x92,
                    (BinaryOp::Sub, false) => 0x93,
                    (BinaryOp::Mul, false) => 0x94,
                    (BinaryOp::Div, false) => 0x95,
                    (BinaryOp::Eq, true) => 0x61,
                    (BinaryOp::Ne, true) => 0x62,
                    (BinaryOp::Lt, true) => 0x63,
                    (BinaryOp::Gt, true) => 0x64,
                    (BinaryOp::Le, true) => 0x65,
                    (BinaryOp::Ge, true) => 0x66,
                    (BinaryOp::Eq, false) => 0x5b,
                    (BinaryOp::Ne, false) => 0x5c,
                    (BinaryOp::Lt, false) => 0x5d,
                    (BinaryOp::Gt, false) => 0x5e,
                    (BinaryOp::Le, false) => 0x5f,
                    (BinaryOp::Ge, false) => 0x60,
                    (BinaryOp::Rem, _) => {
                        return Err(Diagnostic::io(
                            "SPX-W102",
                            "floating-point remainder has no admitted Wasm lowering",
                        ));
                    }
                    _ => unreachable!("float arithmetic operation was matched above"),
                });
                return Ok(());
            }
            if matches!(left.ty, ResolvedType::U8)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                // Checked u8 arithmetic without new host imports: bounded
                // operands make the unsigned i32 opcodes exact, and one
                // unsigned range check traps on any out-of-range result. The
                // scratch locals keep live values off the stack while the
                // polymorphic trap block executes.
                let Some((left_scratch, right_scratch)) = layout.u8_scratch else {
                    return Err(Diagnostic::io(
                        "SPX-W108",
                        "missing WebAssembly local layout for checked u8 arithmetic",
                    ));
                };
                // The stack holds [left, right]; pop them in reverse so each
                // scratch local keeps its operand.
                output.push(0x21);
                write_u32(output, right_scratch);
                output.push(0x21);
                write_u32(output, left_scratch);
                if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
                    output.push(0x20);
                    write_u32(output, right_scratch);
                    output.push(0x45);
                    emit_arithmetic_failure_if(
                        output,
                        if *op == BinaryOp::Rem {
                            aggregate::STATUS_REM_ZERO
                        } else {
                            aggregate::STATUS_DIV_ZERO
                        },
                    );
                    output.push(0x20);
                    write_u32(output, left_scratch);
                    output.push(0x20);
                    write_u32(output, right_scratch);
                    output.push(if *op == BinaryOp::Div { 0x6e } else { 0x70 });
                    return Ok(());
                }
                output.push(0x20);
                write_u32(output, left_scratch);
                output.push(0x20);
                write_u32(output, right_scratch);
                output.push(match op {
                    BinaryOp::Add => 0x6a,
                    BinaryOp::Sub => 0x6b,
                    _ => 0x6c,
                });
                output.push(0x21);
                write_u32(output, left_scratch);
                output.push(0x20);
                write_u32(output, left_scratch);
                output.push(0x41);
                write_i64(output, 255);
                output.push(0x4b);
                emit_arithmetic_failure_if(
                    output,
                    match op {
                        BinaryOp::Add => aggregate::STATUS_ADD_OVERFLOW,
                        BinaryOp::Sub => aggregate::STATUS_SUB_OVERFLOW,
                        BinaryOp::Mul => aggregate::STATUS_MUL_OVERFLOW,
                        _ => unreachable!("u8 arithmetic operation was matched above"),
                    },
                );
                output.push(0x20);
                write_u32(output, left_scratch);
                return Ok(());
            }
            if matches!(left.ty, ResolvedType::Usize)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                let Some((left_scratch, right_scratch)) = layout.usize_scratch else {
                    return Err(Diagnostic::io(
                        "SPX-W108",
                        "missing WebAssembly local layout for checked usize arithmetic",
                    ));
                };
                output.push(0x21);
                write_u32(output, right_scratch);
                output.push(0x21);
                write_u32(output, left_scratch);
                match op {
                    BinaryOp::Add => {
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x7c);
                        output.push(0x22);
                        write_u32(output, right_scratch);
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x54); // i64.lt_u(result, left)
                        emit_arithmetic_failure_if(output, aggregate::STATUS_ADD_OVERFLOW);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                    }
                    BinaryOp::Sub => {
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x54); // i64.lt_u(left, right)
                        emit_arithmetic_failure_if(output, aggregate::STATUS_SUB_OVERFLOW);
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x7d);
                    }
                    BinaryOp::Mul => {
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x50); // i64.eqz
                        output.push(0x45); // i32.eqz => right != 0
                        output.extend_bytes(&[0x04, 0x40]); // if right != 0
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x42);
                        write_i64(output, -1); // UINT64_MAX bit pattern
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x80); // i64.div_u
                        output.push(0x56); // i64.gt_u
                        emit_arithmetic_failure_if(output, aggregate::STATUS_MUL_OVERFLOW);
                        output.push(0x0b); // end overflow check
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x7e);
                    }
                    BinaryOp::Div | BinaryOp::Rem => {
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x50);
                        emit_arithmetic_failure_if(
                            output,
                            if *op == BinaryOp::Div {
                                aggregate::STATUS_DIV_ZERO
                            } else {
                                aggregate::STATUS_REM_ZERO
                            },
                        );
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(if *op == BinaryOp::Div { 0x80 } else { 0x82 });
                    }
                    _ => unreachable!("usize arithmetic operation was matched above"),
                }
                return Ok(());
            }
            if matches!(left.ty, ResolvedType::Char)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                return Err(Diagnostic::io(
                    "SPX-W102",
                    "char arithmetic has no admitted Wasm lowering",
                ));
            }
            match op {
                BinaryOp::Add => call_import(output, 0),
                BinaryOp::Sub => call_import(output, 1),
                BinaryOp::Mul => call_import(output, 2),
                BinaryOp::Div => call_import(output, 3),
                BinaryOp::Rem => call_import(output, 4),
                BinaryOp::Eq | BinaryOp::Ne => {
                    output.push(match (&left.ty, op) {
                        (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Eq) => 0x51,
                        (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Ne) => 0x52,
                        (_, BinaryOp::Eq) => 0x46,
                        (_, BinaryOp::Ne) => 0x47,
                        _ => unreachable!(),
                    });
                }
                BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                    // Ordered comparison compares scalar values; chars ride
                    // the unsigned i32 opcodes while i64 keeps its lane.
                    output.push(match (&left.ty, op) {
                        (ResolvedType::I32, BinaryOp::Lt) => 0x48,
                        (ResolvedType::I32, BinaryOp::Gt) => 0x4a,
                        (ResolvedType::I32, BinaryOp::Le) => 0x4c,
                        (ResolvedType::I32, BinaryOp::Ge) => 0x4e,
                        (ResolvedType::Char, BinaryOp::Lt) => 0x49,
                        (ResolvedType::Char, BinaryOp::Gt) => 0x4b,
                        (ResolvedType::Char, BinaryOp::Le) => 0x4d,
                        (ResolvedType::Char, BinaryOp::Ge) => 0x4f,
                        (ResolvedType::U8, BinaryOp::Lt) => 0x49,
                        (ResolvedType::U8, BinaryOp::Gt) => 0x4b,
                        (ResolvedType::U8, BinaryOp::Le) => 0x4d,
                        (ResolvedType::U8, BinaryOp::Ge) => 0x4f,
                        (ResolvedType::Usize, BinaryOp::Lt) => 0x54,
                        (ResolvedType::Usize, BinaryOp::Gt) => 0x56,
                        (ResolvedType::Usize, BinaryOp::Le) => 0x58,
                        (ResolvedType::Usize, BinaryOp::Ge) => 0x5a,
                        (_, BinaryOp::Lt) => 0x53,
                        (_, BinaryOp::Gt) => 0x55,
                        (_, BinaryOp::Le) => 0x57,
                        (_, BinaryOp::Ge) => 0x59,
                        _ => unreachable!("ordered comparison was matched above"),
                    });
                }
                BinaryOp::And | BinaryOp::Or => unreachable!(),
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        emit_with_control(
                            output,
                            value,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                        let index = layout.lets.get(&binding.id).ok_or_else(|| {
                            Diagnostic::io(
                                "SPX-W108",
                                format!("missing WebAssembly local layout for `{}`", binding.id),
                            )
                        })?;
                        output.push(0x21);
                        write_u32(output, *index);
                    }
                    // The assigned value is emitted fully, then stored into
                    // the target's local with `local.set`.
                    ResolvedStatement::Assign {
                        binding,
                        value: assigned,
                        ..
                    } => {
                        emit_with_control(
                            output,
                            assigned,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                        let index = layout.lets.get(&binding.id).ok_or_else(|| {
                            Diagnostic::io(
                                "SPX-W108",
                                format!(
                                    "missing WebAssembly local layout for assignment target `{}`",
                                    binding.id
                                ),
                            )
                        })?;
                        output.push(0x21);
                        write_u32(output, *index);
                    }
                    // Unsafe boundaries are transparent: the ordinary body is
                    // emitted exactly as-is and its scalar Copy result drops.
                    ResolvedStatement::Unsafe { body, .. } => {
                        emit_with_control(
                            output,
                            body,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                        if body.ty != ResolvedType::Never {
                            output.push(0x1A);
                        }
                    }
                    // Bounded While-Loops v1 lowers to a core `block`/`loop`
                    // pair: the condition re-evaluates at the top, a false
                    // condition branches out of the enclosing block, and the
                    // discarded body value falls through to the back-edge
                    // branch. Checked-arithmetic failures inside the loop use
                    // the same host imports/traps as straight-line code.
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => {
                        let lexical = matches!(statement, ResolvedStatement::LexicalWhile { .. });
                        debug_assert!(matches!(condition.ty, ResolvedType::Bool));
                        output.extend_bytes(&[0x02, 0x40]); // block (empty)
                        output.extend_bytes(&[0x03, 0x40]); // loop (empty) $top
                        control.depth += 2;
                        if lexical {
                            control.enter_loop(condition)?;
                        }
                        emit_with_control(
                            output,
                            condition,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                        output.push(0x45); // i32.eqz
                        output.extend_bytes(&[0x0d, 0x01]); // br_if 1 -> $exit on false
                        emit_with_control(
                            output,
                            body,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            control,
                        )?;
                        if body.ty != ResolvedType::Never {
                            output.push(0x1A);
                        }
                        if lexical {
                            control.leave_loop();
                        }
                        control.depth -= 2;
                        output.extend_bytes(&[0x0c, 0x00]); // br 0 -> $top
                        output.push(0x0b); // end loop
                        output.push(0x0b); // end block
                    }
                }
            }
            emit_with_control(
                output,
                tail,
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            emit_with_control(
                output,
                condition,
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
            output.push(0x04);
            output.push(if expr.ty == ResolvedType::Never {
                0x40
            } else {
                wasm_type(&expr.ty)?
            });
            control.depth += 1;
            emit_with_control(
                output,
                then_branch,
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
            output.push(0x05);
            emit_with_control(
                output,
                else_branch,
                value_indexes,
                function_indexes,
                layout,
                result,
                control,
            )?;
            control.depth -= 1;
            output.push(0x0b);
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            // Refutable Match v1: Copy-scalar scrutinees lower to the
            // literal/guard decision chain on the core lane; aggregates keep
            // the aggregate-lane rejection below.
            if crate::hir::is_refutable_match_scalar(&scrutinee.ty) {
                return emit_scalar_refutable_match(
                    output,
                    expr,
                    scrutinee,
                    arms,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    control,
                );
            }
            return Err(Diagnostic::io(
                "SPX-W110",
                "aggregate expressions require WebAssembly aggregate lowering",
            ));
        }
        ResolvedExprKind::LiteralFormat { .. }
        | ResolvedExprKind::VecFieldRead { .. }
        | ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::ByteRange { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::UpdateRecord { .. } => {
            return Err(Diagnostic::io(
                "SPX-W110",
                "aggregate expressions require WebAssembly aggregate lowering",
            ));
        }
        // Resumable Effects v1 (issue #204): explicit, tested refusal. A
        // `yield` suspends the whole function and hands control back to a
        // caller outside the module; the Core Wasm scalar/aggregate lanes
        // have no such control-transfer mechanism, so a `yields`-declaring
        // function is not admitted for this target at all -- see
        // `docs/RESUMABLE-EFFECTS-V1.md`.
        ResolvedExprKind::Yield { .. } => {
            return Err(Diagnostic::io(
                "SPX-W126",
                "`yield` is not yet lowered by the WebAssembly backend; a `yields`-declaring \
                 function is not admitted for this target",
            ));
        }
    }
    Ok(())
}
