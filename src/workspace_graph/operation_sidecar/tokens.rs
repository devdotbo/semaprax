//! Exact authored token identity helpers, without semantic authority.
use super::*;
pub(super) fn find_identifier_token(
    tokens: &[crate::lexer::Token],
    name: &str,
    start: usize,
    end: usize,
) -> Result<Span, Vec<Diagnostic>> {
    let first = tokens.partition_point(|token| token.span.end <= start);
    tokens[first..]
        .iter()
        .take_while(|token| token.span.start < end)
        .find(|token| {
            token.span.start >= start
                && token.span.end <= end
                && matches!(&token.kind, crate::lexer::TokenKind::Ident(value) if value == name)
        })
        .map(|token| token.span)
        .ok_or_else(operation_sidecar_disagreement)
}

pub(super) fn source_text_token(
    tokens: &[crate::lexer::Token],
    span: Span,
) -> Result<&str, Vec<Diagnostic>> {
    tokens
        .binary_search_by_key(&span.start, |token| token.span.start)
        .ok()
        .and_then(|index| tokens.get(index))
        .filter(|token| token.span == span)
        .and_then(|token| match &token.kind {
            crate::lexer::TokenKind::Ident(value) => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(operation_sidecar_disagreement)
}

pub(super) fn module_use_alias_span(
    tokens: &[crate::lexer::Token],
    module_use: &crate::ast::ModuleUse,
) -> Result<Span, Vec<Diagnostic>> {
    let first = tokens.partition_point(|token| token.span.end <= module_use.span.start);
    let scoped =
        &tokens[first..tokens.partition_point(|token| token.span.start < module_use.span.end)];
    let mut meaningful = scoped
        .iter()
        .filter(|token| !matches!(token.kind, crate::lexer::TokenKind::Eof));
    let semicolon = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    let alias = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    let keyword = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    match (&keyword.kind, &alias.kind, &semicolon.kind) {
        (
            crate::lexer::TokenKind::Ident(as_keyword),
            crate::lexer::TokenKind::Ident(alias_name),
            crate::lexer::TokenKind::Semicolon,
        ) if as_keyword == "as" && alias_name == &module_use.alias => Ok(alias.span),
        _ => Err(operation_sidecar_disagreement()),
    }
}
