//! Complete original contiguous receiver, independently compiled.
/// Two contiguous declaration collections; no flattened reference directory.
/// Construction checks their combined length before an iterator is lent.
#[derive(Clone, Copy)]
pub(crate) struct DeclSlices<'a, T> {
    authored: &'a [T],
    generated: &'a [T],
    length: usize,
}
impl<'a, T> DeclSlices<'a, T> {
    pub(crate) fn one(authored: &'a [T]) -> Self {
        Self {
            authored,
            generated: &[],
            length: authored.len(),
        }
    }
    fn append(authored: &'a [T], generated: &'a [T]) -> Option<Self> {
        Some(Self {
            authored,
            generated,
            length: authored.len().checked_add(generated.len())?,
        })
    }
    pub(crate) fn len(&self) -> usize {
        self.length
    }
    pub(crate) fn first(&self) -> Option<&'a T> {
        self.get(0)
    }
    pub(crate) fn get(&self, index: usize) -> Option<&'a T> {
        if index < self.authored.len() {
            self.authored.get(index)
        } else {
            self.generated.get(index.checked_sub(self.authored.len())?)
        }
    }
    pub(crate) fn suffix(&self, start: usize) -> Option<Self> {
        if start > self.length {
            return None;
        }
        if start < self.authored.len() {
            Self::append(&self.authored[start..], self.generated)
        } else {
            Some(Self::one(&self.generated[start - self.authored.len()..]))
        }
    }
    pub(crate) fn iter(&self) -> DeclIter<'a, T> {
        DeclIter {
            authored: self.authored.iter(),
            generated: self.generated.iter(),
            remaining: self.length,
        }
    }
}
impl<T> std::ops::Index<usize> for DeclSlices<'_, T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("declaration index in bounds")
    }
}
pub(crate) struct DeclIter<'a, T> {
    authored: std::slice::Iter<'a, T>,
    generated: std::slice::Iter<'a, T>,
    remaining: usize,
}
impl<'a, T> Iterator for DeclIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.authored.next().or_else(|| self.generated.next());
        if next.is_some() {
            self.remaining -= 1;
        }
        next
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<T> DoubleEndedIterator for DeclIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let next = self
            .generated
            .next_back()
            .or_else(|| self.authored.next_back());
        if next.is_some() {
            self.remaining -= 1;
        }
        next
    }
}
impl<T> ExactSizeIterator for DeclIter<'_, T> {}
impl<T> std::iter::FusedIterator for DeclIter<'_, T> {}
impl<'a, T> IntoIterator for DeclSlices<'a, T> {
    type Item = &'a T;
    type IntoIter = DeclIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::DeclSlices;
    #[test]
    fn complete_contiguous_receiver_keeps_original_forward_reverse_suffix_and_index_order() {
        let authored = ["prefix", "é", "漢字"];
        let generated = ["suffix", "é"];
        let original = DeclSlices::append(&authored, &generated).unwrap();
        let current = super::super::decl_slices::DeclSlices::append(&authored, &generated).unwrap();
        assert_eq!(original.len(), current.len());
        assert_eq!(original.first(), current.first());
        assert!(original.iter().eq(current.iter()));
        assert!(original.iter().rev().eq(current.iter().rev()));
        for index in 0..=original.len() {
            assert_eq!(original.get(index), current.get(index));
            if index < original.len() {
                assert_eq!(original[index], current[index]);
            }
            assert!(original
                .suffix(index)
                .unwrap()
                .iter()
                .eq(current.suffix(index).unwrap().iter()));
        }
        assert!(original.suffix(original.len() + 1).is_none());
        assert!(current.suffix(current.len() + 1).is_none());
        assert!(original.into_iter().eq(current.into_iter()));
        let empty: [usize; 0] = [];
        assert_eq!(
            DeclSlices::one(&empty).len(),
            super::super::decl_slices::DeclSlices::one(&empty).len()
        );
    }
    #[test]
    fn selected_authored_ordinals_keep_full_values_and_generated_suffix_without_a_second_directory()
    {
        let authored = ["same-prefix\0α", "removed", "same-prefix\0β", "last"];
        let ordinals = [0, 2, 3];
        let generated = ["generated\0尾"];
        let selected =
            super::super::decl_slices::DeclSlices::selected(&authored, &ordinals, &generated)
                .unwrap();
        let independent = [authored[0], authored[2], authored[3], generated[0]];
        assert!(selected.iter().eq(independent.iter()));
        assert!(selected.iter().rev().eq(independent.iter().rev()));
        for start in 0..=independent.len() {
            assert!(selected
                .suffix(start)
                .unwrap()
                .iter()
                .eq(independent[start..].iter()));
        }
        let mut mixed = selected.iter();
        assert_eq!(mixed.next(), Some(&independent[0]));
        assert_eq!(mixed.next_back(), Some(&independent[3]));
        assert_eq!(mixed.next_back(), Some(&independent[2]));
        assert_eq!(mixed.next(), Some(&independent[1]));
        assert_eq!(mixed.next(), None);
        assert_eq!(mixed.next_back(), None);
        assert_eq!(mixed.len(), 0);
        for invalid in [&[0, 0][..], &[2, 0][..], &[authored.len()][..]] {
            assert!(super::super::decl_slices::DeclSlices::selected(
                &authored, invalid, &generated
            )
            .is_none());
        }
    }
}
