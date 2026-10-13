//! Complete production receiver from eb02a89b6174bced05e02f6ede2ebcda55cd5893.
//! Function source SHA-256: 3872ec7000f975255e93706a296461ed89c76ae3abad6428b32627e5718f1ce6
use super::*;

#[rustfmt::skip]
fn classify_nested_owned_byte_record(
    declarations: &DeclarationIndex,
    root: &ResolvedType,
) -> NestedOwnedRecordAdmission {
    // These root branches need neither an owned clone nor a traversal cursor.
    // Preserve the exact profile result; nominal roots retain the complete walk.
    match root {
        ResolvedType::Bytes => {
            return NestedOwnedRecordAdmission::Admitted(NestedOwnedRecordFacts {
                byte_leaves: 1,
                visited_fields: 0,
            })
        }
        ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => return NestedOwnedRecordAdmission::NoOwnedBytes,
        ResolvedType::Never
        | ResolvedType::Unit
        | ResolvedType::ArrayU8(_)
        | ResolvedType::String
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::StringMap
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::MutFunctionI64
        | ResolvedType::Function { .. } => return NestedOwnedRecordAdmission::OutsideProfile,
        ResolvedType::Nominal { .. } => {}
    }
    enum Frame<'a> {
        Type(ResolvedType, usize),
        Fields(
            DeclarationId,
            &'a [ResolvedFieldDeclaration],
            Vec<ResolvedType>,
            usize,
            usize,
        ),
        LeaveRecord,
    }

    let mut frames = vec![Frame::Type(root.clone(), 1)];
    let mut active = Vec::new();
    let mut byte_leaves = 0usize;
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
                byte_leaves += 1;
                if byte_leaves > MAX_NESTED_OWNED_BYTE_LEAVES {
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
                };
                // Structural equality is the complete length-delimited key
                // equality, without constructing text or tree nodes. The
                // active path is bounded by the unchanged depth limit.
                if active.contains(&identity) {
                    return NestedOwnedRecordAdmission::Recursive;
                }
                active.push(identity);
                frames.push(Frame::LeaveRecord);
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
                let Ok(field_ty) = substitute_type(&field.ty, &declaration, &arguments) else {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                };
                frames.push(Frame::Type(field_ty, depth + 1));
            }
            Frame::LeaveRecord => {
                active.pop();
            }
        }
    }

    if byte_leaves == 0 && saw_generic_instance {
        NestedOwnedRecordAdmission::OutsideProfile
    } else if byte_leaves == 0 {
        NestedOwnedRecordAdmission::NoOwnedBytes
    } else {
        NestedOwnedRecordAdmission::Admitted(NestedOwnedRecordFacts {
            byte_leaves,
            visited_fields,
        })
    }
}

pub(super) fn summary(index: &DeclarationIndex, root: &ResolvedType) -> (u8, usize, usize) {
    super::nominal_tests::summary(classify_nested_owned_byte_record(index, root))
}

#[rustfmt::skip]
pub(crate) fn is_admitted_nested_owned_byte_record(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    match classify_nested_owned_byte_record(declarations, ty) {
        NestedOwnedRecordAdmission::Admitted(facts) => {
            facts.byte_leaves != 0 && facts.visited_fields <= MAX_NESTED_OWNED_RECORD_FIELDS
        }
        NestedOwnedRecordAdmission::NoOwnedBytes
        | NestedOwnedRecordAdmission::OutsideProfile
        | NestedOwnedRecordAdmission::Recursive
        | NestedOwnedRecordAdmission::LimitExceeded => false,
    }
}

#[rustfmt::skip]
pub(super) fn record_args_ok(
    declarations: &DeclarationIndex,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> bool {
    arguments
        .iter()
        .all(|argument| matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
        || is_admitted_nested_owned_byte_record(
            declarations,
            &ResolvedType::Nominal {
                declaration: declaration.clone(),
                arguments: arguments.to_vec(),
            },
        )
}
