//! Complete expression planning without storage for bottom completions.
use super::*;

impl FunctionPlan {
    pub(super) fn collect_expr(
        &mut self,
        program: &ResolvedProgram,
        variant_layouts: &VariantLayoutCache,
        expr: &ResolvedExpr,
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        if expr.ty == ResolvedType::Never {
            // Bottom expressions still have declarations and children to plan;
            // only the expression's own storage is absent.
        } else if matches!(expr.kind, ResolvedExprKind::RowLoan { .. }) {
            // A row loan's nominal type is the lent record, but no record is
            // ever materialized: its only storage is the 16-byte descriptor
            // block and i32 local reserved by `collect_row_loans`.
        } else if is_aggregate(program, &expr.ty)? {
            let (size, align) = aggregate_size_align(program, variant_layouts, &expr.ty)?;
            let offset = frame.allocate(size, align)?;
            if self
                .aggregate_expressions
                .insert(expr.id.clone(), offset)
                .is_some()
            {
                return Err(error(format!(
                    "duplicate aggregate expression identity `{}`",
                    expr.id
                )));
            }
        } else {
            let ty = scalar_wasm_type(program, &expr.ty)?;
            let local = self.add_local(parameter_count, ty)?;
            if self
                .scalar_expressions
                .insert(expr.id.clone(), local)
                .is_some()
            {
                return Err(error(format!(
                    "duplicate scalar expression identity `{}`",
                    expr.id
                )));
            }
            if matches!(
                expr.kind,
                ResolvedExprKind::Call { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::HostCommandCall(_)
                    | ResolvedExprKind::Invoke { .. }
            ) {
                let (size, align) = if matches!(&expr.kind,ResolvedExprKind::Call{callee,..} if crate::map_ops::by_id(callee.as_str()).is_some() || crate::string_ops::by_id(callee.as_str()).and_then(map_collections::legacy_op).is_some())
                {
                    (8, 8)
                } else {
                    scalar_size_align(program, &expr.ty)?
                };
                self.call_out
                    .insert(expr.id.clone(), frame.allocate(size, align)?);
            }
        }
        if matches!(expr.kind, ResolvedExprKind::ByteRange { .. }) {
            let offset = frame.allocate(RANGE_DESCRIPTOR_SIZE, 8)?;
            if self
                .range_descriptors
                .insert(expr.id.clone(), offset)
                .is_some()
            {
                return Err(error(format!(
                    "duplicate byte-range descriptor identity `{}`",
                    expr.id
                )));
            }
        }
        if let ResolvedExprKind::Call {
            callee,
            type_arguments,
            ..
        } = &expr.kind
        {
            if (matches!(
                crate::vec_ops::by_id(callee.as_str()),
                Some(crate::vec_ops::VecOp::CloneAt)
            ) || callee.as_str() == crate::iterator_ops::NEXT_ID)
                && matches!(type_arguments.as_slice(), [element]
                    if crate::hir::owned_leaf_collection::layout(&program.declarations, element).is_some()
                        && (callee.as_str() != crate::iterator_ops::NEXT_ID
                            || !crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
                                &program.declarations, element)))
            {
                let bytes = if callee.as_str() == crate::iterator_ops::NEXT_ID {
                    88
                } else {
                    64
                };
                self.owned_leaf_scratch
                    .insert(expr.id.clone(), frame.allocate(bytes, 8)?);
            }
        }

        self.collect_vec_field_scratch(expr, frame)?;
        match &expr.kind {
            ResolvedExprKind::LexicalExit { value, .. } => {
                if let Some(value) = value {
                    self.collect_expr(program, variant_layouts, value, parameter_count, frame)?;
                }
            }
            ResolvedExprKind::Closure { captures, .. } => {
                for capture in captures {
                    self.collect_expr(
                        program,
                        variant_layouts,
                        &capture.value,
                        parameter_count,
                        frame,
                    )?;
                }
            }
            ResolvedExprKind::Invoke { callable, args } => {
                self.collect_expr(program, variant_layouts, callable, parameter_count, frame)?;
                self.collect_exprs(program, variant_layouts, args, parameter_count, frame)?;
                let local = self.add_local(parameter_count, I32)?;
                if self
                    .function_callables
                    .insert(expr.id.clone(), local)
                    .is_some()
                {
                    return Err(error("aggregate function invocation scratch repeats"));
                }
                for argument in args {
                    let local =
                        self.add_local(parameter_count, scalar_wasm_type(program, &argument.ty)?)?;
                    if self
                        .function_arguments
                        .insert(argument.id.clone(), local)
                        .is_some()
                    {
                        return Err(error(
                            "aggregate function invocation argument scratch repeats",
                        ));
                    }
                }
            }
            ResolvedExprKind::FunctionReference { .. } => {}
            ResolvedExprKind::LiteralFormat { args, .. } => {
                self.collect_literal_format(
                    program,
                    variant_layouts,
                    expr,
                    args,
                    parameter_count,
                    frame,
                )?;
            }
            ResolvedExprKind::Call { args, .. }
            | ResolvedExprKind::VecFieldRead { args, .. }
            | ResolvedExprKind::RowLoan { args, .. } => {
                self.collect_exprs(program, variant_layouts, args, parameter_count, frame)?
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                self.collect_exprs(program, variant_layouts, &call.args, parameter_count, frame)?
            }
            ResolvedExprKind::HostCommandCall(call) => {
                self.collect_exprs(program, variant_layouts, &call.args, parameter_count, frame)?
            }
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => self.collect_byte_range(
                program,
                variant_layouts,
                source,
                start,
                end,
                parameter_count,
                frame,
            )?,
            ResolvedExprKind::Unary { value, .. } => {
                self.collect_expr(program, variant_layouts, value, parameter_count, frame)?;
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                self.collect_expr(program, variant_layouts, operand, parameter_count, frame)?;
            }
            ResolvedExprKind::Binary { left, right, .. } => self.collect_binary(
                program,
                variant_layouts,
                left,
                right,
                parameter_count,
                frame,
            )?,
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => self.collect_block(
                program,
                variant_layouts,
                statements,
                tail,
                parameter_count,
                frame,
            )?,
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.collect_if(
                program,
                variant_layouts,
                condition,
                then_branch,
                else_branch,
                parameter_count,
                frame,
            )?,
            ResolvedExprKind::ConstructRecord { fields, .. } => self.collect_record_fields(
                program,
                variant_layouts,
                fields,
                parameter_count,
                frame,
            )?,
            ResolvedExprKind::ConstructVariant { fields, .. } => self.collect_variant_fields(
                program,
                variant_layouts,
                fields,
                parameter_count,
                frame,
            )?,
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => self.collect_match(
                program,
                variant_layouts,
                scrutinee,
                arms,
                parameter_count,
                frame,
            )?,
            ResolvedExprKind::Project { base, .. } => {
                self.collect_expr(program, variant_layouts, base, parameter_count, frame)?;
            }
            ResolvedExprKind::Upcast { source } | ResolvedExprKind::Yield { request: source } => {
                self.collect_expr(program, variant_layouts, source, parameter_count, frame)?;
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                self.collect_expr(program, variant_layouts, base, parameter_count, frame)?;
                for field in fields {
                    self.collect_expr(
                        program,
                        variant_layouts,
                        &field.value,
                        parameter_count,
                        frame,
                    )?;
                }
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::BorrowedUtf8Literal(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
        Ok(())
    }
}
