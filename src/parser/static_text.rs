//! Explicit adjacent borrowed literal spelling; parsing supplies no authority.
use super::*;
impl Parser {
    pub(super) fn adjacent_text_literal(&self, prefix: Span) -> bool {
        self.current().span.start == prefix.end
            && matches!(self.current().kind, TokenKind::String(_))
    }
    pub(super) fn static_text_literal(&mut self, prefix: Span) -> Expr {
        let token = self.bump().clone();
        let TokenKind::String(value) = token.kind else {
            unreachable!("checked adjacent String token")
        };
        Expr {
            kind: ExprKind::BorrowedUtf8Literal(value),
            span: prefix.merge(token.span),
        }
    }
}
