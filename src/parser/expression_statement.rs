//! Intermediate scalar expression statements use the ordinary immutable Let.
use super::{Parser, for_loop, statement_if, statement_if::IfItem};
use crate::ast::{Expr, LetSyntax, Span, Statement};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;
impl Parser {
    pub(super) fn scalar_statement(&mut self, value: Expr) -> Result<IfItem, Diagnostic> {
        if !self.at(&TokenKind::Semicolon) {
            return Ok(IfItem::Value(value));
        }
        let end = self.bump().span;
        if self.at(&TokenKind::RBrace) || self.at(&TokenKind::Eof) {
            return Err(self.expression_statement(&value));
        }
        Ok(IfItem::Statement(Statement::Let {
            syntax: LetSyntax::ExpressionStatement,
            name: self.discard_name(),
            name_span: value.span,
            mutable: false,
            declared: None,
            span: value.span.merge(end),
            value,
        }))
    }
}

impl Parser {
    /// The statements and final expression of a block whose `{` was consumed,
    /// through its `}`. Only a statement `if` branch passes `optional_tail`.
    pub(super) fn block_contents(
        &mut self,
        optional_tail: bool,
    ) -> Result<(Vec<Statement>, Option<Expr>, Span), Diagnostic> {
        let mut statements = Vec::new();
        let mut trailing_if = None;
        loop {
            if self.explicit_lexical_exits && self.at_lexical_exit() {
                let tail = self.lexical_exit_tail()?;
                let end = self.return_end()?;
                return Ok((statements, Some(tail), end));
            }
            if self.at_keyword("if") {
                let checkpoint = self.cursor;
                match self.statement_if()? {
                    statement_if::IfItem::Statement(statement) => {
                        statements.push(statement);
                        trailing_if = Some(checkpoint);
                    }
                    statement_if::IfItem::Value(tail) => match self.scalar_statement(tail)? {
                        statement_if::IfItem::Statement(statement) => {
                            statements.push(statement);
                            trailing_if = None;
                            continue;
                        }
                        statement_if::IfItem::Value(tail) => {
                            return self.block_tail_end(statements, tail);
                        }
                    },
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
                if self.at(&TokenKind::RBrace) {
                    break;
                }
                if let Some(diagnostic) = self
                    .foreign_statement()
                    .or_else(|| self.compound_assignment())
                {
                    return Err(diagnostic);
                }
                let value = self.expression(0)?;
                match self.scalar_statement(value)? {
                    statement_if::IfItem::Statement(statement) => statement,
                    statement_if::IfItem::Value(value) => {
                        return self.block_tail_end(statements, value);
                    }
                }
            };
            statements.push(statement);
            trailing_if = None;
        }
        if self.at(&TokenKind::RBrace) {
            if optional_tail {
                let end = self.bump().span;
                return Ok((statements, None, end));
            }
            if let Some(checkpoint) = trailing_if {
                return Err(self.valueless_tail_if(checkpoint));
            }
            return Err(self.error_here("SPX-P203", "block requires a final value expression"));
        }
        if let Some(diagnostic) = self
            .foreign_statement()
            .or_else(|| self.compound_assignment())
        {
            return Err(diagnostic);
        }
        let tail = self.expression(0)?;
        self.block_tail_end(statements, tail)
    }

    pub(super) fn block_tail_end(
        &mut self,
        statements: Vec<Statement>,
        tail: Expr,
    ) -> Result<(Vec<Statement>, Option<Expr>, Span), Diagnostic> {
        if self.take(&TokenKind::Semicolon) {
            return Err(self.expression_statement(&tail));
        }
        let end = self.expect(&TokenKind::RBrace, "`}` after block")?.span;
        Ok((statements, Some(tail), end))
    }
}
