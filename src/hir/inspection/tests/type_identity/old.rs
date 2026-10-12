//! Saved allocating identity audit, independent of the leaf fast path.
use super::*;
pub(super) fn audit_resolved_type(root: &ResolvedType) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::Function { parameters, result } => {
                pending.extend(parameters);
                pending.push(result);
            }
            ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
            | ResolvedType::Never
            | ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::StringMap => {}
            ResolvedType::TypeParameter { owner, .. } => {
                reject_nul_identity("resolved type-parameter owner", owner.as_str())?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                reject_nul_identity("resolved nominal type", declaration.as_str())?;
                pending.extend(arguments);
            }
        }
    }
    Ok(())
}
