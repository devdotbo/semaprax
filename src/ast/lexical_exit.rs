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
    use super::{Expr, ExprKind as K, Statement as S};
    const NORMAL: u8 = 1;
    const RETURN: u8 = 2;
    const ESCAPE: u8 = 4;
    const LOCAL: u8 = 8;
    fn sequence(left: u8, right: impl FnOnce() -> u8) -> u8 {
        (left & !NORMAL) | if left & NORMAL != 0 { right() } else { 0 }
    }
    fn visit(expr: &Expr, local_loop: bool, depth: usize) -> u8 {
        if depth > crate::cache_codec::MAX_DEPTH {
            return NORMAL | ESCAPE;
        }
        match &expr.kind {
            K::LexicalExit {
                kind: LexicalExitKind::Return,
                ..
            } => RETURN,
            K::LexicalExit { .. } => {
                if local_loop {
                    LOCAL
                } else {
                    ESCAPE
                }
            }
            K::Block { statements, tail } => {
                let mut paths = NORMAL;
                for statement in statements {
                    paths = sequence(paths, || match statement {
                        S::While {
                            condition, body, ..
                        } => sequence(visit(condition, local_loop, depth + 1), || {
                            NORMAL | (visit(body, true, depth + 1) & (RETURN | ESCAPE))
                        }),
                        S::For { values, body, .. }
                        | S::ForBorrow { values, body, .. }
                        | S::ForOwn { values, body, .. } => {
                            sequence(visit(values, local_loop, depth + 1), || {
                                NORMAL | (visit(body, true, depth + 1) & (RETURN | ESCAPE))
                            })
                        }
                        S::Let { value, .. } | S::Assign { value, .. } => {
                            visit(value, local_loop, depth + 1)
                        }
                        S::Unsafe { body, .. } => visit(body, local_loop, depth + 1),
                    });
                }
                sequence(paths, || visit(tail, local_loop, depth + 1))
            }
            K::If {
                condition,
                then_branch,
                else_branch,
            } => sequence(visit(condition, local_loop, depth + 1), || {
                visit(then_branch, local_loop, depth + 1)
                    | visit(else_branch, local_loop, depth + 1)
            }),
            K::Match {
                scrutinee, arms, ..
            } => sequence(visit(scrutinee, local_loop, depth + 1), || {
                if arms.is_empty() {
                    NORMAL
                } else {
                    arms.iter().fold(0, |paths, arm| {
                        paths
                            | sequence(
                                arm.guard
                                    .as_ref()
                                    .map_or(NORMAL, |guard| visit(guard, local_loop, depth + 1)),
                                || visit(&arm.value, local_loop, depth + 1),
                            )
                    })
                }
            }),
            // The independent contour check refuses control crossing values or
            // callable boundaries; those expressions have ordinary completion.
            _ => NORMAL,
        }
    }
    visit(expr, false, 0) == RETURN
}
