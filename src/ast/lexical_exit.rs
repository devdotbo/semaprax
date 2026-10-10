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
