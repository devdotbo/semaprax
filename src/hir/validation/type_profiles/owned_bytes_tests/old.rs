//! Saved complete clone-based query, independent of the borrowed implementation.
use super::*;

pub(super) fn query(program: &ResolvedProgram, ty: &ResolvedType) -> bool {
    let mut pending = vec![ty.clone()];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::Function { .. } => {}
            ResolvedType::Bytes
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair => return true,
            ResolvedType::MutFunctionI64 => {}
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                pending.extend(arguments);
                if !visited.insert(declaration.clone()) {
                    continue;
                }
                let Some(item) = program
                    .types
                    .iter()
                    .find(|candidate| candidate.id == declaration)
                else {
                    continue;
                };
                match &item.kind {
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        pending.extend(fields.iter().map(|field| field.ty.clone()));
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => pending.extend(
                        cases
                            .iter()
                            .flat_map(|case| &case.fields)
                            .map(|field| field.ty.clone()),
                    ),
                    ResolvedTypeDeclarationKind::Resource { .. } => {}
                }
            }
            ResolvedType::Never
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
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::StringMap
            | ResolvedType::TypeParameter { .. } => {}
        }
    }
    false
}
