//! Intermediate scalar expression statements use the ordinary immutable Let.
use super::statement_if::{IfItem, OpenChain};
use super::{for_loop, statement_if, Parser};
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

/// The statements, optional final expression and closing `}` span of a block.
pub(super) type BlockContents = (Vec<Statement>, Option<Expr>, Span);

/// A block whose `{` was consumed and whose items are still being parsed.
struct OpenBlock {
    statements: Vec<Statement>,
    trailing_if: Option<usize>,
    optional_tail: bool,
}

impl OpenBlock {
    fn new(optional_tail: bool) -> Self {
        Self {
            statements: Vec::new(),
            trailing_if: None,
            optional_tail,
        }
    }
}

/// A statement `if` chain waiting on its open branch block.
struct Pending {
    /// The block the `if` item belongs to and the cursor at its `if`; `None`
    /// only for a chain whose caller parsed the condition itself.
    parent: Option<(OpenBlock, usize)>,
    chain: OpenChain,
}

/// How the bottom frame of `Parser::settle_frames` completed.
enum Settled {
    Block(Result<BlockContents, Diagnostic>),
    If(Result<IfItem, Diagnostic>),
}

impl Parser {
    /// The statements and final expression of a block whose `{` was consumed,
    /// through its `}`. Only a statement `if` branch passes `optional_tail`.
    pub(super) fn block_contents(
        &mut self,
        optional_tail: bool,
    ) -> Result<BlockContents, Diagnostic> {
        match self.settle_frames(Vec::new(), OpenBlock::new(optional_tail)) {
            Settled::Block(contents) => contents,
            Settled::If(_) => unreachable!("every chain under a block root has a parent block"),
        }
    }

    /// Finish a statement `if` chain whose first branch `{` was consumed.
    pub(super) fn settle_statement_if(&mut self, chain: OpenChain) -> Result<IfItem, Diagnostic> {
        let root = vec![Pending {
            parent: None,
            chain,
        }];
        match self.settle_frames(root, OpenBlock::new(true)) {
            Settled::If(item) => item,
            Settled::Block(_) => unreachable!("the root chain settles before its branch blocks"),
        }
    }

    /// Statement `if` branches nest blocks through explicit `Pending` frames
    /// rather than native recursion: each frame is the exact suspended state of
    /// the former `block_contents` -> `statement_if` -> branch -> `block_contents`
    /// cycle, resumed in the same order with the same results and diagnostics.
    /// Only the statement `if` cycle moves to the heap; every other construct
    /// parses as before.
    fn settle_frames(&mut self, mut pending: Vec<Pending>, mut block: OpenBlock) -> Settled {
        loop {
            let mut closed = match self.block_items(&mut block) {
                Ok(Some(contents)) => Ok(contents),
                Ok(None) => {
                    let checkpoint = self.cursor;
                    match self.statement_if_open() {
                        Ok(chain) => {
                            let parent = std::mem::replace(&mut block, OpenBlock::new(true));
                            pending.push(Pending {
                                parent: Some((parent, checkpoint)),
                                chain,
                            });
                            continue;
                        }
                        Err(diagnostic) => Err(diagnostic),
                    }
                }
                Err(diagnostic) => Err(diagnostic),
            };
            // Deliver the closed block to the chains waiting on it, until a
            // chain opens another branch or a parent block resumes its items.
            loop {
                let Some(top) = pending.last_mut() else {
                    return Settled::Block(closed);
                };
                let item = match self.statement_if_resume(&mut top.chain, closed) {
                    Ok(None) => {
                        block = OpenBlock::new(true);
                        break;
                    }
                    Ok(Some(item)) => Ok(item),
                    Err(diagnostic) => Err(diagnostic),
                };
                let parent = pending.pop().and_then(|frame| frame.parent);
                let Some((mut parent, checkpoint)) = parent else {
                    return Settled::If(item);
                };
                closed = match self.block_after_if(&mut parent, checkpoint, item) {
                    Ok(None) => {
                        block = parent;
                        break;
                    }
                    Ok(Some(contents)) => Ok(contents),
                    Err(diagnostic) => Err(diagnostic),
                };
            }
        }
    }

    /// Take a finished `if` item into its block. `Some` is the block's
    /// contents when the item was its final value.
    fn block_after_if(
        &mut self,
        block: &mut OpenBlock,
        checkpoint: usize,
        item: Result<IfItem, Diagnostic>,
    ) -> Result<Option<BlockContents>, Diagnostic> {
        match item? {
            statement_if::IfItem::Statement(statement) => {
                block.statements.push(statement);
                block.trailing_if = Some(checkpoint);
            }
            statement_if::IfItem::Value(tail) => match self.scalar_statement(tail)? {
                statement_if::IfItem::Statement(statement) => {
                    block.statements.push(statement);
                    block.trailing_if = None;
                }
                statement_if::IfItem::Value(tail) => {
                    let statements = std::mem::take(&mut block.statements);
                    return self.block_tail_end(statements, tail).map(Some);
                }
            },
        }
        Ok(None)
    }

    /// Parse block items until the block closes (`Some`) or an `if` item
    /// starts at the cursor (`None`).
    fn block_items(&mut self, block: &mut OpenBlock) -> Result<Option<BlockContents>, Diagnostic> {
        loop {
            if self.explicit_lexical_exits && self.at_lexical_exit() {
                let tail = self.lexical_exit_tail()?;
                let end = self.return_end()?;
                let statements = std::mem::take(&mut block.statements);
                return Ok(Some((statements, Some(tail), end)));
            }
            if self.at_keyword("if") {
                return Ok(None);
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
                        let statements = std::mem::take(&mut block.statements);
                        return self.block_tail_end(statements, value).map(Some);
                    }
                }
            };
            block.statements.push(statement);
            block.trailing_if = None;
        }
        let statements = std::mem::take(&mut block.statements);
        if self.at(&TokenKind::RBrace) {
            if block.optional_tail {
                let end = self.bump().span;
                return Ok(Some((statements, None, end)));
            }
            if let Some(checkpoint) = block.trailing_if {
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
        self.block_tail_end(statements, tail).map(Some)
    }

    pub(super) fn block_tail_end(
        &mut self,
        statements: Vec<Statement>,
        tail: Expr,
    ) -> Result<BlockContents, Diagnostic> {
        if self.take(&TokenKind::Semicolon) {
            return Err(self.expression_statement(&tail));
        }
        let end = self.expect(&TokenKind::RBrace, "`}` after block")?.span;
        Ok((statements, Some(tail), end))
    }
}
