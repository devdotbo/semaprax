//! Row Loan v1 Wasm carrier, `semaprax.wasm.borrowed-record-carrier.v1`.
//!
//! A borrowed record parameter stays one i32 linear-memory pointer, so every
//! ordinary caller and external wrapper keeps its exact ABI: a resident
//! record pointer has bit 31 clear (module memory is a few fixed pages). A
//! row loan passes the address of a 16-byte caller-frame block
//! `{owner: i64, index: i64}` with bit 31 set. The helper is compiled once;
//! each Copy scalar field read of a lent parameter dispatches on that tag. The
//! loan path calls the existing private Vec field-read import, which
//! re-authenticates the owner handle, generation, descriptor and bounds; any
//! refusal breaks the frozen-row contract and traps. Nothing is copied.
use super::*;

/// Bit 31 as a signed i32 immediate.
const TAG: i64 = i32::MIN as i64;
const ADDRESS: i64 = i32::MAX as i64;

#[derive(Default)]
pub(super) struct Plan {
    /// RowLoan expression -> (frame offset of its carrier block, i32 local).
    carriers: HashMap<ExpressionId, (u32, u32)>,
    /// Lent field-read Place expression -> 8-byte import scratch offset.
    reads: HashMap<ExpressionId, u32>,
}

impl FunctionPlan {
    /// Additive frame space and locals; programs without row loans allocate
    /// nothing, so their frames, locals and bytes stay exact.
    pub(super) fn collect_row_loans(
        &mut self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        let lent = crate::hir::row_loan::lent_parameters(program);
        let parameters = crate::hir::row_loan::lent_parameter_ids(&lent, function);
        let parameter_count = u32::try_from(function.params.len())
            .map_err(|_| error("aggregate function has too many parameters"))?;
        let mut pending = std::iter::once(&function.body)
            .chain(&function.requires)
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            match &expression.kind {
                ResolvedExprKind::RowLoan { .. } => {
                    let block = frame.allocate(16, 8)?;
                    let local = self.add_local(parameter_count, I32)?;
                    if self
                        .row_loans
                        .carriers
                        .insert(expression.id.clone(), (block, local))
                        .is_some()
                    {
                        return Err(error("row loan carrier identity repeats"));
                    }
                }
                ResolvedExprKind::Place(place) if parameters.contains(&place.root) => {
                    let scratch = frame.allocate(8, 8)?;
                    if self
                        .row_loans
                        .reads
                        .insert(expression.id.clone(), scratch)
                        .is_some()
                    {
                        return Err(error("row loan field read identity repeats"));
                    }
                }
                _ => {}
            }
            crate::hir::push_resolved_expression_children_in_authored_order(
                expression,
                &mut pending,
            );
        }
        Ok(())
    }
}

impl Emitter<'_> {
    /// Caller side: stage `{owner, index}` and return the tagged pointer.
    pub(super) fn emit_row_loan_argument(
        &mut self,
        argument: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let ResolvedExprKind::RowLoan { loan, args } = &argument.kind else {
            return Err(error("row loan argument has no descriptor"));
        };
        let [source, index] = args.as_slice() else {
            return Err(error("row loan requires exactly Vec and index children"));
        };
        if loan.version != crate::hir::RowLoanVersion::V1
            || argument.ty != loan.element
            || argument.ownership != crate::hir::OwnershipMode::Borrow
            || index.ty != ResolvedType::Usize
            || !matches!(&source.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
        {
            return Err(error("row loan disagrees with checked HIR"));
        }
        let (block, local) = *self
            .plan
            .row_loans
            .carriers
            .get(&argument.id)
            .ok_or_else(|| error("row loan carrier is absent"))?;
        let block = Pointer {
            local: self.plan.frame_base,
            offset: block,
        };
        // The whole Vec carrier is inspected before the index, left to right.
        let owner = self.emit_vec_borrow_place(source, &loan.element)?;
        let index = self.emit_expr(index)?;
        self.require_scalar(&index, &ResolvedType::Usize, "row loan index")?;
        self.emit_pointer(block);
        self.get_scalar(&owner);
        self.output.extend([0x37, 0x03, 0x00]); // i64.store owner
        self.emit_pointer(Pointer {
            local: block.local,
            offset: block.offset + 8,
        });
        self.get_scalar(&index);
        self.output.extend([0x37, 0x03, 0x00]); // i64.store index
        self.emit_pointer(block);
        self.output.push(0x41);
        write_i64(self.output, TAG);
        self.output.push(0x72); // i32.or
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: loan.element.clone(),
        })
    }

    /// Helper side: `parameter.field` for a lent parameter, or `None`.
    pub(super) fn emit_row_loan_read(
        &mut self,
        expression: &ResolvedExpr,
        place: &crate::hir::Place,
    ) -> Result<Option<Value>, Diagnostic> {
        let Some(scratch) = self.plan.row_loans.reads.get(&expression.id).copied() else {
            return Ok(None);
        };
        let parameter = self
            .function
            .params
            .iter()
            .find(|parameter| parameter.id == place.root)
            .ok_or_else(|| error("row loan read root is not a parameter"))?;
        let element = parameter.ty.clone();
        let selected =
            crate::hir::row_loan::lent_field_read(&self.program.declarations, &element, place)
                .ok_or_else(|| error("row loan parameter is used outside a scalar field read"))?;
        let position = selected.position;
        let field_id = selected.declaration.id.clone();
        let stored = selected.declaration.ty.clone();
        if expression.ty != stored {
            return Err(error("row loan field read type disagrees"));
        }
        let Some(Value::Aggregate {
            pointer: carrier, ..
        }) = self.bindings.get(&place.root).cloned()
        else {
            return Err(error("row loan parameter is not a pointer carrier"));
        };
        let record = layout(self.program, &element)?;
        let resident = record
            .field(&field_id)
            .cloned()
            .ok_or_else(|| error("row loan field is absent from the resident layout"))?;
        let descriptor = vec_owned_leaf::descriptor(self.program, &element)?;
        if descriptor
            .fields
            .get(position)
            .is_none_or(|(id, ty)| id.as_ref() != Some(&field_id) || *ty != stored)
        {
            return Err(error(
                "row loan field descriptor differs from its declaration",
            ));
        }
        let result = Value::Scalar {
            local: self.plan.expr_scalar(expression)?,
            ty: stored.clone(),
        };
        let scratch = Pointer {
            local: self.plan.frame_base,
            offset: scratch,
        };
        let address = |output: &mut Vec<u8>| {
            output.push(0x20);
            write_u32(output, carrier.local);
            output.push(0x41);
            write_i64(output, ADDRESS);
            output.push(0x71); // i32.and
        };
        self.output.push(0x20);
        write_u32(self.output, carrier.local);
        self.output.push(0x41);
        write_i64(self.output, TAG);
        self.output.push(0x71); // i32.and
        self.output.extend([0x45, 0x04, 0x40]); // resident: i32.eqz; if
        self.copy_value(
            &result,
            &value_at(
                Pointer {
                    local: carrier.local,
                    offset: carrier
                        .offset
                        .checked_add(resident.offset)
                        .ok_or_else(|| error("row loan resident offset overflows"))?,
                },
                stored.clone(),
                self.program,
            )?,
            "row loan resident field",
        )?;
        self.output.push(0x05); // else: frozen Vec row
        address(self.output);
        self.output.extend([0x29, 0x03, 0x00]); // owner
        for constant in [descriptor.identity, descriptor.shape] {
            self.output.push(0x42);
            write_i64(self.output, constant);
        }
        address(self.output);
        self.output.extend([0x29, 0x03, 0x08]); // index
        self.output.push(0x42);
        write_i64(self.output, position as i64);
        self.emit_pointer(scratch);
        self.output.push(0x10);
        write_u32(
            self.output,
            vec_owned_leaf::import_base(self.program) + vec_owned_leaf::IMPORT_COUNT,
        );
        self.output.extend([0x45, 0x45]); // any refusal breaks the frozen row
        self.trap_if();
        self.validate_record_scalar_bits(scratch, &stored)?;
        self.emit_pointer(scratch);
        self.load_scalar(&ResolvedType::I64);
        self.store_vec_element_bits(&result, &stored)?;
        self.output.push(0x0b);
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests;
