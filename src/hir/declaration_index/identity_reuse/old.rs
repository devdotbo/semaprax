//! Saved original index construction and class inheritance; no reuse helper.
use super::*;
impl DeclarationIndex {
    pub(super) fn saved_from_verified_with_prelude(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
    ) -> Result<Self, Diagnostic> {
        let mut index = Self::default();
        for declaration in program.types().iter().chain(prelude) {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                TypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                TypeDeclarationKind::Class { .. } => DeclarationKind::Class,
                TypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            index.insert_top_level(
                declaration.name.clone(),
                DeclarationId::new(declaration.stable_id.clone()),
                kind,
                if crate::prelude::is_compiler_owned_id(&declaration.stable_id) {
                    IdentityOrigin::CompilerOwned
                } else if declaration.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let owner = DeclarationId::new(declaration.stable_id.clone());
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
            let interface_id = DeclarationId::new(interface.stable_id.clone());
            index.insert_top_level(
                interface.name.clone(),
                interface_id.clone(),
                DeclarationKind::Interface,
                IdentityOrigin::Explicit,
            );
            for import in &interface.imports {
                let import_id = DeclarationId::new(import.stable_id.clone());
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
                    DeclarationId::new(declaration.stable_id.clone()),
                    "drop".to_owned(),
                    DeclarationId::new(lifecycle_id.clone()),
                    DeclarationKind::ResourceDrop,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for function in program.functions() {
            let owner = DeclarationId::new(function.stable_id.clone());
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
            return Err(Diagnostic::error(
                "SPX-B107",
                "Native Rust Interop declaration set is unsupported: symbol collision",
                Span::default(),
            )
            .at_path(program.path()));
        }
        for declaration in program.types().iter().chain(prelude) {
            let (TypeDeclarationKind::Record { fields }
            | TypeDeclarationKind::Class { fields, .. }) = &declaration.kind
            else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_fields = Vec::with_capacity(fields.len());
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
                let resolved = ResolvedFieldDeclaration {
                    id: DeclarationId::new(field.stable_id.clone()),
                    name: field.name.clone(),
                    index: field_index,
                    ty,
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
            index.record_fields.insert(owner, resolved_fields);
        }
        for declaration in program.types().iter() {
            let TypeDeclarationKind::Class { fields, methods } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_fields = Vec::with_capacity(fields.len());
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
                let resolved = ResolvedFieldDeclaration {
                    id: DeclarationId::new(field.stable_id.clone()),
                    name: field.name.clone(),
                    index: field_index,
                    ty,
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
            index.record_fields.insert(owner.clone(), resolved_fields);
            for method in methods {
                let method_id = DeclarationId::new(method.stable_id.clone());
                if index.declarations.contains_key(&method_id) {
                    return Err(Diagnostic::error(
                        "SPX-S102",
                        format!("duplicate stable id `{}`", method.stable_id),
                        method.span,
                    )
                    .at_path(program.path()));
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
                    return Err(Diagnostic::error(
                        "SPX-T224",
                        format!(
                            "class method `{}` cannot declare generic parameters in this slice",
                            method.name
                        ),
                        method.span,
                    )
                    .at_path(program.path()));
                }
                index
                    .type_parameters
                    .insert(DeclarationId::new(method.stable_id.clone()), Vec::new());
            }
        }
        for declaration in program.types().iter().chain(prelude) {
            let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_cases = Vec::with_capacity(cases.len());
            for (case_ordinal, case) in cases.iter().enumerate() {
                let case_id = DeclarationId::new(case.stable_id.clone());
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
                let mut resolved_fields = Vec::with_capacity(case.fields.len());
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
                    let resolved = ResolvedFieldDeclaration {
                        id: DeclarationId::new(field.stable_id.clone()),
                        name: field.name.clone(),
                        index: field_index,
                        ty,
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
                index
                    .case_fields
                    .insert(case_id.clone(), resolved_fields.clone());
                resolved_cases.push(ResolvedVariantCaseDeclaration {
                    id: case_id,
                    name: case.name.clone(),
                    index: case_index,
                    fields: resolved_fields,
                    span: case.span,
                });
            }
            index.variant_cases.insert(owner, resolved_cases);
        }
        index.saved_materialize_class_inheritance(program)?;
        if !index.populate_type_facts() {
            return Err(Diagnostic::error(
                "SPX-T217",
                "record declarations contain an illegal by-value recursive layout",
                Span::default(),
            )
            .at_path(program.path()));
        }
        Ok(index)
    }

    /// Class Inheritance v1: resolves `extends` links, rejects unknown or
    /// non-class parents and inheritance cycles, then prepends each class's
    /// inherited field prefix so layouts, semantic facts, projections, and
    /// cleanup treat an inherited member exactly like a declared one.
    fn saved_materialize_class_inheritance(
        &mut self,
        program: &dyn ProgramSource,
    ) -> Result<(), Diagnostic> {
        for declaration in program.types() {
            let Some(extends) = &declaration.extends else {
                continue;
            };
            let child = DeclarationId::new(declaration.stable_id.clone());
            let Type::Named {
                name: parent_name,
                arguments: parent_arguments,
            } = extends
            else {
                return Err(Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` must extend a named class type",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(program.path()));
            };
            if !parent_arguments.is_empty() {
                return Err(Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` extends generic type `{parent_name}`; inheritance over generic classes is closed in this slice",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(program.path()));
            }
            let parent = self.types_by_name.get(parent_name).ok_or_else(|| {
                Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` extends unknown type `{parent_name}`",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(program.path())
            })?;
            if self
                .declaration(parent)
                .is_none_or(|item| item.kind != DeclarationKind::Class)
            {
                return Err(Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` extends non-class type `{parent_name}`",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(program.path()));
            }
            self.class_parents.insert(child, parent.clone());
        }
        for declaration in program.types() {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            let child = DeclarationId::new(declaration.stable_id.clone());
            let mut visited = BTreeSet::new();
            let mut cursor = child.clone();
            while let Some(parent) = self.class_parents.get(&cursor) {
                if parent == &child || !visited.insert(parent.clone()) {
                    return Err(Diagnostic::error(
                        "SPX-T228",
                        format!(
                            "class `{}` participates in an inheritance cycle",
                            declaration.name
                        ),
                        declaration.span,
                    )
                    .at_path(program.path()));
                }
                cursor = parent.clone();
            }
        }
        // Materialize effective fields root-first so a child's prefix equals
        // its standalone ancestor layout at every depth. Declared-only lists
        // are snapshotted first because ancestors are materialized in place.
        let mut declared_fields = BTreeMap::new();
        for declaration in program.types() {
            if matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                let id = DeclarationId::new(declaration.stable_id.clone());
                if let Some(fields) = self.record_fields(&id) {
                    declared_fields.insert(id, fields.to_vec());
                }
            }
        }
        for declaration in program.types() {
            let TypeDeclarationKind::Class { .. } = &declaration.kind else {
                continue;
            };
            let child = DeclarationId::new(declaration.stable_id.clone());
            let mut chain = self.class_ancestors(&child);
            chain.reverse();
            chain.push(child.clone());
            let mut effective = Vec::new();
            let mut seen_names = BTreeSet::new();
            let mut seen_ids = BTreeSet::new();
            for member in &chain {
                let is_child = member == &child;
                let fields = declared_fields.get(member).ok_or_else(|| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("class `{member}` has no resolved fields"),
                        declaration.span,
                    )
                    .at_path(program.path())
                })?;
                for mut field in fields.iter().cloned() {
                    if !seen_names.insert(field.name.clone()) || !seen_ids.insert(field.id.clone())
                    {
                        return Err(Diagnostic::error(
                            "SPX-T229",
                            format!(
                                "class `{}` redeclares member `{}` from an ancestor",
                                declaration.name, field.name
                            ),
                            field.span,
                        )
                        .at_path(program.path()));
                    }
                    // Effective positions are canonical for the extending
                    // class; backends consume them as declaration order.
                    field.index = u32::try_from(effective.len()).map_err(|_| {
                        Diagnostic::error(
                            "SPX-H006",
                            format!("class `{}` has too many fields", declaration.name),
                            declaration.span,
                        )
                        .at_path(program.path())
                    })?;
                    if !is_child {
                        self.fields_by_owner_name
                            .insert((child.clone(), field.name.clone()), field.id.clone());
                    }
                    effective.push(field);
                }
            }
            self.record_fields.insert(child, effective);
        }
        Ok(())
    }
}
