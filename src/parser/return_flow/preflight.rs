//! Allocation-free lower-bound depth census before recursive guard normalization.
use super::*;
impl Parser {
    pub(super) fn guard_return_preflight(&self) -> Result<(), Diagnostic> {
        let mut depth = 0usize;
        let mut guards = 0usize;
        let mut index = self.cursor;
        while let Some(token) = self.tokens.get(index) {
            match token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::Eof => break,
                _ => {}
            }
            if depth == 1 && matches!(&token.kind, TokenKind::Ident(name) if name == "if") {
                let mut open = index + 1;
                while self.tokens.get(open).is_some_and(|token| {
                    !matches!(
                        token.kind,
                        TokenKind::LBrace | TokenKind::RBrace | TokenKind::Eof
                    )
                }) {
                    open += 1;
                }
                if self
                    .tokens
                    .get(open)
                    .is_some_and(|token| token.kind == TokenKind::LBrace)
                {
                    let mut nested = 1usize;
                    let mut close = open + 1;
                    let mut terminal = false;
                    while let Some(token) = self.tokens.get(close) {
                        match token.kind {
                            TokenKind::LBrace => nested += 1,
                            TokenKind::RBrace => {
                                nested -= 1;
                                if nested == 0 {
                                    break;
                                }
                            }
                            TokenKind::Eof => break,
                            _ if nested == 1 && self.starts_return(close) => terminal = true,
                            _ => {}
                        }
                        close += 1;
                    }
                    let has_else = matches!(self.tokens.get(close + 1).map(|token| &token.kind), Some(TokenKind::Ident(name)) if name == "else");
                    if nested == 0 && terminal && !has_else {
                        guards += 1;
                        // Every suffix guard necessarily adds an If and its
                        // enclosing continuation Block to the canonical AST.
                        if 1 + 2 * guards > super::super::depth::MAX_SOURCE_NESTING {
                            return Err(Diagnostic::error(
                                "SPX-P207",
                                "guard-return normalization exceeds the source nesting bound",
                                token.span,
                            )
                            .at_path(&self.path)
                            .with_help("split the guard chain into named helper functions"));
                        }
                    }
                    if nested == 0 {
                        index = close;
                    }
                }
            }
            index += 1;
        }
        Ok(())
    }

    pub(super) fn unary_return_statement(&self, index: usize) -> bool {
        // An existing variable named `return` remains an ordinary expression
        // in `return - value` tails. The explicit form requires its semicolon.
        let mut nesting = 0usize;
        for token in self.tokens.iter().skip(index + 1) {
            match token.kind {
                TokenKind::LParen | TokenKind::LBracket => nesting += 1,
                TokenKind::RParen | TokenKind::RBracket => nesting = nesting.saturating_sub(1),
                TokenKind::Semicolon if nesting == 0 => return true,
                TokenKind::RBrace | TokenKind::Eof if nesting == 0 => return false,
                _ => {}
            }
        }
        false
    }
}
