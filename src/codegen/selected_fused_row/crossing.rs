//! Frozen public ABI crossing refusal, before any lowering or emitted text.
//!
//! The selected function must have no parameters, effects, contracts or
//! yields, return `i64`, and contain no Bytes-typed expression other than its
//! single proven producer, no callable/closure/host/import value, and no call
//! outside the closed selected operations. A private carrier therefore never
//! meets the 16-byte public Bytes ABI, a provider, a callable or a host.
use super::{error, selection::Sites};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExprKind as K, ResolvedType as T};

const CLOSED_CALLS: [&str; 9] = [
    crate::vec_ops::WITH_CAPACITY_ID,
    crate::vec_ops::PUSH_ID,
    crate::vec_ops::LEN_ID,
    crate::vec_ops::SORT_OWNED_ID,
    crate::byte_ops::ZEROED_ID,
    crate::byte_ops::LEN_ID,
    crate::byte_ops::GET_ID,
    crate::str_ops::LEN_BYTES_ID,
    crate::string_ops::I64_FROM_USIZE_ID,
];
/// Walk bound; the closed census bounds the real shape far below this.
const WORK: usize = 1 << 16;

pub(super) fn refuse_crossings(sites: &Sites<'_>) -> Result<(), Diagnostic> {
    let function = sites.function;
    if !function.params.is_empty()
        || function.return_type != T::I64
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return Err(error(
            "selected function signature crosses a public boundary",
        ));
    }
    let mut pending = vec![&function.body];
    let mut work = 0usize;
    while let Some(expression) = pending.pop() {
        work += 1;
        if work > WORK {
            return Err(error("selected function exceeds the crossing scan bound"));
        }
        if expression.ty == T::Bytes && !std::ptr::eq(expression, sites.producer) {
            return Err(error(
                "a Bytes value other than the proven producer would cross the public ABI",
            ));
        }
        match &expression.kind {
            K::Call {
                callee, instance, ..
            } if instance.is_none() && CLOSED_CALLS.contains(&callee.as_str()) => {}
            K::Call { .. }
            | K::Invoke { .. }
            | K::Closure { .. }
            | K::FunctionReference { .. }
            | K::NativeRustImportCall(_)
            | K::HostCommandCall(_)
            | K::LexicalExit { .. }
            | K::Yield { .. }
            | K::Try { .. }
            | K::TryOption { .. } => {
                return Err(error(
                    "a call, callable or exit outside the closed selected domain",
                ));
            }
            _ => {}
        }
        if matches!(callee_of(expression), Some(id) if id == crate::byte_ops::ZEROED_ID)
            && !std::ptr::eq(expression, sites.producer)
        {
            return Err(error("a second Bytes producer outside the proven site"));
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(())
}

fn callee_of(expression: &crate::hir::ResolvedExpr) -> Option<&str> {
    match &expression.kind {
        K::Call { callee, .. } => Some(callee.as_str()),
        _ => None,
    }
}
