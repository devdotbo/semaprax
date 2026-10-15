//! Exact borrowed comparison with the complete ordinary allocating result carrier.
use super::{DeclarationIndex, ResolvedImportResultKind, ResolvedType};

fn nominal<'a>(
    index: &DeclarationIndex,
    name: &str,
    ty: &'a ResolvedType,
) -> Option<&'a [ResolvedType]> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    (index.type_id(name) == Some(declaration)).then_some(arguments.as_slice())
}
impl ResolvedImportResultKind {
    pub(crate) fn matches_value_type(&self, index: &DeclarationIndex, ty: &ResolvedType) -> bool {
        match self {
            Self::Unit => ty == &ResolvedType::Unit,
            Self::I64 => ty == &ResolvedType::I64,
            Self::Bool => ty == &ResolvedType::Bool,
            Self::OwnedString => ty == &ResolvedType::String,
            Self::BorrowedStr { .. } => ty == &ResolvedType::Str,
            Self::ResultI64I64 => matches!(
                nominal(index, "Result", ty),
                Some([ResolvedType::I64, ResolvedType::I64])
            ),
            Self::OwnedOptionString => {
                matches!(nominal(index, "Option", ty), Some([ResolvedType::String]))
            }
            Self::OwnedResultStringI64 => matches!(
                nominal(index, "Result", ty),
                Some([ResolvedType::String, ResolvedType::I64])
            ),
            Self::OwnedResultStringOptionI64 => {
                matches!(nominal(index,"Result",ty), Some([ResolvedType::String,option]) if matches!(nominal(index,"Option",option), Some([ResolvedType::I64])))
            }
            Self::OwnedResultResourceI64 { resource } => {
                matches!(nominal(index,"Result",ty), Some([ResolvedType::Nominal { declaration,arguments },ResolvedType::I64]) if declaration == resource && arguments.is_empty())
            }
            Self::OwnedResource { resource } => {
                matches!(ty,ResolvedType::Nominal { declaration,arguments } if declaration == resource && arguments.is_empty())
            }
        }
    }
}

#[cfg(test)]
mod tests;
