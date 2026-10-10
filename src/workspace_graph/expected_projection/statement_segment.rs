//! Retained HIR projection segments for authored statement children.

pub(in crate::workspace_graph) fn child(
    statement: &crate::ast::Statement,
    index: usize,
) -> &'static str {
    match (statement, index) {
        (crate::ast::Statement::While { .. }, 0) => "condition",
        (crate::ast::Statement::While { .. }, _) => "body",
        (crate::ast::Statement::For { .. } | crate::ast::Statement::ForBorrow { .. }, 0) => {
            "value.s0.value.arg.0"
        }
        (crate::ast::Statement::For { .. } | crate::ast::Statement::ForBorrow { .. }, _) => {
            "value.s2.body.s1.value"
        }
        (crate::ast::Statement::ForOwn { .. }, 0) => "value.s0.value.arg.0",
        (crate::ast::Statement::ForOwn { .. }, _) => "value.s1.body.s0.value.arm.1.value.s0.value",
        _ => "value",
    }
}
