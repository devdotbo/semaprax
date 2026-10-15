use super::*;
pub(super) fn text(
    root: &ResolvedType,
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if !matches!(root, ResolvedType::Nominal { .. }) {
        return Ok(false);
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType, usize),
        Leave,
    }
    let mut pending = Vec::new();
    budget.reserve_vec(function, &mut pending, 1, "text admission root")?;
    pending.push(Frame::Enter(root, 1));
    let mut active = Vec::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut text = false;
    while let Some(frame) = pending.pop() {
        budget.charge(function, 1, "text admission frame")?;
        match frame {
            Frame::Enter(ty, _) if crate::map_ops::is_collection(ty) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String, _) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::Bytes, _) => {
                leaves += 1;
            }
            Frame::Enter(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::Char
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => {}
            Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return Ok(false);
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = ty
                else {
                    unreachable!()
                };
                if declarations.has_record_invariant(declaration)
                    || !arguments.is_empty()
                    || declarations
                        .type_parameters(declaration)
                        .is_none_or(|p| !p.is_empty())
                    || declarations
                        .declaration(declaration)
                        .is_none_or(|d| d.kind != DeclarationKind::Record)
                {
                    return Ok(false);
                }
                let Some(declared) = declarations.record_fields(declaration) else {
                    return Ok(false);
                };
                // Same complete structural identity; only the active path
                // participates in cycle detection, in the original order.
                if active.contains(&ty) {
                    return Ok(false);
                }
                budget.reserve_vec(function, &mut active, 1, "text admission active types")?;
                active.push(ty);
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Ok(false);
                }
                budget.reserve_vec(
                    function,
                    &mut pending,
                    declared.len().saturating_add(1),
                    "text admission child frames",
                )?;
                pending.push(Frame::Leave);
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(&field.ty, depth + 1)),
                );
            }
            Frame::Leave => {
                active.pop();
            }
            Frame::Enter(_, _) => return Ok(false),
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return Ok(false);
        }
    }
    Ok(text)
}
