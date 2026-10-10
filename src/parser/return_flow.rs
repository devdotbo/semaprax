//! Acyclic function guard returns, normalized without a new runtime exit.
//!
//! Each missing-else continuation is moved exactly once, never cloned. A
//! returning arm becomes an ordinary value arm, so postconditions, result
//! staging, ownership and cleanup stay under the existing independent gates.
//! Loops, closures and returns embedded in value expressions remain closed.

use crate::ast::{Expr, ExprKind, Span, Statement};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::{Parser, for_loop, statement_if};

impl Parser {
    pub(super) fn function_return_body(&mut self) -> Result<Expr, Diagnostic> {
        if !self.return_in_block(self.cursor) {
            return self.block("function body");
        }
        let start = self
            .expect(&TokenKind::LBrace, "`{` before function body")?
            .span;
        self.return_body_after_open(start, false, 0)
    }

    fn return_body_after_open(
        &mut self,
        start: Span,
        require_return: bool,
        depth: usize,
    ) -> Result<Expr, Diagnostic> {
        if depth >= super::depth::MAX_SOURCE_NESTING {
            return Err(self
                .error_here(
                    "SPX-P207",
                    "guard-return normalization exceeds the source nesting bound",
                )
                .with_help("split the guard chain into named helper functions"));
        }
        let mut statements = Vec::new();
        loop {
            if self.starts_return(self.cursor) {
                self.bump();
                let value = self.expression(0)?;
                self.expect(&TokenKind::Semicolon, "`;` after return value")?;
                let end = self.return_end()?;
                return Ok(block(start.merge(end), statements, value));
            }
            if self.at_keyword("if") {
                let guard_start = self.keyword("if")?.span;
                let condition = self.expression_with_record_literals(0, false)?;
                if self.return_in_block(self.cursor) {
                    let (value, consumed_end) = self.return_guard_after_condition(
                        guard_start,
                        condition,
                        require_return,
                        depth + 1,
                    )?;
                    let end = match consumed_end {
                        Some(end) => end,
                        None => self.return_end()?,
                    };
                    return Ok(block(start.merge(end), statements, value));
                }
                match self.statement_if_after_condition(guard_start, condition)? {
                    statement_if::IfItem::Statement(statement) => statements.push(statement),
                    statement_if::IfItem::Value(value) => {
                        if require_return {
                            return Err(self.required_guard_return());
                        }
                        let (_, value, end) = self.block_tail_end(Vec::new(), value)?;
                        return Ok(block(
                            start.merge(end),
                            statements,
                            value.expect("value tail"),
                        ));
                    }
                }
                continue;
            }
            let statement = if self.at_keyword("let") {
                self.let_statement()?
            } else if self.at_assign_statement() {
                self.assign_statement()?
            } else if self.at_unsafe_statement() {
                self.unsafe_statement()?
            } else if self.at_keyword("while") {
                self.while_statement()?
            } else if self.at_keyword("for") {
                for_loop::parse(self)?
            } else {
                break;
            };
            statements.push(statement);
        }
        if require_return {
            return Err(self.required_guard_return());
        }
        if self.at(&TokenKind::RBrace) {
            return Err(self.error_here("SPX-P203", "block requires a final value expression"));
        }
        if let Some(diagnostic) = self
            .foreign_statement()
            .or_else(|| self.compound_assignment())
        {
            return Err(diagnostic);
        }
        let value = self.expression(0)?;
        let (_, value, end) = self.block_tail_end(Vec::new(), value)?;
        Ok(block(
            start.merge(end),
            statements,
            value.expect("value tail"),
        ))
    }

    /// Return the guard value and whether its missing-else continuation has
    /// already consumed the enclosing block's closing brace.
    fn return_guard(
        &mut self,
        require_return: bool,
        depth: usize,
    ) -> Result<(Expr, Option<Span>), Diagnostic> {
        let start = self.keyword("if")?.span;
        let condition = self.expression_with_record_literals(0, false)?;
        self.return_guard_after_condition(start, condition, require_return, depth)
    }

    fn return_guard_after_condition(
        &mut self,
        start: Span,
        condition: Expr,
        require_return: bool,
        depth: usize,
    ) -> Result<(Expr, Option<Span>), Diagnostic> {
        let branch_start = self
            .expect(&TokenKind::LBrace, "`{` before returning guard")?
            .span;
        let then_branch = self.return_body_after_open(branch_start, true, depth + 1)?;
        let (else_branch, consumed_end) = if self.at_keyword("else") {
            self.bump();
            if self.at_keyword("if") {
                {
                    let (nested, consumed_end) = self.return_guard(require_return, depth + 1)?;
                    (block(nested.span, Vec::new(), nested), consumed_end)
                }
            } else {
                let branch_start = self
                    .expect(&TokenKind::LBrace, "`{` before returning alternative")?
                    .span;
                (
                    self.return_body_after_open(branch_start, true, depth + 1)?,
                    None,
                )
            }
        } else {
            // The suffix belongs exclusively to the non-returning path. Its
            // statements keep their order and lexical nesting; no AST copies.
            let suffix_start = self.current().span;
            let continuation =
                self.return_body_after_open(suffix_start, require_return, depth + 1)?;
            let end = continuation.span;
            (continuation, Some(end))
        };
        let span = start.merge(else_branch.span);
        Ok((
            Expr {
                kind: ExprKind::If {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                },
                span,
            },
            consumed_end,
        ))
    }

    fn return_end(&mut self) -> Result<Span, Diagnostic> {
        if !self.at(&TokenKind::RBrace) {
            return Err(self
                .error_here(
                    "SPX-P209",
                    "code after an unconditional return is unreachable",
                )
                .with_help("remove the unreachable code or put it on a non-returning guard path"));
        }
        Ok(self.bump().span)
    }

    fn required_guard_return(&self) -> Diagnostic {
        self.error_here(
            "SPX-P210",
            "a returning guard arm must end in an explicit return on every path",
        ).with_help("end this arm with `return value;`; mixed fallthrough arms and loop exits are outside acyclic guard returns v1")
    }

    fn return_in_block(&self, start: usize) -> bool {
        if self
            .tokens
            .get(start)
            .is_none_or(|token| token.kind != TokenKind::LBrace)
        {
            return false;
        }
        let mut depth = 0usize;
        for index in start..self.tokens.len() {
            match self.tokens[index].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                TokenKind::Eof => return false,
                _ if self.starts_return(index) => return true,
                _ => {}
            }
        }
        false
    }

    /// Preserve existing uses of the ordinary identifier `return`, including
    /// calls and arithmetic. Only formerly refused adjacent operands opt in.
    fn starts_return(&self, index: usize) -> bool {
        matches!(self.tokens.get(index).map(|token| &token.kind), Some(TokenKind::Ident(name)) if name == "return")
            && matches!(
                self.tokens.get(index + 1).map(|token| &token.kind),
                Some(
                    TokenKind::Ident(_)
                        | TokenKind::Int(_)
                        | TokenKind::IntMinMagnitude
                        | TokenKind::Int32(_)
                        | TokenKind::Int32MinMagnitude
                        | TokenKind::Float(_)
                        | TokenKind::Char(_)
                        | TokenKind::Uint8(_)
                        | TokenKind::Usize(_)
                        | TokenKind::String(_)
                        | TokenKind::LBracket
                        | TokenKind::LBrace
                        | TokenKind::Bang
                )
            )
    }
}

fn block(span: Span, statements: Vec<Statement>, tail: Expr) -> Expr {
    Expr {
        kind: ExprKind::Block {
            statements,
            tail: Box::new(tail),
        },
        span,
    }
}
