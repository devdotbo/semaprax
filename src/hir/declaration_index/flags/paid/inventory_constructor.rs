//! Genuine caller work for the complete type key/substitution constructors.
//! The prior paid receiver stays independently compiled and unchanged.
use super::storage::{Payer as BackingPayer, Storage};
use crate::bounded_output::classifier_string::{ClassifierString, GrowthFailure};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedType};
use std::fmt;
mod key;
mod substitute;

pub(crate) trait InventoryPayer: BackingPayer {
    fn inventory_work(&mut self, units: usize, phase: &'static str) -> Result<(), Diagnostic>;
    fn inventory_clone(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic>;
}
impl<P: InventoryPayer + ?Sized> InventoryPayer for &mut P {
    fn inventory_work(&mut self, units: usize, phase: &'static str) -> Result<(), Diagnostic> {
        (**self).inventory_work(units, phase)
    }
    fn inventory_clone(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        (**self).inventory_clone(ty)
    }
}

impl<P: InventoryPayer> Storage<P> {
    pub(super) fn work(&mut self, units: usize, phase: &'static str) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.inventory_work(units, phase);
        self.receive(result)
    }
    pub(super) fn inventory_grow<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Option<()> {
        self.work(0, "inventory vector authority")?;
        let required = values.len().checked_add(additional)?;
        self.work(additional, "inventory published vector slots")?;
        self.work(additional, "inventory eventual slot releases")?;
        if required > values.capacity() {
            self.work(values.len(), "inventory retained vector elements moved")?;
            if values.capacity() != 0 {
                self.work(1, "inventory old vector backing release")?;
            }
        }
        self.grow(values, additional)
    }
    pub(super) fn inventory_push<T>(&mut self, values: &mut Vec<T>, value: T) -> Option<()> {
        self.inventory_grow(values, 1)?;
        values.push(value);
        Some(())
    }
    pub(super) fn inventory_clone(&mut self, ty: &ResolvedType) -> Option<ResolvedType> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.inventory_clone(ty);
        self.receive(result)
    }
    pub(super) fn owner_equal(
        &mut self,
        left: &DeclarationId,
        right: &DeclarationId,
    ) -> Option<bool> {
        self.work(1, "inventory substitution owner comparison")?;
        let mut left = left.as_str().bytes();
        let mut right = right.as_str().bytes();
        loop {
            self.work(1, "inventory substitution owner byte comparison")?;
            match (left.next(), right.next()) {
                (None, None) => return Some(true),
                (Some(a), Some(b)) if a == b => {}
                _ => return Some(false),
            }
        }
    }
    fn inventory_growth<T>(&mut self, result: Result<T, GrowthFailure<Diagnostic>>) -> Option<T> {
        let error = match result {
            Ok(value) => return Some(value),
            Err(GrowthFailure::Payment(error)) => error,
            Err(GrowthFailure::Arithmetic) => self
                .payer
                .construction_refusal("cleanup replay classifier string capacity overflowed"),
            Err(GrowthFailure::Allocation) => self
                .payer
                .construction_refusal("cleanup replay classifier string allocation failed"),
            Err(GrowthFailure::Capacity) => self.payer.construction_refusal(
                "cleanup replay classifier string capacity disagrees with its reservation",
            ),
            Err(GrowthFailure::Formatter) => return None,
        };
        self.receive(Err(error))
    }
    pub(super) fn inventory_encoded(&mut self) -> Option<ClassifierString> {
        if self.first.is_some() {
            return None;
        }
        self.work(1, "inventory receiving classifier string release")?;
        let result = ClassifierString::acquire_with_work(
            &mut self.payer,
            &mut |bytes, payer| payer.bytes(bytes),
            &mut |units, payer| payer.inventory_work(units, "inventory classifier text work"),
        );
        self.inventory_growth(result)
    }
    pub(super) fn inventory_append(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = output.capped_with_work(
            arguments,
            &mut self.payer,
            &mut |bytes, payer| payer.bytes(bytes),
            &mut |units, payer| payer.inventory_work(units, "inventory classifier text work"),
        );
        self.inventory_growth(result)
    }
    pub(super) fn inventory_append_raw(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = output.raw_with_work(
            arguments,
            &mut self.payer,
            &mut |bytes, payer| payer.bytes(bytes),
            &mut |units, payer| payer.inventory_work(units, "inventory classifier text work"),
        );
        self.inventory_growth(result)
    }
    pub(super) fn inventory_append_strict(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = output.strict_with_work(
            arguments,
            &mut self.payer,
            &mut |bytes, payer| payer.bytes(bytes),
            &mut |units, payer| payer.inventory_work(units, "inventory classifier text work"),
        );
        self.inventory_growth(result)
    }
    pub(super) fn inventory_text(&mut self, arguments: fmt::Arguments<'_>) -> Option<String> {
        let mut output = self.inventory_encoded()?;
        self.inventory_append_strict(&mut output, arguments)?;
        Some(output.into_string())
    }
    pub(super) fn inventory_literal(&mut self, value: &str) -> Option<String> {
        self.work(value.len(), "inventory classifier literal bytes")?;
        self.bytes(value.len())?;
        let mut text = String::new();
        if text.try_reserve_exact(value.len()).is_err() {
            let error = self
                .payer
                .construction_refusal("cleanup replay classifier literal allocation failed");
            return self.receive(Err(error));
        }
        if text.capacity() != value.len() {
            let error = self.payer.construction_refusal(
                "cleanup replay classifier literal capacity disagrees with its reservation",
            );
            return self.receive(Err(error));
        }
        text.push_str(value);
        Some(text)
    }
}

pub(crate) fn identity<P: InventoryPayer>(
    ty: &ResolvedType,
    mut payer: P,
) -> Result<Option<String>, Diagnostic> {
    payer.inventory_work(0, "inventory identity authority")?;
    payer.bytes(std::mem::size_of::<Storage<P>>())?;
    let mut storage = Storage { payer, first: None };
    let value = key::identity(ty, &mut storage);
    match storage.first {
        Some(error) => Err(error),
        None => Ok(value),
    }
}
pub(crate) fn substitute<P: InventoryPayer>(
    ty: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
    mut payer: P,
) -> Result<Option<ResolvedType>, Diagnostic> {
    payer.inventory_work(0, "inventory substitution authority")?;
    payer.bytes(std::mem::size_of::<Storage<P>>())?;
    let mut storage = Storage { payer, first: None };
    let value = substitute::resolve(ty, owner, arguments, &mut storage);
    match storage.first {
        Some(error) => Err(error),
        None => Ok(value),
    }
}
