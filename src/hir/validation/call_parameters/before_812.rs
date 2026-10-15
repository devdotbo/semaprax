use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_argument_ownership_view_saved_812(
        &self,
        argument: &ResolvedExpr,
        param: ParameterView<'_>,
    ) -> Result<(), Diagnostic> {
        #[cfg(test)]
        if ownership_flags::original_selected() {
            return self.validate_argument_ownership_view_before(argument, param);
        }
        let actual = argument.ownership;
        let facts = self.borrowed_ownership_copy(param.ty)?.ok_or_else(|| {
            hir_error_at_span(
                argument.span,
                format!("type `{}` has no semantic facts", param.ty.identity_key()),
            )
        })?;
        let valid = if facts {
            actual == OwnershipMode::Value && param.ownership == OwnershipMode::Value
        } else {
            match param.ownership {
                OwnershipMode::Own => actual == OwnershipMode::Own,
                OwnershipMode::Borrow => {
                    let exact_place = matches!(&argument.kind, ResolvedExprKind::Place(_));
                    if let ResolvedExprKind::RowLoan { loan, .. } = &argument.kind {
                        // Row Loan v1: its call slot, parameter identity and
                        // callee proof are replayed by `hir::row_loan`.
                        actual == OwnershipMode::Borrow && loan.element == *param.ty
                    } else if crate::stdin_stream_ops::is_reader(param.ty) {
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                            && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
                    } else if *param.ty == ResolvedType::Bytes
                        || crate::hir::owned_collection_record::vector(
                            &self.program.declarations,
                            param.ty,
                        )
                    {
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow) && exact_place
                    } else if crate::map_ops::is_collection(param.ty) {
                        // Compiler-owned nominal collections are leaves, not
                        // authored aggregate records; projected and temporary
                        // readers borrow their carrier without transferring it.
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                    } else if resolved_type_contains_owned_bytes(self.program, param.ty) {
                        (vec_intrinsic::is_owned_vec_carrier(self.program, param.ty)
                            || crate::hir::type_reachability::is_admitted_nested_owned_byte_record(
                                &self.program.declarations,
                                param.ty,
                            )
                            || self.executable_profiles.runtime_admitted(param.ty)?
                            || resolved_type_is_direct_owned_variant(self.program, param.ty)
                            || resolved_type_is_flat_owned_byte_variant(self.program, param.ty))
                            && matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                            && matches!(
                                &argument.kind,
                                ResolvedExprKind::Place(place) if place.projections.is_empty()
                            )
                    } else if self.executable_profiles.runtime_admitted(param.ty)?
                        || resolved_type_is_direct_owned_variant(self.program, param.ty)
                    {
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                            && matches!(
                                &argument.kind,
                                ResolvedExprKind::Place(place) if place.projections.is_empty()
                            )
                    } else {
                        true
                    }
                }
                OwnershipMode::Shared => actual == OwnershipMode::Shared,
                OwnershipMode::Value => false,
            }
        };
        if valid {
            Ok(())
        } else {
            Err(hir_error_at_span(
                argument.span,
                format!(
                    "argument ownership is incompatible with parameter `{}`",
                    param.identity
                ),
            ))
        }
    }
}
