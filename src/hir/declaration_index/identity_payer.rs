//! Explicit identity payer for initial declaration-index construction.
//! Semantic diagnostics retain their original owned compatibility contract.
//! Identity refusal returns the exact caller error, without rendering a new
//! Diagnostic, allocating a report, or consulting an ambient ledger.
use super::*;

#[derive(Debug)]
pub(in crate::hir) enum IdentityIndexError<E> {
    Semantic(Diagnostic),
    Identity(E),
}
impl<E> From<Diagnostic> for IdentityIndexError<E> {
    fn from(error: Diagnostic) -> Self {
        Self::Semantic(error)
    }
}

impl DeclarationIndex {
    /// The caller owns this reservation callback and its refusal carrier.
    /// Only bytes are requested from it; exact source text is constructed
    /// internally, so callback presence cannot substitute declaration IDs.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "explicit index payer seam awaits caller-owned outward report migration"
        )
    )]
    pub(in crate::hir) fn from_verified_with_identity_reservation<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        mut reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        Self::from_verified_with_identity_payer(program, prelude, &mut |text| {
            DeclarationId::paid_borrowed(text, &mut reserve)
        })
    }

    /// Same explicit owner, now also acquiring the canonical nominal owners.
    pub(in crate::hir) fn from_verified_with_nominal_reservation<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        mut reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        let workspace = super::nominal_rollback::Workspace::acquire(program, prelude, &mut reserve)
            .map_err(IdentityIndexError::Identity)?;
        let mut index = Self::build_nominal_index(
            program,
            prelude,
            &mut |text| DeclarationId::paid_borrowed(text, &mut reserve),
            true,
            workspace,
        )?;
        index
            .share_nominal_payloads(&mut reserve)
            .map_err(IdentityIndexError::Identity)?;
        index.nominal_drop_workspace.finish();
        Ok(index)
    }

    // Borrow the callback on the stack so this complete constructor specializes
    // by its refusal carrier, rather than every caller closure type. No callback
    // escapes and reservation order and owned identity construction stay exact.
    pub(super) fn from_verified_with_identity_payer<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        identity: &mut dyn FnMut(&str) -> Result<DeclarationId, E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        Self::build_nominal_index(
            program,
            prelude,
            identity,
            false,
            super::nominal_rollback::Workspace::default(),
        )
    }

    fn build_nominal_index<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        identity: &mut dyn FnMut(&str) -> Result<DeclarationId, E>,
        canonical_nominals: bool,
        workspace: super::nominal_rollback::Workspace,
    ) -> Result<Self, IdentityIndexError<E>> {
        let mut index = Self::default();
        index.nominal_drop_workspace = workspace;
        for declaration in program.types().iter().chain(prelude) {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                TypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                TypeDeclarationKind::Class { .. } => DeclarationKind::Class,
                TypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            index.insert_top_level(
                declaration.name.clone(),
                identity(&declaration.stable_id).map_err(IdentityIndexError::Identity)?,
                kind,
                if crate::prelude::is_compiler_owned_id(&declaration.stable_id) {
                    IdentityOrigin::CompilerOwned
                } else if declaration.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let owner = index.inserted_identity(&declaration.stable_id);
            let parameters = declaration
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("type `{}` has too many parameters", declaration.name),
                                declaration.span,
                            )
                            .at_path(program.path())
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        for interface in program.interfaces() {
            let interface_id =
                identity(&interface.stable_id).map_err(IdentityIndexError::Identity)?;
            index.insert_top_level(
                interface.name.clone(),
                interface_id.clone(),
                DeclarationKind::Interface,
                IdentityOrigin::Explicit,
            );
            for import in &interface.imports {
                let import_id =
                    identity(&import.stable_id).map_err(IdentityIndexError::Identity)?;
                index
                    .imports_by_key
                    .insert(import.stable_id.clone(), import_id.clone());
                if import.native_rust {
                    index
                        .native_rust_imports_by_name
                        .insert(import.name.clone(), import_id.clone());
                }
                index.insert_owned_declaration(
                    interface_id.clone(),
                    import.name.clone(),
                    import_id,
                    DeclarationKind::Import,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for declaration in program.types().iter().chain(prelude) {
            let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind else {
                continue;
            };
            if let [lifecycle] = lifecycles.as_slice() {
                let lifecycle_id = lifecycle
                    .stable_id
                    .as_ref()
                    .expect("verified lifecycle has an explicit identity");
                index.insert_owned_declaration(
                    index.inserted_identity(&declaration.stable_id),
                    "drop".to_owned(),
                    identity(lifecycle_id).map_err(IdentityIndexError::Identity)?,
                    DeclarationKind::ResourceDrop,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for function in program.functions() {
            let owner = identity(&function.stable_id).map_err(IdentityIndexError::Identity)?;
            index.insert_top_level(
                function.name.clone(),
                owner.clone(),
                DeclarationKind::Function,
                if super::record_invariants::is_synthesized(&function.name) {
                    IdentityOrigin::CompilerOwned
                } else if function.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let parameters = function
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("function `{}` has too many parameters", function.name),
                                function.span,
                            )
                            .at_path(program.path())
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        if program
            .interfaces()
            .iter()
            .flat_map(|interface| &interface.imports)
            .filter(|import| import.native_rust)
            .any(|import| index.functions_by_name.contains_key(&import.name))
        {
            return Err(IdentityIndexError::Semantic(
                Diagnostic::error(
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: symbol collision",
                    Span::default(),
                )
                .at_path(program.path()),
            ));
        }
        for declaration in program.types().iter().chain(prelude) {
            let (TypeDeclarationKind::Record { fields }
            | TypeDeclarationKind::Class { fields, .. }) = &declaration.kind
            else {
                continue;
            };
            let owner = index.inserted_identity(&declaration.stable_id);
            let mut resolved_fields = index
                .nominal_drop_workspace
                .fields::<ResolvedFieldDeclaration>(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = index
                    .resolve_source_type(&field.ty, Some(&owner))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "SPX-H001",
                            format!("unresolved field type `{}`", field.ty),
                            field.span,
                        )
                        .at_path(program.path())
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("record `{}` has too many fields", declaration.name),
                        declaration.span,
                    )
                    .at_path(program.path())
                })?;
                let ty = index.nominal_drop_workspace.resolved_type(ty);
                let resolved = ResolvedFieldDeclaration {
                    id: identity(&field.stable_id).map_err(IdentityIndexError::Identity)?,
                    name: field.name.clone(),
                    index: field_index,
                    ty: ty.into_type(),
                    span: field.span,
                };
                index.insert_field(
                    owner.clone(),
                    resolved.clone(),
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                resolved_fields.push(resolved);
            }
            index
                .record_fields
                .insert(owner, resolved_fields.into_vec());
        }
        for declaration in program.types().iter() {
            let TypeDeclarationKind::Class { fields, methods } = &declaration.kind else {
                continue;
            };
            let owner = index.inserted_identity(&declaration.stable_id);
            let mut resolved_fields = index
                .nominal_drop_workspace
                .fields::<ResolvedFieldDeclaration>(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = index
                    .resolve_source_type(&field.ty, Some(&owner))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "SPX-H001",
                            format!("unresolved class field type `{}`", field.ty),
                            field.span,
                        )
                        .at_path(program.path())
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("class `{}` has too many fields", declaration.name),
                        declaration.span,
                    )
                    .at_path(program.path())
                })?;
                let ty = index.nominal_drop_workspace.resolved_type(ty);
                let resolved = ResolvedFieldDeclaration {
                    id: identity(&field.stable_id).map_err(IdentityIndexError::Identity)?,
                    name: field.name.clone(),
                    index: field_index,
                    ty: ty.into_type(),
                    span: field.span,
                };
                index.insert_field(
                    owner.clone(),
                    resolved.clone(),
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                resolved_fields.push(resolved);
            }
            index
                .record_fields
                .insert(owner.clone(), resolved_fields.into_vec());
            for method in methods {
                let method_id =
                    identity(&method.stable_id).map_err(IdentityIndexError::Identity)?;
                if index.declarations.contains_key(&method_id) {
                    return Err(IdentityIndexError::Semantic(
                        Diagnostic::error(
                            "SPX-S102",
                            format!("duplicate stable id `{}`", method.stable_id),
                            method.span,
                        )
                        .at_path(program.path()),
                    ));
                }
                index.insert_owned_declaration(
                    owner.clone(),
                    method.name.clone(),
                    method_id,
                    DeclarationKind::Function,
                    if method.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                if !method.type_parameters.is_empty() {
                    return Err(IdentityIndexError::Semantic(
                        Diagnostic::error(
                            "SPX-T224",
                            format!(
                                "class method `{}` cannot declare generic parameters in this slice",
                                method.name
                            ),
                            method.span,
                        )
                        .at_path(program.path()),
                    ));
                }
                index
                    .type_parameters
                    .insert(index.inserted_identity(&method.stable_id), Vec::new());
            }
        }
        for declaration in program.types().iter().chain(prelude) {
            let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                continue;
            };
            let owner = index.inserted_identity(&declaration.stable_id);
            let mut resolved_cases = index
                .nominal_drop_workspace
                .fields::<ResolvedVariantCaseDeclaration>(cases.len());
            for (case_ordinal, case) in cases.iter().enumerate() {
                let case_id = identity(&case.stable_id).map_err(IdentityIndexError::Identity)?;
                let case_index = u32::try_from(case_ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("variant `{}` has too many cases", declaration.name),
                        declaration.span,
                    )
                    .at_path(program.path())
                })?;
                index.insert_case(
                    owner.clone(),
                    case.name.clone(),
                    case_id.clone(),
                    if crate::prelude::is_compiler_owned_id(&case.stable_id) {
                        IdentityOrigin::CompilerOwned
                    } else if case.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                let mut resolved_fields = index
                    .nominal_drop_workspace
                    .fields::<ResolvedFieldDeclaration>(case.fields.len());
                for (field_ordinal, field) in case.fields.iter().enumerate() {
                    let ty = index
                        .resolve_source_type(&field.ty, Some(&owner))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "SPX-H001",
                                format!("unresolved case field type `{}`", field.ty),
                                field.span,
                            )
                            .at_path(program.path())
                        })?;
                    let field_index = u32::try_from(field_ordinal).map_err(|_| {
                        Diagnostic::error(
                            "SPX-H006",
                            format!(
                                "variant case `{}::{}` has too many fields",
                                declaration.name, case.name
                            ),
                            case.span,
                        )
                        .at_path(program.path())
                    })?;
                    let ty = index.nominal_drop_workspace.resolved_type(ty);
                    let resolved = ResolvedFieldDeclaration {
                        id: identity(&field.stable_id).map_err(IdentityIndexError::Identity)?,
                        name: field.name.clone(),
                        index: field_index,
                        ty: ty.into_type(),
                        span: field.span,
                    };
                    index.insert_case_field(
                        case_id.clone(),
                        resolved.clone(),
                        if crate::prelude::is_compiler_owned_id(&field.stable_id) {
                            IdentityOrigin::CompilerOwned
                        } else if field.explicit_id {
                            IdentityOrigin::Explicit
                        } else {
                            IdentityOrigin::Automatic
                        },
                    );
                    resolved_fields.push(resolved);
                }
                if !canonical_nominals {
                    index
                        .case_fields
                        .insert(case_id.clone(), resolved_fields.clone());
                }
                resolved_cases.push(ResolvedVariantCaseDeclaration {
                    id: case_id,
                    name: case.name.clone(),
                    index: case_index,
                    fields: resolved_fields.into_vec(),
                    span: case.span,
                });
            }
            index.variant_cases.insert(owner, resolved_cases.into_vec());
        }
        index.materialize_class_inheritance(program)?;
        if !index.populate_type_facts() {
            return Err(IdentityIndexError::Semantic(
                Diagnostic::error(
                    "SPX-T217",
                    "record declarations contain an illegal by-value recursive layout",
                    Span::default(),
                )
                .at_path(program.path()),
            ));
        }
        Ok(index)
    }
}
