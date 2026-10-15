//! Parsing the two source-level bounded traversal spellings.

use crate::ast::{Statement, TraversalIndex};
use crate::diagnostic::Diagnostic;

use super::Parser;

pub(super) fn parse(parser: &mut Parser) -> Result<Statement, Diagnostic> {
    let start = parser.keyword("for")?.span;
    let consuming = parser.at_keyword("own");
    let borrowing = !consuming && parser.at_keyword("borrow");
    if consuming || borrowing {
        parser.bump();
    }
    let (item, item_span) = parser.ident("loop item binding")?;
    let index = if borrowing && parser.at_keyword("at") {
        parser.bump();
        let (name, span) = parser.ident("traversal index binding")?;
        Some(TraversalIndex { name, span })
    } else {
        None
    };
    parser.keyword("in")?;
    let values = parser
        .expression_with_record_literals(0, false)
        .map_err(|diagnostic| parser.range_for_hint(diagnostic))?;
    let body = parser.block("`for` body")?;
    let span = start.merge(body.span);
    Ok(if consuming {
        Statement::ForOwn {
            item,
            item_span,
            values: Box::new(values),
            body: Box::new(body),
            span,
        }
    } else if borrowing {
        Statement::ForBorrow {
            item,
            item_span,
            index,
            values: Box::new(values),
            body: Box::new(body),
            span,
        }
    } else {
        Statement::For {
            item,
            item_span,
            values: Box::new(values),
            body: Box::new(body),
            span,
        }
    })
}
