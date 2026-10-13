//! Raw semantic text and the original key's sole capped nominal-child frontier.
use crate::bounded_output::classifier_string::{ClassifierString, GrowthFailure};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedFunction, TypeFactsPayer};
use std::fmt;

pub(super) struct Storage<'a, P> {
    pub(super) payer: &'a mut P,
    pub(super) first: Option<Diagnostic>,
}
impl<P: TypeFactsPayer> Storage<'_, P> {
    fn receive<T>(&mut self, result: Result<T, Diagnostic>) -> Option<T> {
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
    fn growth<T>(&mut self, result: Result<T, GrowthFailure<Diagnostic>>) -> Option<T> {
        let error = match result {
            Ok(value) => return Some(value),
            Err(GrowthFailure::Payment(error)) => error,
            Err(GrowthFailure::Arithmetic) => self
                .payer
                .construction_refusal("cleanup diagnostic string capacity overflowed"),
            Err(GrowthFailure::Allocation) => self
                .payer
                .construction_refusal("cleanup diagnostic string allocation failed"),
            Err(GrowthFailure::Capacity) => self.payer.construction_refusal(
                "cleanup diagnostic string capacity disagrees with its reservation",
            ),
            // Both original raw String and capped String writes expect success.
            Err(GrowthFailure::Formatter) => {
                panic!("a formatting trait implementation returned an error")
            }
        };
        self.receive(Err(error))
    }
    pub(super) fn encoded(&mut self) -> Option<ClassifierString> {
        if self.first.is_some() {
            return None;
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
    pub(super) fn text(&mut self, arguments: fmt::Arguments<'_>) -> Option<String> {
        let mut output = self.encoded()?;
        self.append_raw(&mut output, arguments)?;
        Some(output.into_string())
    }
    pub(super) fn literal(&mut self, value: &str) -> Option<String> {
        self.bytes(value.len())?;
        let mut text = String::new();
        if text.try_reserve_exact(value.len()).is_err() {
            let error = self
                .payer
                .construction_refusal("cleanup diagnostic literal allocation failed");
            return self.receive(Err(error));
        }
        if text.capacity() != value.len() {
            let error = self.payer.construction_refusal(
                "cleanup diagnostic literal capacity disagrees with its reservation",
            );
            return self.receive(Err(error));
        }
        text.push_str(value);
        Some(text)
    }
}
pub(super) fn raw<P: TypeFactsPayer>(
    payer: &mut P,
    arguments: fmt::Arguments<'_>,
) -> Result<String, Diagnostic> {
    payer.bytes(std::mem::size_of::<Storage<'_, P>>())?;
    let mut storage = Storage { payer, first: None };
    let value = storage.text(arguments);
    match storage.first {
        Some(error) => Err(error),
        None => Ok(value.expect("raw diagnostic formatter completed")),
    }
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn replay_diagnostic<P: TypeFactsPayer>(
    function: &ResolvedFunction,
    message: &str,
    payer: &mut P,
) -> Result<Diagnostic, Diagnostic> {
    let text = raw(
        payer,
        format_args!(
            "cleanup plan for function `{}` failed independent replay: {}",
            function.id, message
        ),
    )?;
    Ok(Diagnostic::io("SPX-H006", text))
}
