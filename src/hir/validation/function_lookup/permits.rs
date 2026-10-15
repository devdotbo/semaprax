//! Exact membership only; permits never suppress current effect/body checks.
#[cfg(test)]
use super::*;
pub(in crate::hir::validation) enum Permits<'a> {
    Borrowed(&'a [&'a str]),
    Source(&'a [String]),
    #[cfg(test)]
    Legacy(BTreeSet<&'a str>),
}
impl Permits<'_> {
    pub(in crate::hir::validation) fn contains(&self, effect: &str) -> bool {
        match self {
            Self::Borrowed(permits) => permits.binary_search(&effect).is_ok(),
            Self::Source(permits) => permits.iter().any(|permit| permit == effect),
            #[cfg(test)]
            Self::Legacy(permits) => permits.contains(effect),
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod source_tests;
