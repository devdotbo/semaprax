//! Immutable borrowed `Slice<u8>` traversal over the existing byte reads.
//!
//! `for byte in view` keeps the Copy-scalar `for` paths exactly: one
//! `byte_len` snapshot at `.value.s0`, an ordinary `usize` counter at `.s1`,
//! and the bounded loop at `.s2`. Each item is one *receiver*: the existing
//! `byte_get` result matched once, whose only arm is `Some`. The counter is
//! below the snapshot of an immutable, frozen view, so `None` is inconsistent;
//! it selects no arm and reaches every backend's existing exhaustive-match
//! invariant refusal. No byte is forged, defaulted, skipped or loaded
//! unchecked, and no buffer or view is materialized. This spelling carries no
//! authority: `hir::validation::byte_traversal` replays it independently.
//!
//! The shared paths construct the snapshot and receiver in place; no Vec
//! carrier is built and discarded. Raw AST has no source type, so the
//! receiver is outside the workspace `for` prebound: its two further
//! expression identities debit their own carriers as every `ExpressionId`
//! does, and [`receiver_backing`] debits every other byte it retains on the
//! active builder ledger before anything is constructed.
use super::*;
use crate::hir::{
    ResolvedMatchArm, ResolvedMatchMode, ResolvedMatchPattern, ResolvedMatchPatternField,
    SourceForKind,
};

const WRAPPER: &str = "#for-bytes";
const LENGTH: &str = "#for-bytes-length";
const PAYLOAD: &str = "#for-byte";
/// Receiver identities beside its root at `{path}`.
const SCRUTINEE: &str = ".scrutinee";
const VIEW_ARGUMENT: &str = ".scrutinee.arg.0";
const INDEX_ARGUMENT: &str = ".scrutinee.arg.1";
const PAYLOAD_BINDING: &str = ".arm.0.binding.0";
const ARM_VALUE: &str = ".arm.0.value";

/// Only an immutable named view of the exact compiler-owned `Slice<u8>` type.
pub(super) fn admits(source: &Binding) -> bool {
    source.ty == ResolvedType::SliceU8 && !source.mutable && source.traversal_row.is_none()
}

/// `None` is an unaddressable receiver spelling or a builder-ledger refusal;
/// a refusal is sticky on the active budget. Either way nothing of the
/// traversal was constructed.
pub(super) fn lower(
    function: &FunctionExecutionId,
    statement_path: &str,
    item_name: &str,
    item_span: Span,
    source: &ResolvedBinding,
    authored_body: ResolvedExpr,
    span: Span,
) -> Option<ResolvedStatement> {
    let path_len = statement_path
        .len()
        .checked_add(".value.s2.body.s0.value".len())?;
    if !crate::bounded_output::reserve_active_required(receiver_backing(function, path_len)?) {
        return None;
    }
    Some(lower_profile(
        function,
        statement_path,
        item_name,
        item_span,
        source,
        authored_body,
        span,
        Profile {
            wrapper: WRAPPER,
            length: LENGTH,
            length_callee: crate::byte_ops::LEN_ID,
            length_arguments: Vec::new(),
            item: ResolvedType::U8,
            kind: SourceForKind::BorrowBytes,
        },
        |path, counter| receiver(function, path, source, counter, item_span, span),
    ))
}

/// Exact requested backing one receiver at a `path_len`-byte path retains
/// beyond the shared carriers its five `ExpressionId` constructors debit
/// themselves: the five identity texts, the payload `ValueId`, its name, the
/// five declaration identities (`byte_get`, the `Option<u8>` result type, and
/// the matched variant, case and field), the result type's argument vector,
/// and the boxed scrutinee, argument, arm and field vectors. `None` means the
/// spelling is not addressable and nothing may be constructed.
pub(super) fn receiver_backing(function: &FunctionExecutionId, path_len: usize) -> Option<usize> {
    use crate::hir::ids::streamed::scoped_len;
    let expression =
        |suffix: &str| scoped_len(function, "expression", path_len.checked_add(suffix.len())?);
    let identities = [
        expression("")?,
        expression(SCRUTINEE)?,
        expression(VIEW_ARGUMENT)?,
        expression(INDEX_ARGUMENT)?,
        expression(ARM_VALUE)?,
    ];
    let payload_text = scoped_len(
        function,
        "value:local",
        path_len.checked_add(PAYLOAD_BINDING.len())?,
    )?;
    let payload = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::array::<u8>(payload_text).ok()?)
        .ok()?
        .0
        .pad_to_align()
        .size();
    let declarations = [
        crate::byte_ops::GET_ID.len(),
        crate::prelude::OPTION_ID.len(),
        crate::prelude::OPTION_ID.len(),
        crate::prelude::OPTION_SOME_ID.len(),
        crate::prelude::OPTION_SOME_VALUE_ID.len(),
    ];
    let structure = [
        std::mem::size_of::<ResolvedType>(),
        std::mem::size_of::<ResolvedExpr>(),
        2 * std::mem::size_of::<ResolvedExpr>(),
        std::mem::size_of::<ResolvedMatchArm>(),
        std::mem::size_of::<ResolvedMatchPatternField>(),
    ];
    identities
        .into_iter()
        .chain([payload, PAYLOAD.len()])
        .chain(declarations)
        .chain(structure)
        .try_fold(0usize, usize::checked_add)
}

/// One bounded byte read: `match byte_get(view, counter) { Some { value } => value }`.
fn receiver(
    function: &FunctionExecutionId,
    path: &str,
    view: &ResolvedBinding,
    counter: &ResolvedBinding,
    item_span: Span,
    span: Span,
) -> ResolvedExpr {
    let id = ExpressionId::new(function, path);
    let scrutinee_id = ExpressionId::new(function, &format!("{path}{SCRUTINEE}"));
    let view = place(function, &format!("{path}{VIEW_ARGUMENT}"), view, span);
    let index = place(function, &format!("{path}{INDEX_ARGUMENT}"), counter, span);
    let payload = ResolvedBinding {
        id: ValueId::local(function, &format!("{path}{PAYLOAD_BINDING}")),
        name: PAYLOAD.to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::U8,
        span: item_span,
    };
    let value = place(function, &format!("{path}{ARM_VALUE}"), &payload, span);
    let read = ResolvedExpr {
        id: scrutinee_id,
        ty: crate::byte_ops::ByteOp::Get.return_type(),
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(crate::byte_ops::GET_ID),
            type_arguments: Vec::new(),
            instance: None,
            args: vec![view, index],
        },
        span,
    };
    ResolvedExpr {
        id,
        ty: ResolvedType::U8,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Value,
            scrutinee: Box::new(read),
            arms: vec![ResolvedMatchArm {
                pattern: ResolvedMatchPattern::Variant {
                    variant: DeclarationId::new(crate::prelude::OPTION_ID),
                    case: DeclarationId::new(crate::prelude::OPTION_SOME_ID),
                    fields: vec![ResolvedMatchPatternField {
                        field: DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID),
                        binding: payload,
                    }],
                },
                guard: None,
                value,
                span,
            }],
        },
        span,
    }
}

#[cfg(test)]
mod tests;
