//! Conservative canonical projection: a deserialized marker cannot hide a reader.
use super::{Expr, Statement};
use std::cell::Cell;
thread_local! { static SAFE: Cell<Option<bool>> = const { Cell::new(None) }; }
pub(super) struct Scope(Option<bool>);
impl Drop for Scope {
    fn drop(&mut self) {
        SAFE.with(|slot| slot.set(self.0));
    }
}
pub(super) fn scope(root: &Expr) -> Scope {
    let old = SAFE.with(Cell::get);
    if old.is_none() {
        SAFE.with(|slot| {
            slot.set(Some(
                crate::ast::expression_statement::projection_is_hygienic(
                    root,
                    crate::bounded_output::reserve_active_required,
                ),
            ))
        });
    }
    Scope(old)
}
pub(super) fn function_scope(root: &Expr, params: &[crate::ast::Param]) -> Scope {
    let old = SAFE.with(Cell::get);
    if old.is_none() {
        SAFE.with(|slot| {
            slot.set(Some(
                crate::ast::expression_statement::projection_with_bindings(
                    root,
                    params.iter().map(|p| p.name.as_str()),
                    crate::bounded_output::reserve_active_required,
                ),
            ))
        });
    }
    Scope(old)
}
pub(super) fn value(statement: &Statement) -> Option<&Expr> {
    SAFE.with(Cell::get)
        .unwrap_or(false)
        .then(|| crate::ast::expression_statement::value(statement))
        .flatten()
}
pub(super) fn write_let_prefix(output: &mut impl std::fmt::Write, statement: &Statement) {
    if value(statement).is_some() {
        return;
    }
    let Statement::Let {
        name,
        mutable,
        declared,
        ..
    } = statement
    else {
        unreachable!()
    };
    if *mutable {
        write!(output, "let mut {name}").unwrap();
    } else {
        write!(output, "let {name}").unwrap();
    }
    if let Some(ty) = declared {
        write!(output, ": ").unwrap();
        super::write_type(output, ty);
    }
    write!(output, " = ").unwrap();
}
