//! Complete per-invocation compiler suffix catalog, in exact source order.
//! No authored schema, process cache, inferred lookup authority, or base work.
use super::*;
use std::mem::size_of;

#[derive(Debug)]
pub(crate) struct VectorExtension {
    pub(super) values: [Option<ResolvedTypeDeclaration>; 9],
}
impl VectorExtension {
    pub(super) fn empty() -> Self {
        Self {
            values: std::array::from_fn(|_| None),
        }
    }
    pub(super) fn text<P: Payer>(value: &str, payer: &mut P) -> Result<String, Failure<P::Error>> {
        // Separate actual helper initializer/conversion/receiving carriers;
        // String backing and its copy/validation work stay with paid::string.
        payer
            .bytes(
                size_of::<String>()
                    + size_of::<Vec<u8>>()
                    + size_of::<Result<String, std::string::FromUtf8Error>>()
                    + size_of::<Result<String, Failure<P::Error>>>(),
            )
            .map_err(Failure::Payer)?;
        paid::string(value, payer)
    }
    #[cfg(test)]
    pub(super) fn acquire<P: Payer>(payer: &mut P) -> Result<Self, Failure<P::Error>> {
        payer
            .bytes(
                2 * size_of::<Self>()
                    + size_of::<Option<Self>>()
                    + size_of::<Result<Self, Failure<P::Error>>>(),
            )
            .map_err(Failure::Payer)?;
        let mut value = Self::empty();
        value.values[0] = Some(super::extension_producer::acquire(0, &value.values, payer)?);
        Ok(value)
    }
    pub(crate) fn for_source<'a>(
        value: Option<&'a Self>,
        prelude: &[crate::ast::TypeDeclaration],
        source: &crate::ast::TypeDeclaration,
    ) -> Option<&'a ResolvedTypeDeclaration> {
        let current = crate::prelude::declarations();
        if !matches!(prelude.len(), 3 | 4 | 6 | 8 | 9 | 11)
            || !std::ptr::eq(prelude, &current[..prelude.len()])
        {
            return None;
        }
        let slot = prelude[2..]
            .iter()
            .position(|item| std::ptr::eq(item, source))?;
        value?.values[slot].as_ref()
    }
}

#[cfg(test)]
#[path = "vector_extension/tests.rs"]
mod tests;
