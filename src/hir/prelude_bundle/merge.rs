//! Logical exports for Index roots and codecs. These infallible borrowed
//! adapters create no allocation or work allowance. Both inputs must already
//! follow the same canonical key order; duplicate keys retain the base value.
use super::*;

pub(crate) fn merge_by_key<'a, K: Ord + ?Sized + 'a, V: ?Sized + 'a>(
    base: impl Iterator<Item = (&'a K, &'a V)> + 'a,
    local: impl Iterator<Item = (&'a K, &'a V)> + 'a,
) -> impl Iterator<Item = (&'a K, &'a V)> + 'a {
    let mut base = base.peekable();
    let mut local = local.peekable();
    std::iter::from_fn(move || match (base.peek(), local.peek()) {
        (Some((a, _)), Some((b, _))) => match a.cmp(b) {
            std::cmp::Ordering::Less => base.next(),
            std::cmp::Ordering::Greater => local.next(),
            std::cmp::Ordering::Equal => {
                local.next();
                base.next()
            }
        },
        (Some(_), None) => base.next(),
        (None, Some(_)) => local.next(),
        (None, None) => None,
    })
}

struct Exact<I> {
    inner: I,
    remaining: usize,
}
impl<I: Iterator> Iterator for Exact<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.inner.next();
        if value.is_some() {
            self.remaining -= 1;
        }
        value
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<I: Iterator> ExactSizeIterator for Exact<I> {}

impl BasePreludeBundle {
    pub(crate) fn declaration_iteration<'a>(
        &'a self,
        local: &'a crate::hir::DeclarationIndex,
    ) -> impl ExactSizeIterator<Item = &'a Declaration> + 'a {
        let remaining = local
            .declarations
            .len()
            .checked_add(
                self.declarations_slice()
                    .iter()
                    .filter(|entry| !local.declarations.contains_key(&entry.id))
                    .count(),
            )
            .expect("held declaration inventories have addressable length");
        Exact {
            remaining,
            inner: merge_by_key(
                self.declarations_slice()
                    .iter()
                    .map(|entry| (&entry.id, entry)),
                local.declarations.iter(),
            )
            .map(|(_, value)| value),
        }
    }
}
