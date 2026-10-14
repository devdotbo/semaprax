//! Held-program memo of executable runtime observations for call parameters.
//!
//! A `Call` is a sealed ordinal into the immutable `ResolvedProgram` the
//! validator already holds. It is built only by `Bound::call`, with the exact
//! source-order predicates of `ResolvedProgram::resolve_call_target`. A
//! `Parameter` is built only from a `Call`. A memo entry is authorized only
//! when the carrier names the bound program and its parameter type is the very
//! reference the caller is classifying. Entries are keyed by numeric slot and
//! parameter index and hold only a successful runtime verdict; nothing is
//! owned, cloned or keyed by address.
use super::*;
use crate::hir::ids::{DeclarationId, FunctionInstanceId};
use crate::hir::nodes::{ResolvedFunction, ResolvedParam, ResolvedProgram};
use crate::hir::owned_text_record::paid;

/// Storage cap for the optimization only; a full table stops remembering.
pub(super) const MAX_CACHE_ENTRIES: usize = 128;
const FIRST_CAPACITY: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    Function(usize),
    Instance(usize),
}

#[derive(Clone, Copy)]
pub(super) struct Call<'a> {
    program: &'a ResolvedProgram,
    slot: Slot,
}

impl<'a> Call<'a> {
    /// Constant-time ordinal get.
    pub(super) fn function(&self) -> &'a ResolvedFunction {
        let program: &'a ResolvedProgram = self.program;
        match self.slot {
            Slot::Function(at) => &program.functions[at],
            Slot::Instance(at) => &program.function_instances[at].function,
        }
    }

    pub(super) fn parameter(&self, index: usize) -> Parameter<'a> {
        Parameter { call: *self, index }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Parameter<'a> {
    call: Call<'a>,
    index: usize,
}

impl<'a> Parameter<'a> {
    /// The actual immutable parameter of the held program.
    pub(super) fn param(&self) -> &'a ResolvedParam {
        &self.call.function().params[self.index]
    }
}

/// Opaque authenticated memo key, produced only by `Bound::authenticate`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key {
    slot: Slot,
    parameter: usize,
}

struct Held {
    key: Key,
    copy: Option<bool>,
    runtime: Option<bool>,
}

pub(super) struct Bound<'a> {
    program: &'a ResolvedProgram,
    held: Vec<Held>,
}

impl<'a> Bound<'a> {
    pub(super) fn new(program: &'a ResolvedProgram) -> Self {
        Self {
            program,
            held: Vec::new(),
        }
    }

    /// The only constructor of `Call`: the original first-match predicates.
    pub(super) fn call(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<Call<'a>> {
        let program = self.program;
        let slot = match instance {
            None => program
                .functions
                .iter()
                .position(|function| function.id == *callee)
                .map(Slot::Function),
            Some(instance) => program
                .function_instances
                .iter()
                .position(|candidate| candidate.id == *instance && candidate.template == *callee)
                .map(Slot::Instance),
        }?;
        Some(Call { program, slot })
    }

    /// Pointer identity authenticates only; it is never a key.
    pub(super) fn authenticate(&self, parameter: &Parameter<'_>, ty: &ResolvedType) -> Option<Key> {
        if !std::ptr::eq(parameter.call.program, self.program) {
            return None;
        }
        let actual = parameter.call.function().params.get(parameter.index)?;
        std::ptr::eq(&actual.ty, ty).then_some(Key {
            slot: parameter.call.slot,
            parameter: parameter.index,
        })
    }

    pub(super) fn recall(&self, key: Key) -> Option<bool> {
        let at = self
            .held
            .binary_search_by_key(&key, |entry| entry.key)
            .ok()?;
        self.held[at].runtime
    }

    pub(super) fn recall_copy(&self, key: Key) -> Option<bool> {
        let at = self
            .held
            .binary_search_by_key(&key, |entry| entry.key)
            .ok()?;
        self.held[at].copy
    }
    // A constant false runtime observation only refines existing paid storage.
    pub(super) fn settle_runtime_false(&mut self, key: Key) {
        if let Ok(at) = self.held.binary_search_by_key(&key, |entry| entry.key) {
            self.held[at].runtime.get_or_insert(false);
        }
    }

    // Only successful pure results arrive here. A full table, or a payer that
    // declines growth, leaves the verdict and every sticky flag untouched.
    pub(super) fn remember(&mut self, key: Key, value: bool) -> Result<(), Diagnostic> {
        if !value {
            paid::fund(Some(0))?;
            self.settle_runtime_false(key);
            return Ok(());
        }
        self.store(key, None, Some(value))
    }
    pub(super) fn remember_copy(&mut self, key: Key, value: bool) -> Result<(), Diagnostic> {
        self.store(key, Some(value), None)
    }
    fn store(
        &mut self,
        key: Key,
        copy: Option<bool>,
        runtime: Option<bool>,
    ) -> Result<(), Diagnostic> {
        paid::fund(Some(0))?;
        let at = match self.held.binary_search_by_key(&key, |entry| entry.key) {
            Ok(at) => {
                let entry = &mut self.held[at];
                entry.copy = entry.copy.or(copy);
                entry.runtime = entry.runtime.or(runtime);
                return Ok(());
            }
            Err(at) => at,
        };
        if self.held.len() >= MAX_CACHE_ENTRIES {
            return Ok(());
        }
        if self.held.len() == self.held.capacity() {
            let capacity = match self.held.capacity() {
                0 => FIRST_CAPACITY,
                current => (current * 2).min(MAX_CACHE_ENTRIES),
            };
            // The full replacement backing is paid before it is acquired.
            let bytes = capacity * std::mem::size_of::<Held>();
            paid::fund(Some(0))?;
            if crate::bounded_output::active_remaining().is_none() {
                return Ok(());
            }
            if !crate::bounded_output::reserve_active_preserving(bytes, 0) {
                return Ok(());
            }
            if self
                .held
                .try_reserve_exact(capacity - self.held.len())
                .is_err()
                || self.held.capacity() != capacity
            {
                // Match the existing paid Vec receiver's sticky allocator
                // failure contract. No entry has been published.
                return paid::fund(None);
            }
        }
        self.held.insert(at, Held { key, copy, runtime });
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.held.len()
    }

    #[cfg(test)]
    pub(super) fn owned_capacity(&self) -> usize {
        self.held.capacity() * std::mem::size_of::<Held>()
    }
}

#[cfg(test)]
pub(super) mod tests;

#[cfg(test)]
mod source_audit;
