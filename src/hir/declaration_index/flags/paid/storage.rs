//! Caller-owned physical operations; no private quota or ambient factory budget.
use crate::bounded_output::classifier_string::{ClassifierString, GrowthFailure};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedType, TypeFacts};
use std::fmt;

pub(crate) trait Payer {
    fn bytes(&mut self, bytes: usize) -> Result<(), Diagnostic>;
    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic>;
    fn node<K, V>(&mut self) -> Result<(), Diagnostic>;
    fn clone_type(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic>;
    fn boxed_type(&mut self, ty: ResolvedType) -> Result<Box<ResolvedType>, Diagnostic>;
    fn construction_refusal(&mut self, message: &'static str) -> Diagnostic;
    /// Explicit test account only; the default keeps the captured production
    /// logical receiver. This selector grants no payment or subject authority.
    #[cfg(test)]
    fn classifier_sink_is_explicit(&self) -> bool {
        false
    }
    #[cfg(test)]
    fn classifier_reserve_sink(&mut self, _bytes: usize) -> Result<bool, Diagnostic> {
        Err(Diagnostic::io_static(
            "SPX-H006",
            "explicit classifier sink has no payer",
        ))
    }
    /// Explicit test-account work only. Ordinary payers retain their receiver;
    /// this default grants neither a leased account nor a work allowance.
    #[cfg(test)]
    fn oracle_work(&mut self, _units: usize, _phase: &'static str) -> Result<(), Diagnostic> {
        Ok(())
    }
}

impl<P: Payer + ?Sized> Payer for &mut P {
    fn bytes(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        (**self).bytes(bytes)
    }
    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
        (**self).grow(values, additional)
    }
    fn node<K, V>(&mut self) -> Result<(), Diagnostic> {
        (**self).node::<K, V>()
    }
    fn clone_type(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        (**self).clone_type(ty)
    }
    fn boxed_type(&mut self, ty: ResolvedType) -> Result<Box<ResolvedType>, Diagnostic> {
        (**self).boxed_type(ty)
    }
    fn construction_refusal(&mut self, message: &'static str) -> Diagnostic {
        (**self).construction_refusal(message)
    }

    #[cfg(test)]
    fn classifier_sink_is_explicit(&self) -> bool {
        (**self).classifier_sink_is_explicit()
    }
    #[cfg(test)]
    fn classifier_reserve_sink(&mut self, bytes: usize) -> Result<bool, Diagnostic> {
        (**self).classifier_reserve_sink(bytes)
    }
    #[cfg(test)]
    fn oracle_work(&mut self, units: usize, phase: &'static str) -> Result<(), Diagnostic> {
        (**self).oracle_work(units, phase)
    }
}

pub(super) struct Storage<P> {
    pub(super) payer: P,
    pub(super) first: Option<Diagnostic>,
}
impl<P: Payer> Storage<P> {
    pub(super) fn receive<T>(&mut self, result: Result<T, Diagnostic>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                if self.first.is_none() {
                    self.first = Some(error);
                }
                None
            }
        }
    }
    pub(super) fn bytes(&mut self, bytes: usize) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.bytes(bytes);
        self.receive(result)
    }
    pub(super) fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.grow(values, additional);
        self.receive(result)
    }
    pub(super) fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Option<()> {
        self.grow(values, 1)?;
        values.push(value);
        Some(())
    }
    pub(super) fn clone_type(&mut self, ty: &ResolvedType) -> Option<ResolvedType> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.clone_type(ty);
        self.receive(result)
    }
    pub(super) fn substitute(
        &mut self,
        ty: &ResolvedType,
        owner: &DeclarationId,
        arguments: &[ResolvedType],
    ) -> Option<ResolvedType> {
        super::substitute::resolve(ty, owner, arguments, self)
    }
    pub(super) fn boxed_type(&mut self, ty: ResolvedType) -> Option<Box<ResolvedType>> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.boxed_type(ty);
        self.receive(result)
    }
    pub(super) fn node<K, V>(&mut self) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.node::<K, V>();
        self.receive(result)
    }
    /// The original first Diagnostic is retained; a refused receiver performs
    /// no later callback work and never substitutes a semantic None for refusal.
    #[cfg(test)]
    pub(super) fn oracle_work(&mut self, units: usize, phase: &'static str) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.oracle_work(units, phase);
        self.receive(result)
    }
    fn growth<T>(&mut self, result: Result<T, GrowthFailure<Diagnostic>>) -> Option<T> {
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
            // Original CappedString .ok()? drops the partial output on a
            // formatter error without publishing a physical allocation refusal.
            Err(GrowthFailure::Formatter) => {
                #[cfg(test)]
                if self.payer.classifier_sink_is_explicit() {
                    let error = self
                        .payer
                        .construction_refusal("explicit classifier formatter failed");
                    return self.receive(Err(error));
                }
                return None;
            }
        };
        self.receive(Err(error))
    }
    pub(super) fn encoded(&mut self) -> Option<ClassifierString> {
        if self.first.is_some() {
            return None;
        }
        #[cfg(test)]
        if self.payer.classifier_sink_is_explicit() {
            let result = ClassifierString::acquire_explicit(&mut |bytes| self.payer.bytes(bytes));
            return self.growth(result);
        }
        let result = ClassifierString::acquire(&mut |bytes| self.payer.bytes(bytes));
        self.growth(result)
    }
    pub(super) fn append(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        #[cfg(test)]
        if self.payer.classifier_sink_is_explicit() {
            let result = output.capped_explicit(
                arguments,
                &mut self.payer,
                &mut |bytes, payer| payer.bytes(bytes),
                &mut |bytes, payer| payer.classifier_reserve_sink(bytes),
            );
            return self.growth(result);
        }
        let result = output.capped(arguments, &mut |bytes| self.payer.bytes(bytes));
        self.growth(result)
    }
    pub(super) fn append_raw(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = output.raw(arguments, &mut |bytes| self.payer.bytes(bytes));
        self.growth(result)
    }
    pub(super) fn append_strict(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        #[cfg(test)]
        if self.payer.classifier_sink_is_explicit() {
            let result = output.strict_explicit(
                arguments,
                &mut self.payer,
                &mut |bytes, payer| payer.bytes(bytes),
                &mut |bytes, payer| payer.classifier_reserve_sink(bytes),
            );
            return self.growth(result);
        }
        let result = output.strict(arguments, &mut |bytes| self.payer.bytes(bytes));
        self.growth(result)
    }
    pub(super) fn text(&mut self, arguments: fmt::Arguments<'_>) -> Option<String> {
        if self.first.is_some() {
            return None;
        }
        let mut output = self.encoded()?;
        #[cfg(test)]
        if self.payer.classifier_sink_is_explicit() {
            self.append_strict(&mut output, arguments)?;
            return Some(output.into_string());
        }
        let result = output.strict(arguments, &mut |bytes| self.payer.bytes(bytes));
        self.growth(result)?;
        Some(output.into_string())
    }
    pub(super) fn literal(&mut self, value: &str) -> Option<String> {
        #[cfg(test)]
        self.oracle_work(value.len(), "classifier literal bytes")?;
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.bytes(value.len());
        self.receive(result)?;
        let mut text = String::new();
        let result = text.try_reserve_exact(value.len());
        if result.is_err() {
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
    pub(super) fn facts_clone(&mut self, facts: &TypeFacts) -> Option<TypeFacts> {
        Some(TypeFacts {
            copy: facts.copy,
            contains_resource: facts.contains_resource,
            sized: facts.sized,
            needs_drop: facts.needs_drop,
            layout_key: self.literal(&facts.layout_key)?,
        })
    }
}
