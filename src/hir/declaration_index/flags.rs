//! Flag-only receiver; the owned layout/fact API remains the independent oracle.
use super::{DeclarationIndex, ResolvedType, TypeFacts};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TypeFlags {
    pub(crate) copy: bool,
    pub(crate) contains_resource: bool,
    pub(crate) sized: bool,
    pub(crate) needs_drop: bool,
}
impl From<&TypeFacts> for TypeFlags {
    fn from(facts: &TypeFacts) -> Self {
        Self {
            copy: facts.copy,
            contains_resource: facts.contains_resource,
            sized: facts.sized,
            needs_drop: facts.needs_drop,
        }
    }
}
impl DeclarationIndex {
    /// Preserve current-index cache precedence, including hostile cached facts.
    /// Dynamic identities retain the old key traversal and miss computation.
    /// A cache hit borrows the facts instead of cloning their layout payload.
    pub(crate) fn type_flags(&self, ty: &ResolvedType) -> Option<TypeFlags> {
        let key = match ty {
            ResolvedType::Never => "never:lexical-exit:v1",
            ResolvedType::Unit => "unit",
            ResolvedType::I64 => "i64",
            ResolvedType::I32 => "i32",
            ResolvedType::Char => "char",
            ResolvedType::U8 => "u8",
            ResolvedType::Usize => "usize",
            ResolvedType::F32 => "f32",
            ResolvedType::F64 => "f64",
            ResolvedType::Bool => "bool",
            ResolvedType::String => "string",
            ResolvedType::Bytes => "bytes",
            ResolvedType::Str => "str",
            ResolvedType::SliceU8 => "slice-u8",
            ResolvedType::StringMap => "map:string:i64:v1",
            ResolvedType::OnceFunction => "fn-once:bytes:i64:v1",
            ResolvedType::OnceFunctionI64 => "fn-once:bytes+i64:i64:v2",
            ResolvedType::OnceFunctionI64Pair => "fn-once:bytes+i64+i64:i64:v3",
            ResolvedType::MutFunctionI64 => "fn-mut:i64+i64:i64:v1",
            _ => {
                let key = ty.identity_key();
                return self
                    .type_facts_by_id
                    .get(&key)
                    .map(TypeFlags::from)
                    .or_else(|| self.recompute_type_facts(ty).as_ref().map(TypeFlags::from));
            }
        };
        if let Some(facts) = self.type_facts_by_id.get(key) {
            return Some(TypeFlags::from(facts));
        }
        // These are exactly compute_type_facts' leaf branches after cache miss.
        let (copy, needs_drop) = match ty {
            ResolvedType::Never => return None,
            ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool => (true, false),
            ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::StringMap
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair => (false, true),
            ResolvedType::Str | ResolvedType::SliceU8 | ResolvedType::MutFunctionI64 => {
                (false, false)
            }
            _ => unreachable!("only static leaf identities reach leaf facts"),
        };
        Some(TypeFlags {
            copy,
            contains_resource: false,
            sized: true,
            needs_drop,
        })
    }
}
#[cfg(test)]
mod tests;
