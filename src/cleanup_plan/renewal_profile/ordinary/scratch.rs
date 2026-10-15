//! Actual collector storage; the caller lends its existing phase ledgers.
use std::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Work,
    Backing,
    Arithmetic,
    Allocation,
    Capacity,
    Subject,
}

/// No allowance is created here. Both callbacks belong to the enclosing phase.
pub(crate) struct Admission<'a> {
    pub(crate) work: &'a mut dyn FnMut(usize) -> bool,
    pub(crate) backing: &'a mut dyn FnMut(usize) -> bool,
    failed: bool,
    pub(crate) probes: Option<&'a dyn crate::hir::renewal_probes::Index>,
}
impl<'a> Admission<'a> {
    pub(crate) fn new(
        work: &'a mut dyn FnMut(usize) -> bool,
        backing: &'a mut dyn FnMut(usize) -> bool,
    ) -> Self {
        Self {
            work,
            backing,
            failed: false,
            probes: None,
        }
    }
    pub(crate) fn units(&mut self, count: usize) -> Result<(), Refusal> {
        if self.failed || !(self.work)(count) {
            self.failed = true;
            return Err(Refusal::Work);
        }
        Ok(())
    }
    pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), Refusal> {
        if self.failed || !(self.backing)(bytes) {
            self.failed = true;
            return Err(Refusal::Backing);
        }
        Ok(())
    }
    pub(crate) fn arithmetic<T>(&mut self, value: Option<T>) -> Result<T, Refusal> {
        match value {
            Some(value) => Ok(value),
            None => {
                self.failed = true;
                Err(Refusal::Arithmetic)
            }
        }
    }
    pub(crate) fn refuse<T>(&mut self, reason: Refusal) -> Result<T, Refusal> {
        self.failed = true;
        Err(reason)
    }
}

/// Retained header and every replacement request are paid separately. Moving
/// the old elements does not allocate identity strings or duplicate owners.
pub(crate) struct Scratch<T> {
    items: Vec<T>,
}
impl<T> Scratch<T> {
    pub(crate) fn new(admission: &mut Admission<'_>) -> Result<Self, Refusal> {
        admission.reserve(size_of::<Self>())?;
        Ok(Self { items: Vec::new() })
    }
    pub(crate) fn push(&mut self, item: T, admission: &mut Admission<'_>) -> Result<(), Refusal> {
        // A spare slot cannot hide a failure selected by another receiver.
        admission.reserve(0)?;
        admission.units(1)?;
        if self.items.len() == self.items.capacity() {
            let count = admission
                .arithmetic(self.items.capacity().checked_mul(2))?
                .max(1);
            let payload = admission.arithmetic(count.checked_mul(size_of::<T>()))?;
            let bytes = admission.arithmetic(payload.checked_add(size_of::<Vec<T>>()))?;
            admission.reserve(bytes)?;
            let mut replacement = Vec::new();
            if replacement.try_reserve_exact(count).is_err() {
                return admission.refuse(Refusal::Allocation);
            }
            if replacement.capacity() != count {
                if replacement.capacity() > count {
                    let excess = admission
                        .arithmetic((replacement.capacity() - count).checked_mul(size_of::<T>()))?;
                    admission.reserve(excess)?;
                }
                return admission.refuse(Refusal::Capacity);
            }
            admission.units(self.items.len())?;
            replacement.append(&mut self.items);
            self.items = replacement;
        }
        self.items.push(item);
        Ok(())
    }
    pub(crate) fn pop(&mut self, admission: &mut Admission<'_>) -> Result<Option<T>, Refusal> {
        admission.reserve(0)?;
        admission.units(1)?;
        Ok(self.items.pop())
    }
    pub(crate) fn reverse_suffix(
        &mut self,
        start: usize,
        admission: &mut Admission<'_>,
    ) -> Result<(), Refusal> {
        admission.units(self.items.len() - start)?;
        self.items[start..].reverse();
        Ok(())
    }
    pub(crate) fn as_slice(&self) -> &[T] {
        &self.items
    }
    pub(crate) fn into_vec(self) -> Vec<T> {
        self.items
    }
}
