//! Retained path prefixes after the exact Enter header has succeeded.
//! This is a lifetime/storage helper, not an identity or membership authority.
use super::*;
#[cfg(test)]
mod tests;

/// The sole production caller has already rejected NUL, required the complete
/// `ExpressionId::matches(function, expected_path)` encoding, inserted the
/// unique expression ID, and validated its type. That exact comparison proves
/// the final expected_len bytes are the expected UTF-8 path. Borrow only from
/// the current immutable expression ID, never from the temporary expected text.
pub(super) fn after_header(expression: &ResolvedExpr, expected_len: usize) -> &str {
    let identity = expression.id.as_str();
    let start = identity
        .len()
        .checked_sub(expected_len)
        .expect("authenticated identity includes its expected path bytes");
    identity
        .get(start..)
        .expect("authenticated path starts at a UTF-8 boundary")
}
