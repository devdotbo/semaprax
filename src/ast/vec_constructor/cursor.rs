//! Borrowed node-only traversal. Block statements and Match arms advance once.
use crate::ast::{Expr, ExprKind};
#[derive(Clone, Copy)]
pub(crate) struct NodeCursor<'a> {
    expression: &'a Expr,
    outer: usize,
    inner: usize,
}
impl<'a> NodeCursor<'a> {
    pub(crate) fn new(expression: &'a Expr) -> Self {
        Self {
            expression,
            outer: 0,
            inner: 0,
        }
    }
}
impl<'a> Iterator for NodeCursor<'a> {
    type Item = &'a Expr;
    fn next(&mut self) -> Option<Self::Item> {
        match &self.expression.kind {
            ExprKind::Block { statements, tail } => {
                while self.outer < statements.len() {
                    let statement = &statements[self.outer];
                    if let Some(child) = statement.child(self.inner) {
                        self.inner += 1;
                        return Some(child);
                    }
                    self.outer += 1;
                    self.inner = 0;
                }
                if self.outer == statements.len() {
                    self.outer += 1;
                    Some(tail)
                } else {
                    None
                }
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                if self.outer == 0 {
                    self.outer = 1;
                    return Some(scrutinee);
                }
                let arm = arms.get(self.outer - 1)?;
                if self.inner == 0 {
                    self.inner = 1;
                    if let Some(guard) = &arm.guard {
                        return Some(guard);
                    }
                }
                self.outer += 1;
                self.inner = 0;
                Some(&arm.value)
            }
            _ => {
                let child = self.expression.child(self.outer)?;
                self.outer += 1;
                Some(child)
            }
        }
    }
}
/// Three generated ancestors precede each original item: marker, Block, Push.
/// No generated chain exists until this fixed source-depth/work forecast passes.
pub(crate) fn items_fit_depth(items: &[Expr]) -> bool {
    let mut frames = [None; 128];
    let mut work = 0usize;
    for item in items {
        frames[0] = Some(NodeCursor::new(item));
        let mut length = 1usize;
        while length != 0 {
            work = match work.checked_add(1) {
                Some(n) if n <= 16 * 1024 * 1024 => n,
                _ => return false,
            };
            let cursor = frames[length - 1].as_mut().expect("item census frame");
            if let Some(child) = cursor.next() {
                if length + 3 >= 128 {
                    return false;
                }
                frames[length] = Some(NodeCursor::new(child));
                length += 1;
            } else {
                length -= 1;
                frames[length] = None;
            }
        }
    }
    true
}
