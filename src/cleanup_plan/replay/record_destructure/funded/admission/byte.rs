use super::*;
pub(super) fn byte_head<'a>(
    declarations: &'a DeclarationIndex,
    declaration: &'a DeclarationId,
    arguments: &'a [ResolvedType],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    use std::borrow::Cow;

    #[derive(PartialEq)]
    struct Head<'a> {
        declaration: Cow<'a, DeclarationId>,
        arguments: Cow<'a, [ResolvedType]>,
    }

    enum Frame<'a> {
        Type(ResolvedType, usize),
        Nominal(Head<'a>, usize),
        Fields(&'a [ResolvedFieldDeclaration], usize, usize),
        LeaveRecord,
    }

    let mut frames = Vec::new();
    budget.reserve_vec(function, &mut frames, 1, "byte admission root frames")?;
    frames.push(Frame::Nominal(
        Head {
            declaration: Cow::Borrowed(declaration),
            arguments: Cow::Borrowed(arguments),
        },
        1,
    ));
    let mut active = Vec::new();
    let mut byte_leaves = 0usize;
    let mut visited_fields = 0usize;
    let mut saw_generic_instance = false;

    while let Some(frame) = frames.pop() {
        budget.charge(function, 1, "byte admission frame")?;
        match frame {
            Frame::Type(
                ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
                | ResolvedType::Function { .. },
                _,
            ) => return Ok(false),
            Frame::Type(ResolvedType::Bytes, _) => {
                byte_leaves += 1;
                if byte_leaves > MAX_NESTED_OWNED_BYTE_LEAVES {
                    return Ok(false);
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
                budget.reserve_vec(function, &mut frames, 1, "byte admission nominal frames")?;
                frames.push(Frame::Nominal(
                    Head {
                        declaration: Cow::Owned(declaration),
                        arguments: Cow::Owned(arguments),
                    },
                    depth,
                ));
            }
            Frame::Nominal(head, depth) => {
                let declaration = head.declaration.as_ref();
                let arguments = head.arguments.as_ref();
                if depth > MAX_NESTED_OWNED_RECORD_DEPTH {
                    return Ok(false);
                }
                saw_generic_instance |= !arguments.is_empty();
                if declarations
                    .type_parameters(declaration)
                    .is_none_or(|parameters| parameters.len() != arguments.len())
                    || arguments.iter().any(|argument| {
                        *argument != ResolvedType::Bytes
                            && !nested_record_copy_scalar_is_admitted(argument)
                            && !matches!(argument, ResolvedType::Nominal { .. })
                    })
                    || declarations
                        .declaration(declaration)
                        .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Ok(false);
                }
                let Some(fields) = declarations.record_fields(declaration) else {
                    return Ok(false);
                };
                // Complete structural nominal equality includes every argument tree.
                // Retain the environment once until its LeaveRecord continuation.
                if active.contains(&head) {
                    return Ok(false);
                }
                budget.reserve_vec(function, &mut active, 1, "byte admission active heads")?;
                active.push(head);
                budget.reserve_vec(
                    function,
                    &mut frames,
                    2,
                    "byte admission record continuations",
                )?;
                frames.push(Frame::LeaveRecord);
                frames.push(Frame::Fields(fields, 0, depth));
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
            ) => return Ok(false),
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
            Frame::Fields(fields, index, depth) => {
                let Some(field) = fields.get(index) else {
                    continue;
                };
                visited_fields += 1;
                if visited_fields > MAX_NESTED_OWNED_RECORD_FIELDS {
                    return Ok(false);
                }
                let head = active
                    .last()
                    .expect("field continuation has an active record");
                let field_ty = match crate::cleanup_plan::replay::ordinary_inventory::substitute::substitute_type(
                    &field.ty, head.declaration.as_ref(), head.arguments.as_ref(), function, budget,
                ) {
                    Ok(ty) => ty,
                    Err(error) if budget.refusal_spent || budget.oracle_lease().is_some_and(|lease| lease.refused()) => return Err(error),
                    Err(_) => return Ok(false),
                };
                budget.reserve_vec(
                    function,
                    &mut frames,
                    2,
                    "byte admission field continuations",
                )?;
                frames.push(Frame::Fields(fields, index + 1, depth));
                frames.push(Frame::Type(field_ty, depth + 1));
            }
            Frame::LeaveRecord => {
                active.pop();
            }
        }
    }

    // All original negative categories remain false; generic zero-leaf
    // closure and ordinary zero-leaf closure have the same public bool result.
    let _ = saw_generic_instance;
    Ok(byte_leaves != 0 && visited_fields <= MAX_NESTED_OWNED_RECORD_FIELDS)
}
