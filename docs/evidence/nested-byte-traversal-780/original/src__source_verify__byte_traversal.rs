//! Source twin of immutable borrowed `Slice<u8>` traversal admission.
//!
//! `for byte in view` admits one simple immutable `Slice<u8>` binding and a
//! fresh immutable Copy `u8` item. The traversal freezes the view's
//! authenticated root for its whole lexical extent under its own loan. The
//! view's alias loan may expire by last use inside a body that never names
//! the view, so a source-name scan alone cannot keep the owner frozen while
//! the next iteration still reads through the alias.
use super::binding::{Binding, SourceLoan, SourceLoanId};
use crate::ast::{Span, Type};
use std::collections::HashMap;

/// The item type of an admitted byte traversal source.
pub(super) fn element(ty: &Type) -> Option<Type> {
    (*ty == Type::SliceU8).then_some(Type::U8)
}

/// One freeze per traversal statement, spanning its source through its body.
pub(super) fn freeze_id(item: &str, values: Span, body: Span) -> SourceLoanId {
    SourceLoanId {
        borrower: format!("for byte traversal `{item}`"),
        start: values.start,
        end: body.end,
    }
}

/// Freezes the whole or subrange root behind `source`. A parameter view has
/// no local owner; its caller's loan already spans this invocation.
pub(super) fn freeze(bindings: &mut HashMap<String, Binding>, source: &str, id: &SourceLoanId) {
    let Some(origin) = bindings
        .get(source)
        .and_then(|binding| binding.borrow_origin.clone())
    else {
        return;
    };
    if let Some(owner) = bindings.get_mut(&origin.root) {
        owner.active_loans.insert(SourceLoan {
            id: id.clone(),
            projections: origin.projections,
        });
    }
}

/// Releases exactly this traversal's freeze, wherever its root binding lives.
pub(super) fn release(bindings: &mut HashMap<String, Binding>, id: &SourceLoanId) {
    for binding in bindings.values_mut() {
        binding.active_loans.retain(|loan| &loan.id != id);
    }
}
