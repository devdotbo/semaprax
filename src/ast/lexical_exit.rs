//! Authored unlabeled lexical exit kinds; target authority belongs to HIR replay.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LexicalExitKind {
    Break,
    Continue,
    Return,
}
impl LexicalExitKind {
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Break => "break",
            Self::Continue => "continue",
            Self::Return => "return",
        }
    }
}

/// Structural reachability only; type/ownership/target checks remain independent.
pub fn has_no_normal_completion(expr: &super::Expr) -> bool {
    fn visit(expr: &super::Expr, depth: usize) -> bool {
        if depth >= 128 {
            return false;
        }
        match &expr.kind {
            super::ExprKind::LexicalExit { .. } => true,
            super::ExprKind::Block { tail, .. } => visit(tail, depth + 1),
            super::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => visit(then_branch, depth + 1) && visit(else_branch, depth + 1),
            super::ExprKind::Match { arms, .. } => {
                !arms.is_empty() && arms.iter().all(|arm| visit(&arm.value, depth + 1))
            }
            _ => false,
        }
    }
    visit(expr, 0)
}

/// A terminating body whose completed routes all leave the function. A nested
/// loop exit does not qualify: it can still reach the enclosing continuation.
pub(crate) fn returns_only(expr: &super::Expr) -> bool {
    fn visit(expr: &super::Expr, depth: usize) -> bool {
        if depth > crate::cache_codec::MAX_DEPTH {
            return false;
        }
        match &expr.kind {
            super::ExprKind::LexicalExit {
                kind: LexicalExitKind::Return,
                ..
            } => true,
            super::ExprKind::Block { tail, .. } => visit(tail, depth + 1),
            super::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => visit(then_branch, depth + 1) && visit(else_branch, depth + 1),
            super::ExprKind::Match { arms, .. } => {
                !arms.is_empty() && arms.iter().all(|arm| visit(&arm.value, depth + 1))
            }
            _ => false,
        }
    }
    visit(expr, 0)
}
