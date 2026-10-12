//! Complete record-pattern binding admission over either scope owner.
use super::*;
mod binding_path;
#[cfg(test)]
mod owned;
#[cfg(test)]
mod tests;
impl HirValidator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_record_match_pattern(
        &mut self,
        function: &FunctionExecutionId,
        expected: &ResolvedType,
        record: &DeclarationId,
        instance: &ResolvedType,
        fields: &[ResolvedRecordMatchPatternField],
        scope: &mut dyn ScopeWrite,
        path: impl std::fmt::Display,
        mode: ResolvedMatchMode,
    ) -> Result<(), Diagnostic> {
        let expected_root = expected;
        let exact_recursive = mode != ResolvedMatchMode::Value
            && super::super::type_reachability::is_nested_nonflat_owned_byte_record(
                &self.program.declarations,
                expected,
            );
        enum Frame<'a> {
            Enter {
                expected: ResolvedType,
                record: &'a DeclarationId,
                instance: &'a ResolvedType,
                fields: &'a [ResolvedRecordMatchPatternField],
                mode: ResolvedMatchMode,
            },
            Fields {
                expected: ResolvedType,
                record: &'a DeclarationId,
                fields: &'a [ResolvedRecordMatchPatternField],
                declared_fields: &'a [ResolvedFieldDeclaration],
                index: usize,
                seen: BTreeSet<DeclarationId>,
                mode: ResolvedMatchMode,
            },
        }
        let mut frames = vec![Frame::Enter {
            expected: expected.clone(),
            record,
            instance,
            fields,
            mode,
        }];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter {
                    expected,
                    record,
                    instance,
                    fields,
                    mode,
                } => {
                    if instance != &expected {
                        return Err(hir_error(
                            "resolved record pattern has the wrong concrete instance",
                        ));
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments: _,
                    } = &expected
                    else {
                        return Err(hir_error("resolved record pattern instance is not nominal"));
                    };
                    if declaration != record
                        || self
                            .program
                            .declarations
                            .declaration(record)
                            .is_none_or(|item| item.kind != DeclarationKind::Record)
                    {
                        return Err(hir_error(
                            "resolved record pattern references a foreign record",
                        ));
                    }
                    if mode != ResolvedMatchMode::Value
                        && resolved_type_contains_owned_bytes(self.program, &expected)
                        && !super::super::type_reachability::is_admitted_nested_owned_byte_record(
                            &self.program.declarations,
                            &expected,
                        )
                        && !super::super::owned_text_record::runtime_admitted(
                            &expected,
                            &self.program.declarations,
                        )
                    {
                        return Err(hir_error(
                            "ownership-aware record pattern is outside the bounded nested owned-Bytes profile",
                        ));
                    }
                    let facts =
                        self.program
                            .declarations
                            .type_facts(&expected)
                            .ok_or_else(|| {
                                hir_error("resolved record pattern has no exact type facts")
                            })?;
                    match mode {
                        ResolvedMatchMode::Value
                            if facts.copy && !facts.contains_resource && !facts.needs_drop => {}
                        ResolvedMatchMode::Own | ResolvedMatchMode::Borrow
                            if !facts.copy && facts.needs_drop => {}
                        _ => {
                            return Err(hir_error(
                                "resolved record pattern disagrees with its match ownership mode",
                            ));
                        }
                    }
                    let declared_fields = self
                        .program
                        .declarations
                        .record_fields(record)
                        .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?;
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        fields,
                        declared_fields,
                        index: 0,
                        seen: BTreeSet::new(),
                        mode,
                    });
                }
                Frame::Fields {
                    expected,
                    record,
                    fields,
                    declared_fields,
                    index,
                    mut seen,
                    mode,
                } => {
                    let Some(field) = fields.get(index) else {
                        if seen.len() != declared_fields.len() {
                            return Err(hir_error("resolved record pattern is missing fields"));
                        }
                        continue;
                    };
                    let declared = declared_fields
                        .iter()
                        .find(|candidate| candidate.id == field.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "resolved record pattern contains foreign field `{}`",
                                field.field
                            ))
                        })?;
                    if !seen.insert(field.field.clone()) {
                        return Err(hir_error(
                            "resolved record pattern contains a duplicate field",
                        ));
                    }
                    let ResolvedType::Nominal { arguments, .. } = &expected else {
                        unreachable!("validated record instance remains nominal")
                    };
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        fields,
                        declared_fields,
                        index: index + 1,
                        seen,
                        mode,
                    });
                    match &field.pattern {
                        ResolvedRecordMatchFieldPattern::Binding(binding) => {
                            let field_facts =
                                self.program.declarations.type_facts(&field_ty).ok_or_else(
                                    || hir_error("resolved record field has no exact type facts"),
                                )?;
                            if exact_recursive
                                && !(mode == ResolvedMatchMode::Own
                                    && super::super::owned_record_subtree::admitted(
                                        &self.program.declarations,
                                        expected_root,
                                        &field_ty,
                                    ))
                                && type_profiles::requires_record_pattern(self.program, &field_ty)
                            {
                                return Err(hir_error(
                                    "resolved nested owned-record field lacks a recursive record pattern",
                                ));
                            }
                            let ownership = if field_facts.needs_drop {
                                match mode {
                                    ResolvedMatchMode::Own => OwnershipMode::Own,
                                    ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                                    ResolvedMatchMode::Value => OwnershipMode::Value,
                                }
                            } else {
                                OwnershipMode::Value
                            };
                            // The depth-first directory contains only pending ancestor
                            // Fields continuations, including this field's continuation.
                            // Its next index is exactly the visited field index plus one.
                            let expected_path = binding_path::BindingPath::new(
                                &path,
                                frames.iter().map(|frame| match frame {
                                    Frame::Fields { index, .. } => index - 1,
                                    Frame::Enter { .. } => {
                                        unreachable!("record entry consumed before binding")
                                    }
                                }),
                            );
                            if !binding.id.matches_local_display(
                                function,
                                expected_path.len()?,
                                &expected_path,
                            ) || binding.ty != field_ty
                                || binding.ownership != ownership
                            {
                                return Err(hir_error(
                                    "resolved record pattern binding is not canonical",
                                ));
                            }
                            self.insert_value(&binding.id)?;
                            self.validate_type(&binding.ty)?;
                            if scope.contains_key(&binding.id) {
                                return Err(hir_error(
                                    "resolved record pattern binding shadows an existing value",
                                ));
                            }
                            if scope.old_mut().is_none() {
                                scope_checkpoint::charge::clone_type(&binding.ty)?;
                            }
                            scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    contract_local: false,
                                    mutable: false,
                                    ty: binding.ty.clone(),
                                    ownership,
                                    availability: Availability::Available,
                                    active_loans: BTreeSet::new(),
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            )?;
                        }
                        ResolvedRecordMatchFieldPattern::Wildcard => {
                            if (mode == ResolvedMatchMode::Own || exact_recursive)
                                && self
                                    .program
                                    .declarations
                                    .type_facts(&field_ty)
                                    .is_some_and(|facts| facts.needs_drop)
                            {
                                let message = if exact_recursive {
                                    "resolved exact owned-record pattern wildcards a droppable field"
                                } else {
                                    "resolved owned record pattern leaves a droppable field unbound"
                                };
                                return Err(hir_error(message));
                            }
                        }
                        ResolvedRecordMatchFieldPattern::Record {
                            record,
                            instance,
                            fields,
                        } => {
                            let nested_mode = if self
                                .program
                                .declarations
                                .type_facts(&field_ty)
                                .is_some_and(|facts| facts.copy && !facts.needs_drop)
                            {
                                ResolvedMatchMode::Value
                            } else {
                                mode
                            };
                            frames.push(Frame::Enter {
                                expected: field_ty,
                                record,
                                instance,
                                fields,
                                mode: nested_mode,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
