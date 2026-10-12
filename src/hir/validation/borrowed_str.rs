//! Root authentication for immutable borrowed-string local bindings.

use super::*;

impl HirValidator<'_> {
    pub(super) fn borrowed_str_let_origin(
        &self,
        value: &ResolvedExpr,
        binding: &ResolvedBinding,
    ) -> Result<(Place, bool), Diagnostic> {
        match &value.kind {
            ResolvedExprKind::BorrowedUtf8Literal(text) => {
                crate::hir::static_text::validate_literal(text)?;
                if value.ty != ResolvedType::Str || value.ownership != OwnershipMode::Borrow {
                    return Err(hir_error("static text local has invalid type or ownership"));
                }
                Ok((
                    Place {
                        root: binding.id.clone(),
                        projections: Vec::new(),
                    },
                    false,
                ))
            }
            ResolvedExprKind::Call { .. }
                if crate::vec_ops::string_slot::inspect(value).is_some() =>
            {
                Ok((
                    crate::vec_ops::string_slot::place(value)
                        .expect("authenticated slot place")
                        .clone(),
                    true,
                ))
            }
            ResolvedExprKind::VecFieldRead { args, .. } => {
                let Some(ResolvedExpr {
                    kind: ResolvedExprKind::Place(place),
                    ..
                }) = args.first()
                else {
                    return Err(hir_error("vector view lacks a named carrier origin"));
                };
                Ok((place.clone(), true))
            }
            ResolvedExprKind::Place(place) if place.projections.is_empty() => self
                .borrowed_str_aliases
                .get(&place.root)
                .cloned()
                .map(|origin| (origin, false))
                .ok_or_else(|| {
                    hir_error("borrowed-str local alias lacks authenticated root provenance")
                }),
            ResolvedExprKind::BorrowPlace { operation, place }
                if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID =>
            {
                Ok((place.clone(), true))
            }
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(call.result, ResolvedImportResultKind::BorrowedStr { .. }) =>
            {
                let import = self
                    .program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .find(|import| import.id == call.import)
                    .filter(|import| {
                        native_borrow::admitted(self.program, import)
                            && import.result.kind == call.result
                    })
                    .ok_or_else(|| {
                        hir_error("native string view lacks its authenticated receiver relation")
                    })?;
                let [receiver] = call.args.as_slice() else {
                    return Err(hir_error("native string view must have one receiver"));
                };
                let ResolvedExprKind::Place(place) = &receiver.kind else {
                    return Err(hir_error("native string view requires a named receiver"));
                };
                if !place.projections.is_empty() || receiver.ty != import.parameters[0].ty {
                    return Err(hir_error("native string view receiver identity mismatch"));
                }
                Ok((place.clone(), true))
            }
            ResolvedExprKind::HostCommandCall(call)
                if call.operation == ResolvedHostCommandOperation::ArgUtf8
                    || crate::environment_ops::is_lookup(call.operation) =>
            {
                Ok((
                    Place {
                        root: ValueId::intrinsic_parameter(
                            if crate::environment_ops::is_lookup(call.operation) {
                                crate::environment_ops::ARENA_ID
                            } else {
                                crate::command_io_ops::ARG_UTF8_ID
                            },
                            usize::MAX,
                        ),
                        projections: Vec::new(),
                    },
                    false,
                ))
            }
            _ => Err(hir_error(
                "borrowed-str local must be an exact alias or authenticated owning String view",
            )),
        }
    }
}

impl HirValidator<'_> {
    pub(super) fn finish_static_text(
        &self,
        expression: &ResolvedExpr,
        value: &crate::hir::StaticUtf8,
    ) -> Result<(), Diagnostic> {
        if !value.is_backed() {
            return Err(hir_error("static text backing allocation was refused"));
        }
        crate::hir::static_text::validate_literal(value)?;
        self.record_static_pool(value.len(), true)?;
        self.finish_expr(expression, &ResolvedType::Str, OwnershipMode::Borrow)
    }
}

impl HirValidator<'_> {
    pub(super) fn record_static_pool(
        &self,
        bytes: usize,
        selected: bool,
    ) -> Result<(), Diagnostic> {
        let (present, total) = self.static_text.get();
        // The mandatory expression walk counts every decoded occurrence, including
        // ordinary owned text. No extra cursor or heap inventory is introduced.
        let total = total
            .checked_add(bytes)
            .ok_or_else(|| hir_error("decoded text pool length overflow"))?;
        self.static_text.set((present || selected, total));
        Ok(())
    }
    pub(super) fn finish_static_pool(&self) -> Result<bool, Diagnostic> {
        let (selected, bytes) = self.static_text.get();
        if selected && bytes > crate::hir::static_text::MAX_POOL_BYTES {
            return Err(hir_error("selected decoded text pool exceeds 65536 bytes"));
        }
        Ok(selected)
    }
}

impl HirValidator<'_> {
    pub(super) fn binding_has_live_overlapping_loans(
        &self,
        binding: &ValidationBinding,
        requested: &Place,
        expression: &ExpressionId,
        phase: LoanPointPhase,
    ) -> Result<bool, Diagnostic> {
        for loan in &binding.active_loans {
            if let Some(ordinal) = self
                .canonical_loans
                .live(&(expression.clone(), phase, *loan))
            {
                let origin = self.canonical_loans.origin(*ordinal).ok_or_else(|| {
                    hir_error("canonical shared-loan origin directory is incomplete")
                })?;
                if origin.root != requested.root {
                    return Err(hir_error(
                        "active shared-loan provenance disagrees with its owner binding",
                    ));
                }
                if path_is_prefix(&origin.projections, &requested.projections)
                    || path_is_prefix(&requested.projections, &origin.projections)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
