//! Validation-only immutable result views; derived signatures keep owning storage.
use super::*;

pub(super) type ResultType<'a> = std::borrow::Cow<'a, ResolvedType>;

#[cfg(test)]
pub(super) fn owned_capacity(result: &ResultType<'_>) -> usize {
    match result {
        ResultType::Borrowed(_) => 0,
        ResultType::Owned(ty) => resolved_type_owned_capacity(ty),
    }
}

#[cfg(test)]
mod tests;
