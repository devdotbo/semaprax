//! Borrowed scratch identities with actual probes on the receiving ledger.
//! No runtime plan vector is reordered or repaired by this scratch table.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::cleanup_plan::replay) enum Key<'a> {
    Expression(&'a ExpressionId),
    Status(&'a StatusSourceId),
}
pub(in crate::cleanup_plan::replay) struct Ids<'a> {
    entries: Vec<Key<'a>>,
}
impl<'a> Ids<'a> {
    pub(in crate::cleanup_plan::replay) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        budget.reserve_array::<Self>(function, 1, "borrowed scratch identity header")?;
        Ok(Self {
            entries: Vec::new(),
        })
    }
    pub(in crate::cleanup_plan::replay) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(in crate::cleanup_plan::replay) fn contains(
        &self,
        function: &ResolvedFunction,
        key: Key<'_>,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        for actual in &self.entries {
            budget.charge(function, 1, "borrowed scratch actual identity probe")?;
            let same = match (actual, key) {
                (Key::Expression(a), Key::Expression(b)) => {
                    schema::borrowed::same_schema(function, a.as_str(), b.as_str(), budget)?
                }
                (Key::Status(a), Key::Status(b)) => {
                    budget.charge(function, 1, "borrowed scratch actual status lane")?;
                    a.lane == b.lane
                        && schema::borrowed::same_schema(
                            function,
                            a.expression.as_str(),
                            b.expression.as_str(),
                            budget,
                        )?
                }
                _ => false,
            };
            if same {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(in crate::cleanup_plan::replay) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: Key<'a>,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        if self.contains(function, key, budget)? {
            return Ok(false);
        }
        if self.entries.len() == self.entries.capacity() {
            budget.charge(
                function,
                self.entries.len(),
                "borrowed scratch actual backing relocation",
            )?;
        }
        budget.reserve_vec(
            function,
            &mut self.entries,
            1,
            "borrowed scratch identity backing",
        )?;
        budget.charge(function, 1, "borrowed scratch actual identity append")?;
        self.entries.push(key);
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
