//! Text-ordered scratch keys borrow the same immutable current HIR declarations.
//! No lookup, duplicate priority, authority or debit policy changes here.
use super::*;
pub(super) type Ids<'a> = BTreeSet<&'a DeclarationId>;
pub(super) type Imports<'a> = BTreeMap<&'a DeclarationId, &'a ResolvedImport>;
#[cfg(test)]
mod tests;
