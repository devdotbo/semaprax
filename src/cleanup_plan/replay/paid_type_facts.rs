//! One existing replay payer for classifier constructors and their receiving
//! lifetime. No work allowance, fallback quota or fact authority is introduced.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::hir::{DeclarationIndex, TypeFactsPayer};

struct Payer<'a> {
    function: &'a ResolvedFunction,
    budget: &'a mut ReplayBudget,
    ledger: ClassifierLedger,
}
impl Payer<'_> {
    fn current(&mut self) -> Result<(), Diagnostic> {
        if self.ledger.current() {
            return Ok(());
        }
        Err(self.budget.budget_refusal(
            self.function,
            "cleanup replay classifier payer is outside its current invocation",
            "",
        ))
    }
}
impl TypeFactsPayer for Payer<'_> {
    fn bytes(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.current()?;
        self.budget
            .reserve_bytes(self.function, bytes, "type-fact classifier backing")
    }
    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
        self.current()?;
        self.budget.reserve_vec(
            self.function,
            values,
            additional,
            "type-fact classifier vector",
        )
    }
    fn node<K, V>(&mut self) -> Result<(), Diagnostic> {
        self.current()?;
        self.budget
            .reserve_btree_node::<K, V>(self.function, "type-fact classifier ordered tree")
    }
    fn clone_type(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        self.current()?;
        let owns_backing = match ty {
            ResolvedType::Nominal { arguments, .. } => !arguments.is_empty(),
            ResolvedType::Function { .. } => true,
            _ => false,
        };
        if !owns_backing {
            self.bytes(0)?;
            return Ok(ty.clone());
        }
        // The original classifier has no new depth/census quota. Use a real
        // paid ancestor cursor instead of converting a fixed optimization
        // census bound into admission refusal for a wider/deeper current type.
        self.bytes(std::mem::size_of::<Vec<(&ResolvedType, usize)>>())?;
        let mut pending = Vec::new();
        self.grow(&mut pending, 1)?;
        pending.push((ty, 0usize));
        while let Some((current, child)) = pending.last().copied() {
            if child == 0 {
                let bytes = match current {
                    ResolvedType::Nominal { arguments, .. } => arguments
                        .len()
                        .checked_mul(std::mem::size_of::<ResolvedType>()),
                    ResolvedType::Function { parameters, .. } => parameters
                        .len()
                        .checked_mul(std::mem::size_of::<ResolvedType>())
                        .and_then(|n| n.checked_add(std::mem::size_of::<ResolvedType>())),
                    _ => Some(0),
                }
                .ok_or_else(|| {
                    self.construction_refusal("cleanup replay classifier clone layout overflowed")
                })?;
                self.bytes(bytes)?;
            }
            let next = match current {
                ResolvedType::Nominal { arguments, .. } => arguments.get(child),
                ResolvedType::Function { parameters, result } => {
                    if child == parameters.len() {
                        Some(result.as_ref())
                    } else {
                        parameters.get(child)
                    }
                }
                _ => None,
            };
            if let Some(next) = next {
                pending.last_mut().expect("classifier ancestor present").1 += 1;
                self.grow(&mut pending, 1)?;
                pending.push((next, 0));
            } else {
                pending.pop();
            }
        }
        Ok(ty.clone())
    }
    fn boxed_type(&mut self, ty: ResolvedType) -> Result<Box<ResolvedType>, Diagnostic> {
        self.bytes(std::mem::size_of::<ResolvedType>())?;
        Ok(Box::new(ty))
    }
    fn construction_refusal(&mut self, message: &'static str) -> Diagnostic {
        self.budget.budget_refusal(self.function, message, "")
    }
}

pub(super) fn flags(
    index: &DeclarationIndex,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<Option<crate::hir::TypeFlags>, Diagnostic> {
    #[cfg(test)]
    if SAVED.with(std::cell::Cell::get) {
        return Ok(index.type_flags(ty));
    }
    index.type_flags_paid(
        ty,
        Payer {
            function,
            budget,
            ledger: ClassifierLedger::capture(),
        },
    )
}

mod droppability;
pub(super) fn needs_drop(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    if SAVED.with(std::cell::Cell::get) {
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    let mut payer = Payer {
        function,
        budget,
        ledger: ClassifierLedger::capture(),
    };
    match droppability::needs_drop_paid(program, ty, &mut payer) {
        Ok(value) => Ok(value),
        Err(droppability::Failure::Physical(error)) => Err(error),
        Err(droppability::Failure::Semantic(message)) => Err(droppability::replay_diagnostic(
            function, &message, &mut payer,
        )?),
    }
}
#[cfg(test)]
thread_local! { static SAVED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
fn with_saved_classifier<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SAVED.with(|saved| saved.set(self.0));
        }
    }
    let _restore = Restore(SAVED.with(|saved| saved.replace(true)));
    run()
}
#[cfg(test)]
mod tests;
#[cfg(test)]
mod zero_tests;
