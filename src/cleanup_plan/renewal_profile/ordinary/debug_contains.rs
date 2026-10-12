//! Exact legacy Debug substring semantics without an argument-sized String.
use super::{Admission, Refusal};
use std::fmt::{self, Write};
use std::mem::size_of;
struct Count<'a, 'b> {
    admission: &'a mut Admission<'b>,
    bytes: usize,
    refusal: Option<Refusal>,
}
impl Write for Count<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        match self.admission.units(text.len()).and_then(|()| {
            self.admission
                .arithmetic(self.bytes.checked_add(text.len()))
        }) {
            Ok(bytes) => {
                self.bytes = bytes;
                Ok(())
            }
            Err(error) => {
                self.refusal = Some(error);
                Err(fmt::Error)
            }
        }
    }
}
struct Fill<'a, 'b> {
    admission: &'a mut Admission<'b>,
    text: &'a mut String,
    refusal: Option<Refusal>,
}
impl Write for Fill<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if let Err(error) = self.admission.units(text.len()) {
            self.refusal = Some(error);
            return Err(fmt::Error);
        }
        if text.len() > self.text.capacity() - self.text.len() {
            self.refusal = Some(Refusal::Capacity);
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
struct Search<'a, 'b> {
    admission: &'a mut Admission<'b>,
    needle: &'a [u8],
    prefix: &'a [usize],
    matched: usize,
    found: bool,
    refusal: Option<Refusal>,
}
impl Write for Search<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if let Err(error) = self.admission.units(1) {
                self.refusal = Some(error);
                return Err(fmt::Error);
            }
            if self.found {
                continue;
            }
            while self.matched != 0 {
                if let Err(error) = self.admission.units(1) {
                    self.refusal = Some(error);
                    return Err(fmt::Error);
                }
                if self.needle[self.matched] == byte {
                    break;
                }
                self.matched = self.prefix[self.matched - 1];
            }
            if let Err(error) = self.admission.units(1) {
                self.refusal = Some(error);
                return Err(fmt::Error);
            }
            if self.needle[self.matched] == byte {
                self.matched += 1;
            }
            self.found = self.matched == self.needle.len();
        }
        Ok(())
    }
}
pub(super) struct Needle {
    text: String,
    prefix: Vec<usize>,
}
impl Needle {
    pub(super) fn new(
        value: &impl fmt::Debug,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.reserve(size_of::<Self>())?;
        let mut count = Count {
            admission,
            bytes: 0,
            refusal: None,
        };
        if fmt::write(&mut count, format_args!("{value:?}")).is_err() {
            return count
                .admission
                .refuse(count.refusal.unwrap_or(Refusal::Arithmetic));
        }
        let length = count.bytes;
        let prefix_bytes = admission.arithmetic(length.checked_mul(size_of::<usize>()))?;
        let bytes = admission.arithmetic(length.checked_add(prefix_bytes))?;
        admission.reserve(bytes)?;
        let mut text = String::new();
        let mut prefix = Vec::new();
        if text.try_reserve_exact(length).is_err() || prefix.try_reserve_exact(length).is_err() {
            return admission.refuse(Refusal::Allocation);
        }
        if text.capacity() != length || prefix.capacity() != length {
            let text_extra = text.capacity().saturating_sub(length);
            let prefix_extra = admission.arithmetic(
                prefix
                    .capacity()
                    .saturating_sub(length)
                    .checked_mul(size_of::<usize>()),
            )?;
            let extra = admission.arithmetic(text_extra.checked_add(prefix_extra))?;
            admission.reserve(extra)?;
            return admission.refuse(Refusal::Capacity);
        }
        let mut fill = Fill {
            admission,
            text: &mut text,
            refusal: None,
        };
        if fmt::write(&mut fill, format_args!("{value:?}")).is_err() {
            return fill
                .admission
                .refuse(fill.refusal.unwrap_or(Refusal::Arithmetic));
        }
        if text.len() != length {
            return admission.refuse(Refusal::Capacity);
        }
        admission.units(length)?;
        prefix.resize(length, 0);
        let mut matched = 0;
        for index in 1..length {
            admission.units(1)?;
            while matched != 0 {
                admission.units(1)?;
                if text.as_bytes()[matched] == text.as_bytes()[index] {
                    break;
                }
                matched = prefix[matched - 1];
            }
            admission.units(1)?;
            if text.as_bytes()[matched] == text.as_bytes()[index] {
                matched += 1;
            }
            prefix[index] = matched;
        }
        Ok(Self { text, prefix })
    }
    pub(super) fn contains(
        &self,
        value: &impl fmt::Debug,
        admission: &mut Admission<'_>,
    ) -> Result<bool, Refusal> {
        if self.text.is_empty() {
            return Ok(true);
        }
        let mut search = Search {
            admission,
            needle: self.text.as_bytes(),
            prefix: &self.prefix,
            matched: 0,
            found: false,
            refusal: None,
        };
        if fmt::write(&mut search, format_args!("{value:?}")).is_err() {
            return search
                .admission
                .refuse(search.refusal.unwrap_or(Refusal::Arithmetic));
        }
        Ok(search.found)
    }
}
