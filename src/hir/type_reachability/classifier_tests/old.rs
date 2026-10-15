//! Saved pre-reduction classifiers. No production classifier delegation.
use super::super::*;

struct NestedOwnedRecordFacts {
    /// Declaration-stable paths to every owned byte leaf, in declaration
    /// order. HIR admission therefore never depends on display names.
    byte_paths: Vec<Vec<PlaceProjection>>,
    visited_fields: usize,
}

enum NestedOwnedRecordAdmission {
    Admitted(NestedOwnedRecordFacts),
    NoOwnedBytes,
    OutsideProfile,
    Recursive,
    LimitExceeded,
}

fn classify_nested_owned_byte_record(
    declarations: &DeclarationIndex,
    root: &ResolvedType,
) -> NestedOwnedRecordAdmission {
    enum Frame<'a> {
        Type(ResolvedType, usize),
        Fields(
            DeclarationId,
            &'a [ResolvedFieldDeclaration],
            Vec<ResolvedType>,
            usize,
            usize,
        ),
        LeaveRecord(String),
        LeaveField,
    }

    let mut frames = vec![Frame::Type(root.clone(), 1)];
    let mut active = BTreeSet::new();
    let mut path = Vec::new();
    let mut byte_paths = Vec::new();
    let mut visited_fields = 0usize;
    let mut saw_generic_instance = false;

    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Type(
                ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
                | ResolvedType::Function { .. },
                _,
            ) => return NestedOwnedRecordAdmission::OutsideProfile,
            Frame::Type(ResolvedType::Bytes, _) => {
                byte_paths.push(path.clone());
                if byte_paths.len() > MAX_NESTED_OWNED_BYTE_LEAVES {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
            }
            Frame::Type(ref ty, _) if nested_record_copy_scalar_is_admitted(ty) => {}
            Frame::Type(
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                },
                depth,
            ) => {
                if depth > MAX_NESTED_OWNED_RECORD_DEPTH {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
                saw_generic_instance |= !arguments.is_empty();
                if declarations
                    .type_parameters(&declaration)
                    .is_none_or(|parameters| parameters.len() != arguments.len())
                    || arguments.iter().any(|argument| {
                        *argument != ResolvedType::Bytes
                            && !nested_record_copy_scalar_is_admitted(argument)
                            && !matches!(argument, ResolvedType::Nominal { .. })
                    })
                    || declarations
                        .declaration(&declaration)
                        .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                }
                let Some(fields) = declarations.record_fields(&declaration) else {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                };
                let identity = ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments: arguments.clone(),
                }
                .identity_key();
                if !active.insert(identity.clone()) {
                    return NestedOwnedRecordAdmission::Recursive;
                }
                frames.push(Frame::LeaveRecord(identity));
                frames.push(Frame::Fields(declaration, fields, arguments, 0, depth));
            }
            Frame::Type(
                ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::ArrayU8(_)
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap
                | ResolvedType::TypeParameter { .. },
                _,
            ) => return NestedOwnedRecordAdmission::OutsideProfile,
            Frame::Type(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => unreachable!("admitted scalar handled above"),
            Frame::Fields(declaration, fields, arguments, index, depth) => {
                let Some(field) = fields.get(index) else {
                    continue;
                };
                visited_fields += 1;
                if visited_fields > MAX_NESTED_OWNED_RECORD_FIELDS {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
                frames.push(Frame::Fields(
                    declaration.clone(),
                    fields,
                    arguments.clone(),
                    index + 1,
                    depth,
                ));
                path.push(PlaceProjection::Field(field.id.clone()));
                frames.push(Frame::LeaveField);
                let Ok(field_ty) = substitute_type(&field.ty, &declaration, &arguments) else {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                };
                frames.push(Frame::Type(field_ty, depth + 1));
            }
            Frame::LeaveRecord(identity) => {
                active.remove(&identity);
            }
            Frame::LeaveField => {
                path.pop();
            }
        }
    }

    if byte_paths.is_empty() && saw_generic_instance {
        NestedOwnedRecordAdmission::OutsideProfile
    } else if byte_paths.is_empty() {
        NestedOwnedRecordAdmission::NoOwnedBytes
    } else {
        NestedOwnedRecordAdmission::Admitted(NestedOwnedRecordFacts {
            byte_paths,
            visited_fields,
        })
    }
}

pub(crate) fn is_admitted_nested_owned_byte_record(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    match classify_nested_owned_byte_record(declarations, ty) {
        NestedOwnedRecordAdmission::Admitted(facts) => {
            !facts.byte_paths.is_empty() && facts.visited_fields <= MAX_NESTED_OWNED_RECORD_FIELDS
        }
        NestedOwnedRecordAdmission::NoOwnedBytes
        | NestedOwnedRecordAdmission::OutsideProfile
        | NestedOwnedRecordAdmission::Recursive
        | NestedOwnedRecordAdmission::LimitExceeded => false,
    }
}

pub(crate) fn is_flat_owned_byte_record(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if declarations
        .declaration(declaration)
        .is_none_or(|item| item.kind != DeclarationKind::Record)
    {
        return false;
    }
    let Some(parameters) = declarations.type_parameters(declaration) else {
        return false;
    };
    if arguments.len() != parameters.len() {
        return false;
    }
    if !arguments.is_empty()
        && arguments.iter().any(|argument| {
            *argument != ResolvedType::Bytes && !nested_record_copy_scalar_is_admitted(argument)
        })
    {
        return false;
    }
    declarations
        .record_fields(declaration)
        .and_then(|fields| {
            fields
                .iter()
                .map(|field| substitute_type(&field.ty, declaration, arguments).ok())
                .collect::<Option<Vec<_>>>()
        })
        .is_some_and(|fields| {
            fields.contains(&ResolvedType::Bytes)
                && fields.iter().all(|field| {
                    *field == ResolvedType::Bytes || nested_record_copy_scalar_is_admitted(field)
                })
        })
}

pub(super) fn summary(index: &DeclarationIndex, ty: &ResolvedType) -> (u8, usize, usize) {
    match classify_nested_owned_byte_record(index, ty) {
        NestedOwnedRecordAdmission::Admitted(f) => (0, f.byte_paths.len(), f.visited_fields),
        NestedOwnedRecordAdmission::NoOwnedBytes => (1, 0, 0),
        NestedOwnedRecordAdmission::OutsideProfile => (2, 0, 0),
        NestedOwnedRecordAdmission::Recursive => (3, 0, 0),
        NestedOwnedRecordAdmission::LimitExceeded => (4, 0, 0),
    }
}
