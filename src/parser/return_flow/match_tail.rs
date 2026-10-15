//! Terminal all-return match arms reuse the ordinary match grammar and joins.
use super::*;

impl Parser {
    pub(super) fn return_match_tail(
        &mut self,
        start: Span,
        depth: usize,
    ) -> Result<(Expr, bool), Diagnostic> {
        let mut returning = 0usize;
        let mut ordinary = 0usize;
        let value = self.match_expression_with_arm(start, |parser| {
            if parser.return_in_block(parser.cursor) {
                let start = parser
                    .expect(&TokenKind::LBrace, "`{` before returning match arm")?
                    .span;
                let value = parser.return_body_after_open(start, true, depth + 1)?;
                returning += 1;
                Ok(value)
            } else {
                ordinary += 1;
                parser.expression(0)
            }
        })?;
        if returning != 0 && ordinary != 0 {
            return Err(self.required_guard_return());
        }
        // Exhaustiveness, guards, existing owning carrier joins and borrowed
        // publication are independently checked by the ordinary source/HIR
        // gates. No lexical target or runtime exit is minted by this parser.
        Ok((value, returning != 0))
    }
}
