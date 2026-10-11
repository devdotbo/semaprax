//! Statement `if`: `if <condition> { <statements> }`, optionally followed by
//! `else if` and `else` branches, in statement position.
//!
//! This is parse-level semantic sugar with parser-minted canonical syntax provenance.
//! A statement `if` lowers to the value discard agents had to spell by hand,
//! `let _if1 = if <condition> { <statements>; 0 } else { 0 };`. The binding
//! is an ordinary immutable `i64` local, because a block admits no shadowing:
//! each discard takes the next `_if<n>` name that no identifier in the file
//! already spells. Canonical text preserves statement spelling and parses back to
//! the same normalized program. A branch that
//! ends without a value receives the `i64` tail `0`, and a missing `else`
//! becomes `else { 0 }`, so resolution, both verifiers, cleanup plans, the
//! semantic graph, the interpreter, and native and Wasm code see exactly the
//! value `if` they already admit. A branch that does end with a value keeps an
//! integer literal tail as is and otherwise discards it with `let _if<n> = <value>;`
//! before the `0`, so branches never disagree on the discarded type.
//!
//! A chain in which every branch ends with a value and a final `else` exists
//! is an ordinary value `if`: it stays the block's final expression when `}`,
//! `;`, or an operator follows it, and is discarded as a statement only when
//! another statement follows. One exception: a chain containing a parsed
//! lexical exit, followed directly by `;` and another item, is that same
//! statement discard; canonical text drops the `;`. The `if` is parsed once;
//! no input that parsed before changes meaning.

use std::collections::BTreeSet;

use crate::ast::{BranchTail, Expr, ExprKind, LetSyntax, Span, Statement, StatementIfSyntax};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

/// The `_if<n>` discard names handed out while parsing one file.
#[derive(Default)]
pub(super) struct DiscardNames {
    next: usize,
    /// Every identifier the file spells, gathered on first use.
    taken: Option<BTreeSet<String>>,
    /// Only provisional names from a selective legacy-first parse attempt.
    provisional: Vec<usize>,
    recording: bool,
}

impl DiscardNames {
    pub(super) fn needs_inventory(&self) -> bool {
        self.taken.is_none()
    }
    pub(super) fn constructor_provisional_target(&self, additional: usize) -> Option<usize> {
        if !self.recording
            || self
                .provisional
                .capacity()
                .saturating_sub(self.provisional.len())
                >= additional
        {
            return Some(0);
        }
        self.provisional.len().checked_add(additional)
    }
    pub(super) fn prepare_constructor_provisional(&mut self, target: usize) -> bool {
        target == 0
            || (self
                .provisional
                .try_reserve_exact(target - self.provisional.len())
                .is_ok()
                && self.provisional.capacity() <= target)
    }
}

/// What an `if` at the start of a block item turned out to be.
pub(super) enum IfItem {
    /// A statement `if`, already lowered to `let _if<n> = if …;`.
    Statement(Statement),
    /// A value `if` that is (the start of) the block's final expression.
    Value(Expr),
}

/// A branch body before the chain decides whether it carries a value.
struct Branch {
    statements: Vec<Statement>,
    tail: Option<Expr>,
    span: Span,
}

struct Arm {
    start: Span,
    condition: Expr,
    body: Branch,
}

impl Parser {
    pub(super) fn statement_if(&mut self) -> Result<IfItem, Diagnostic> {
        let start = self.keyword("if")?.span;
        let condition = self.expression_with_record_literals(0, false)?;
        self.statement_if_after_condition(start, condition)
    }

    pub(super) fn statement_if_after_condition(
        &mut self,
        start: Span,
        condition: Expr,
    ) -> Result<IfItem, Diagnostic> {
        let mut next_condition = Some(condition);
        let mut arms = Vec::new();
        let mut otherwise = None;
        let mut arm_start = start;
        loop {
            let condition = match next_condition.take() {
                Some(condition) => condition,
                None => self.expression_with_record_literals(0, false)?,
            };
            let body = self.branch("`if` condition")?;
            arms.push(Arm {
                start: arm_start,
                condition,
                body,
            });
            if !self.at_keyword("else") {
                break;
            }
            self.bump();
            if self.at_keyword("if") {
                arm_start = self.bump().span;
                continue;
            }
            otherwise = Some(self.branch("`else`")?);
            break;
        }
        let valued = otherwise
            .as_ref()
            .is_some_and(|branch| branch.tail.is_some())
            && arms.iter().all(|arm| arm.body.tail.is_some());
        let tail_kind = |branch: &Branch| match &branch.tail {
            None => BranchTail::Absent,
            Some(value)
                if valued
                    || matches!(value.kind, ExprKind::Int(_))
                    || crate::ast::has_no_normal_completion(value) =>
            {
                BranchTail::Retained
            }
            Some(_) => BranchTail::Discarded,
        };
        let syntax = LetSyntax::StatementIf(StatementIfSyntax {
            branches: arms.iter().map(|arm| tail_kind(&arm.body)).collect(),
            alternative: otherwise.as_ref().map(tail_kind),
        });
        let expression = self.assemble(arms, otherwise, valued);
        if valued && crate::ast::has_no_normal_completion(&expression) {
            if !self.at(&TokenKind::RBrace) {
                return Err(self.error_here(
                    "SPX-P209",
                    "code after an unconditional lexical exit is unreachable",
                ));
            }
            return Ok(IfItem::Value(expression));
        }
        if valued {
            let before = self.cursor;
            let expression = self.postfix(expression, true)?;
            let expression = self.binary_continuation(expression, 0, true)?;
            if self.cursor == before && self.at_exit_guard_terminator(&expression) {
                let end = self.bump().span;
                return Ok(IfItem::Statement(
                    self.discard(start, expression, Some(end), syntax),
                ));
            }
            if self.cursor != before
                || self.at(&TokenKind::RBrace)
                || self.at(&TokenKind::Semicolon)
            {
                return Ok(IfItem::Value(expression));
            }
            return Ok(IfItem::Statement(
                self.discard(start, expression, None, syntax),
            ));
        }
        let end = self
            .take(&TokenKind::Semicolon)
            .then(|| self.previous_span());
        Ok(IfItem::Statement(
            self.discard(start, expression, end, syntax),
        ))
    }

    /// A bare valued exit guard may close with `;` when another block item
    /// follows. Only a parsed exit selects this; ordinary valued `if`s keep the
    /// scalar expression statement, and a final `;` keeps `SPX-P106`. Canonical
    /// text omits the `;`, so an item that could continue this value `if`
    /// (`-x;`, `(x);`) keeps the generic discard rather than change meaning.
    fn at_exit_guard_terminator(&mut self, expression: &Expr) -> bool {
        if !self.at(&TokenKind::Semicolon) {
            return false;
        }
        let terminator = self.cursor;
        self.cursor += 1;
        let separate = !matches!(
            self.current().kind,
            TokenKind::RBrace
                | TokenKind::Eof
                | TokenKind::LParen
                | TokenKind::Dot
                | TokenKind::Question
                | TokenKind::ColonColon
        ) && self.binary_op().is_none()
            && !self.at_keyword("with");
        self.cursor = terminator;
        separate && expression.contains_lexical_exit()
    }

    fn branch(&mut self, description: &str) -> Result<Branch, Diagnostic> {
        let start = self
            .expect(&TokenKind::LBrace, &format!("`{{` before {description}"))?
            .span;
        let (statements, tail, end) = self
            .block_contents(true)
            .map_err(|diagnostic| Self::attach_block_help(diagnostic, description))?;
        Ok(Branch {
            statements,
            tail,
            span: start.merge(end),
        })
    }

    /// A statement `if` is the last item of a block that needs a value. The
    /// value grammar reports what that `if` lacks (an `else`, or a branch
    /// value), so the diagnostic is the one this input always produced.
    pub(super) fn valueless_tail_if(&mut self, checkpoint: usize) -> Diagnostic {
        self.cursor = checkpoint;
        match self.expression(0) {
            Err(diagnostic) => diagnostic,
            Ok(_) => self.error_here("SPX-P203", "block requires a final value expression"),
        }
    }

    fn assemble(&mut self, arms: Vec<Arm>, otherwise: Option<Branch>, valued: bool) -> Expr {
        let mut alternative = match otherwise {
            Some(branch) => self.finish(branch, valued),
            None => {
                let last = arms.last().map_or_else(Span::default, |arm| arm.body.span);
                let end = point(last);
                Expr {
                    kind: ExprKind::Block {
                        statements: Vec::new(),
                        tail: Box::new(zero(end)),
                    },
                    span: end,
                }
            }
        };
        let count = arms.len();
        for (index, arm) in arms.into_iter().rev().enumerate() {
            let then_branch = self.finish(arm.body, valued);
            let span = arm.start.merge(alternative.span);
            let conditional = Expr {
                kind: ExprKind::If {
                    condition: Box::new(arm.condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(alternative),
                },
                span,
            };
            // `else if` is the nested `else { if … }` block, exactly as the value
            // grammar builds it.
            alternative = if index + 1 == count {
                conditional
            } else {
                Expr {
                    span: conditional.span,
                    kind: ExprKind::Block {
                        statements: Vec::new(),
                        tail: Box::new(conditional),
                    },
                }
            };
        }
        alternative
    }

    fn finish(&mut self, branch: Branch, valued: bool) -> Expr {
        let Branch {
            mut statements,
            tail,
            span,
        } = branch;
        let end = point(span);
        let tail = match tail {
            Some(tail)
                if valued
                    || matches!(tail.kind, ExprKind::Int(_))
                    || crate::ast::has_no_normal_completion(&tail) =>
            {
                tail
            }
            Some(value) => {
                statements.push(self.discard(value.span, value, None, LetSyntax::BranchTail));
                zero(end)
            }
            None => zero(end),
        };
        Expr {
            kind: ExprKind::Block {
                statements,
                tail: Box::new(tail),
            },
            span,
        }
    }

    fn discard(
        &mut self,
        start: Span,
        value: Expr,
        end: Option<Span>,
        syntax: LetSyntax,
    ) -> Statement {
        let span = start.merge(end.unwrap_or(value.span));
        Statement::Let {
            syntax,
            name: self.discard_name(),
            name_span: start,
            mutable: false,
            declared: None,
            value,
            span,
        }
    }

    pub(super) fn discard_name(&mut self) -> String {
        let tokens = &self.tokens;
        let taken = self.discards.taken.get_or_insert_with(|| {
            tokens
                .iter()
                .filter_map(|token| match &token.kind {
                    TokenKind::Ident(name) => Some(name.clone()),
                    _ => None,
                })
                .collect()
        });
        loop {
            self.discards.next += 1;
            let name = format!("_if{}", self.discards.next);
            if taken.insert(name.clone()) {
                if self.discards.recording {
                    self.discards.provisional.push(self.discards.next);
                }
                return name;
            }
        }
    }
}

/// The `0` a valueless branch yields, placed at the branch's closing brace.
fn zero(span: Span) -> Expr {
    Expr {
        kind: ExprKind::Int(0),
        span,
    }
}

/// The last byte of `span`, where a synthesised value is reported.
fn point(span: Span) -> Span {
    Span {
        start: span.end.saturating_sub(1).max(span.start),
        ..span
    }
}

/// An index-only undo log: authored names and successful prior names are never cloned.
pub(super) struct DiscardCheckpoint {
    next: usize,
    length: usize,
    recording: bool,
}
impl DiscardNames {
    pub(super) fn checkpoint(&mut self) -> DiscardCheckpoint {
        let checkpoint = DiscardCheckpoint {
            next: self.next,
            length: self.provisional.len(),
            recording: self.recording,
        };
        self.recording = true;
        checkpoint
    }
    pub(super) fn finish(&mut self, checkpoint: DiscardCheckpoint, rollback: bool) {
        if rollback {
            while self.provisional.len() > checkpoint.length {
                let index = self.provisional.pop().expect("provisional discard index");
                if let Some(taken) = &mut self.taken {
                    taken.remove(&format!("_if{index}"));
                }
            }
            self.next = checkpoint.next;
        } else if !checkpoint.recording {
            self.provisional.truncate(checkpoint.length);
        }
        self.recording = checkpoint.recording;
        if !self.recording {
            self.provisional = Vec::new();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{ExprKind, LetSyntax, Program, Statement};
    use crate::cache_codec::{decode, encode};

    fn source(terminator: &str) -> String {
        format!(
            "module exit.guard; @id(\"main\") fn main()->i64 {{ let mut i=0; while i<3 {{ i=i+1; if i==2 {{continue;}} else {{0}}{terminator} i=i+0; 0 }} i }}"
        )
    }

    fn guard(program: &Program) -> &Statement {
        let ExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
            panic!("function block")
        };
        let Statement::While { body, .. } = &statements[1] else {
            panic!("authored loop")
        };
        let ExprKind::Block { statements, .. } = &body.kind else {
            panic!("loop block")
        };
        &statements[1]
    }

    #[test]
    fn terminated_exit_guard_reuses_the_statement_if_and_roundtrips_the_cache() {
        let old = crate::parse(&source(""), "exit-guard.spx").unwrap();
        let new = crate::parse(&source(";"), "exit-guard.spx").unwrap();
        let (
            Statement::Let {
                syntax: old_syntax,
                name: old_name,
                value: old_value,
                span: old_span,
                ..
            },
            Statement::Let {
                syntax,
                name,
                mutable: false,
                declared: None,
                value,
                span,
                ..
            },
        ) = (guard(&old), guard(&new))
        else {
            panic!("normalized discards")
        };
        assert!(matches!(syntax, LetSyntax::StatementIf(_)));
        assert_eq!(syntax, old_syntax);
        assert_eq!(name, old_name);
        // The value tree, its spans and its hidden name are those of the old
        // spelling; only the statement span additionally covers the `;`.
        assert_eq!(value, old_value);
        assert_eq!((span.start, span.end), (old_span.start, old_span.end + 1));
        let canonical = crate::format::canonical(&old);
        assert_eq!(crate::format::canonical(&new), canonical);
        assert!(!canonical.contains("let _if"), "{canonical}");
        let bytes = encode(&new).unwrap();
        let restored: Program = decode(&bytes).unwrap();
        assert_eq!(restored.functions, new.functions);
        assert_eq!(encode(&restored).unwrap(), bytes);
        assert!(decode::<Program>(&bytes[..bytes.len() - 1]).is_err());
        assert_eq!(crate::format::canonical(&restored), canonical);
        let reparsed = crate::parse(&canonical, "exit-guard.spx").unwrap();
        let Statement::Let {
            syntax: reparsed_syntax,
            name: reparsed_name,
            ..
        } = guard(&reparsed)
        else {
            panic!("canonical discard")
        };
        assert_eq!((reparsed_syntax, reparsed_name), (old_syntax, old_name));
    }
}
