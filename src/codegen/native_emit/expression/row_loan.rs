//! Row Loan v1 native carrier, `semaprax.native.borrowed-record-carrier.v1`.
//!
//! A lent `borrow R` parameter receives `const spx_row_loan_v1 *` instead of
//! the ordinary record pointer and hidden leaf pointers. Tag 1 names a
//! resident `const R *` (every ordinary caller adapts its argument this way);
//! tag 2 names the frozen owned-leaf Vec carrier, the row index and the Vec
//! generation captured when the loan was created. The helper is compiled once;
//! each Copy scalar field read dispatches on the tag and re-authenticates the
//! carrier, generation, descriptor and bounds before reading. No row, record
//! or leaf is copied, moved or materialized.
use super::*;

pub(in crate::codegen::native_emit) const RUNTIME: &str = r#"
typedef struct spx_row_loan_v1 {
    uint32_t tag;
    const void *resident;
    const spx_vec_v1 *vector;
    uint64_t index;
    uint64_t generation;
} spx_row_loan_v1;
static __attribute__((unused)) void spx_row_loan_check_v1(const spx_row_loan_v1 *l) {
    if (!l) spx_runtime_invariant_failure("row loan carrier is absent");
    if (l->tag == UINT32_C(1)) {
        if (!l->resident || l->vector) spx_runtime_invariant_failure("invalid resident row loan carrier");
        return;
    }
    if (l->tag != UINT32_C(2) || l->resident || !l->vector || l->vector->generation != l->generation)
        spx_runtime_invariant_failure("invalid or stale row loan carrier");
}
"#;

impl<O: COutput> CEmitter<'_, O> {
    /// Index of `root` when it is one of this function's lent parameters.
    pub(in crate::codegen::native_emit) fn lent_parameter(
        &self,
        root: &hir::ValueId,
    ) -> Option<usize> {
        let index = self.function.params.iter().position(|p| p.id == *root)?;
        self.functions
            .get(self.execution)
            .is_some_and(|metadata| metadata.lent.get(index) == Some(&true))
            .then_some(index)
    }

    /// Caller side: authenticate the frozen Vec and row, then stage a tag-2
    /// carrier. The Vec place is inspected before the index, left to right.
    pub(super) fn emit_row_loan(
        &mut self,
        expr: &ResolvedExpr,
        loan: &hir::RowLoan,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        let [source, index] = args else {
            return Err(backend_error(
                "row loan requires exactly Vec and index children",
            ));
        };
        let ResolvedExprKind::Place(place) = &source.kind else {
            return Err(backend_error(
                "row loan requires one authenticated Vec place",
            ));
        };
        if loan.version != hir::RowLoanVersion::V1
            || !place.projections.is_empty()
            || expr.ty != loan.element
            || expr.ownership != hir::OwnershipMode::Borrow
            || index.ty != ResolvedType::Usize
        {
            return Err(backend_error("row loan disagrees with validated HIR"));
        }
        let layout = crate::codegen::native_vec::owned_leaf::layout(self.program, &loan.element)
            .ok_or_else(|| backend_error("row loan has no native row descriptor"))?;
        let vector = self.emit_place(place)?;
        self.require_type(
            &vector.ty,
            &crate::vec_ops::resolved_vec(loan.element.clone()),
            "row loan Vec carrier",
        )?;
        let index = self.emit_expr(index)?;
        let carrier = format!("spx_row_loan_{}", self.next_local);
        self.next_local += 1;
        let source = vector.code;
        let symbol = layout.symbol;
        self.line(&format!(
            "(void)spx_leaf_check(spx_ctx, &({source}), &{symbol});"
        ));
        self.line(&format!(
            "if ({} >= ({source}).len) spx_runtime_invariant_failure(\"row loan index exceeds its frozen traversal bound\");",
            index.code
        ));
        self.line(&format!(
            "const spx_row_loan_v1 {carrier} = {{UINT32_C(2), NULL, &({source}), {}, ({source}).generation}};",
            index.code
        ));
        Ok(CValue {
            code: carrier,
            ty: loan.element.clone(),
        })
    }

    /// Call side: the carrier pointer for one lent parameter. Ordinary
    /// arguments are resident records and adapt to a tag-1 carrier.
    pub(super) fn row_loan_argument(
        &mut self,
        argument: &ResolvedExpr,
        value: &CValue,
    ) -> Result<String, Diagnostic> {
        if matches!(argument.kind, ResolvedExprKind::RowLoan { .. }) {
            return Ok(format!("&({})", value.code));
        }
        let ResolvedExprKind::Place(place) = &argument.kind else {
            return Err(backend_error(
                "resident borrowed record argument is not one authenticated place",
            ));
        };
        if !place.projections.is_empty() {
            return Err(backend_error(
                "resident borrowed record argument is projected",
            ));
        }
        let carrier = format!("spx_row_loan_{}", self.next_local);
        self.next_local += 1;
        self.line(&format!(
            "const spx_row_loan_v1 {carrier} = {{UINT32_C(1), (const void *)&({}), NULL, UINT64_C(0), UINT64_C(0)}};",
            value.code
        ));
        Ok(format!("&{carrier}"))
    }

    /// Helper side: one exact Copy scalar field read through the carrier.
    pub(super) fn emit_row_loan_field_read(
        &mut self,
        index: usize,
        place: &hir::Place,
    ) -> Result<CValue, Diagnostic> {
        let element = self
            .function
            .params
            .get(index)
            .map(|parameter| parameter.ty.clone())
            .ok_or_else(|| backend_error("row loan parameter is absent"))?;
        let selected =
            crate::hir::row_loan::lent_field_read(&self.program.declarations, &element, place)
                .ok_or_else(|| {
                    backend_error("row loan parameter is used outside a Copy scalar field read")
                })?;
        let position = selected.position;
        let field_id = selected.declaration.id.clone();
        let field_type = selected.declaration.ty.clone();
        let record = self.record_layout(&element)?;
        let resident_field = record
            .field(&field_id)
            .cloned()
            .ok_or_else(|| backend_error("row loan field is absent from the resident layout"))?;
        if resident_field.size == 0 {
            return Err(backend_error("row loan reached an erased resident field"));
        }
        let rows = crate::codegen::native_vec::owned_leaf::layout(self.program, &element)
            .ok_or_else(|| backend_error("row loan has no native row descriptor"))?;
        if position >= rows.offsets.len() {
            return Err(backend_error(
                "row loan field order disagrees with the native row descriptor",
            ));
        }
        let record_type = c_value_type(self.program, self.resource_abi, &element)?;
        let carrier = format!("spx_param_{index}");
        let output = self.temporary(&field_type)?;
        let suffix = self.next_local;
        self.next_local += 1;
        let slot = format!("spx_row_slot_{suffix}");
        let bits = format!("spx_row_bits_{suffix}");
        self.line(&format!("spx_row_loan_check_v1({carrier});"));
        self.line(&format!("if ({carrier}->tag == UINT32_C(1)) {{"));
        self.indent += 1;
        self.line(&format!(
            "{output} = ((const {record_type} *){carrier}->resident)->{};",
            c_field_symbol(&field_id)
        ));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("const unsigned char *{slot} = NULL;"));
        self.line(&format!(
            "if (spx_leaf_read_field(spx_ctx, &(*{carrier}->vector), &{}, {carrier}->index, UINT32_C({position}), &{slot}) != SPX_STATUS_SUCCESS) spx_runtime_invariant_failure(\"row loan lost its frozen row\");",
            rows.symbol
        ));
        self.line(&format!("uint64_t {bits} = UINT64_C(0);"));
        self.line(&format!("memcpy(&{bits}, {slot}, sizeof({bits}));"));
        self.line(&format!(
            "{output} = {};",
            super::vec_ops::vec_bits_to_scalar(&bits, &field_type)?
        ));
        self.indent -= 1;
        self.line("}");
        Ok(CValue {
            code: output,
            ty: field_type,
        })
    }
}
