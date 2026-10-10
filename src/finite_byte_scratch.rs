//! Independent authority for the closed finite owned-iterator scratch profile.
//!
//! Neither cleanup certificates nor a Vec's physical slot ceiling supplies an
//! iteration bound. Source and HIR recover the successful immutable constructor
//! chain and separately authenticate all local allocation sites and their uses.
mod resolved;
mod source;
pub(crate) use resolved::{resolved, resolved_function, ResolvedScratch};
pub(crate) use source::source;
#[cfg(test)]
pub(crate) use source::SourceScratch;
const LIMIT: usize = crate::byte_data_capacity::MAX_BYTES_COPY_SITES as usize;
const WORK: usize = 65_536;
const DEPTH: usize = crate::cache_codec::MAX_DEPTH;
#[derive(Clone, Copy)]
struct Length {
    length: u32,
}

pub(crate) fn loops(
    function: &crate::hir::ResolvedFunction,
) -> impl Iterator<Item = (&crate::hir::ResolvedExpr, &crate::hir::ResolvedExpr)> {
    use crate::hir::{ResolvedExprKind as K, ResolvedStatement as S};
    let statements = match &function.body.kind {
        K::Block { statements, .. } => statements.as_slice(),
        _ => &[],
    };
    statements.iter().filter_map(|statement| {
        let S::Let { value, .. } = statement else {
            return None;
        };
        let K::Block { statements, .. } = &value.kind else {
            return None;
        };
        match statements.last()? {
            S::While {
                condition, body, ..
            }
            | S::LexicalWhile {
                condition, body, ..
            } => Some((condition.as_ref(), body.as_ref())),
            _ => None,
        }
    })
}
pub(crate) fn function_requires(function: &crate::hir::ResolvedFunction) -> bool {
    loops(function).any(|(condition, body)| resolved_function(function, condition, body).is_some())
}
#[cfg(test)]
mod tests;
