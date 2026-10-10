//! Explicit source tails only when the acyclic compatibility route cannot apply.
use super::*;
use crate::ast::LexicalExitKind;
impl Parser {
    pub(super) fn needs_explicit_lexical_exits(&self, start: usize) -> bool {
        if self
            .tokens
            .get(start)
            .is_none_or(|token| token.kind != TokenKind::LBrace)
        {
            return false;
        }
        let mut depth = 0usize;
        let mut loop_seen = false;
        let mut returning = false;
        for index in start..self.tokens.len() {
            match &self.tokens[index].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::Ident(name)
                    if matches!(name.as_str(), "break" | "continue")
                        && self
                            .tokens
                            .get(index + 1)
                            .is_some_and(|token| token.kind == TokenKind::Semicolon) =>
                {
                    return true
                }
                TokenKind::Ident(name) if matches!(name.as_str(), "while" | "for") => {
                    loop_seen = true
                }
                _ if self.starts_return(index) => returning = true,
                TokenKind::Eof => break,
                _ => {}
            }
        }
        loop_seen && returning
    }
    pub(super) fn at_lexical_exit(&self) -> bool {
        self.starts_return(self.cursor)
            || (matches!(&self.current().kind, TokenKind::Ident(name) if matches!(name.as_str(), "break" | "continue"))
                && self
                    .tokens
                    .get(self.cursor + 1)
                    .is_some_and(|token| token.kind == TokenKind::Semicolon))
    }
    pub(super) fn lexical_exit_tail(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump();
        let TokenKind::Ident(name) = token.kind else {
            unreachable!("exit lookahead");
        };
        let kind = match name.as_str() {
            "break" => LexicalExitKind::Break,
            "continue" => LexicalExitKind::Continue,
            "return" => LexicalExitKind::Return,
            _ => unreachable!("exit lookahead"),
        };
        let value = if kind == LexicalExitKind::Return {
            Some(Box::new(self.expression(0)?))
        } else {
            None
        };
        let end = self
            .expect(&TokenKind::Semicolon, "`;` after lexical exit")?
            .span;
        Ok(Expr {
            kind: ExprKind::LexicalExit { kind, value },
            span: token.span.merge(end),
        })
    }
}
