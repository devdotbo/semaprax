//! Sorted dictionaries expose each actual key probe and relocation.
use super::*;
use std::cmp::Ordering;
mod keys;
pub(in crate::cleanup_plan::replay::factored) trait Key {
    fn compare(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic>;
}
pub(in crate::cleanup_plan::replay::factored) struct Dictionary<K, V> {
    rows: Vec<(K, V)>,
}
impl<K, V> Default for Dictionary<K, V> {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}
impl<K: Key, V> Dictionary<K, V> {
    pub(in crate::cleanup_plan::replay::factored) fn new() -> Self {
        Self { rows: Vec::new() }
    }
    fn position(
        &self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<Result<usize, usize>, Diagnostic> {
        budget.charge(function, 1, "borrowed dictionary lookup header")?;
        let (mut low, mut high) = (0, self.rows.len());
        while low < high {
            budget.charge(function, 1, "borrowed dictionary actual key probe")?;
            let mid = low + (high - low) / 2;
            match self.rows[mid].0.compare(key, function, budget)? {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Ok(Ok(mid)),
            }
        }
        Ok(Err(low))
    }
    pub(in crate::cleanup_plan::replay::factored) fn get(
        &self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&V>, Diagnostic> {
        Ok(self
            .position(function, key, budget)?
            .ok()
            .map(|i| &self.rows[i].1))
    }
    pub(in crate::cleanup_plan::replay::factored) fn contains(
        &self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        Ok(self.position(function, key, budget)?.is_ok())
    }
    fn vacant(
        &mut self,
        function: &ResolvedFunction,
        index: usize,
        key: K,
        value: V,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.charge(
            function,
            self.rows.len() - index + 1,
            "borrowed dictionary actual insertion shifts",
        )?;
        if self.rows.len() == self.rows.capacity() {
            budget.charge(
                function,
                self.rows.len(),
                "borrowed dictionary backing relocation",
            )?;
        }
        budget.reserve_vec(
            function,
            &mut self.rows,
            1,
            "borrowed dictionary exact slots",
        )?;
        self.rows.insert(index, (key, value));
        Ok(())
    }
    pub(in crate::cleanup_plan::replay::factored) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: K,
        value: V,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        match self.position(function, &key, budget)? {
            Ok(i) => {
                budget.charge(function, 1, "borrowed dictionary replacement")?;
                self.rows[i].1 = value;
            }
            Err(i) => self.vacant(function, i, key, value, budget)?,
        }
        Ok(())
    }
    pub(in crate::cleanup_plan::replay::factored) fn entry(
        &mut self,
        function: &ResolvedFunction,
        key: K,
        budget: &mut ReplayBudget,
    ) -> Result<&mut V, Diagnostic>
    where
        V: Default,
    {
        let index = match self.position(function, &key, budget)? {
            Ok(i) => i,
            Err(i) => {
                self.vacant(function, i, key, V::default(), budget)?;
                i
            }
        };
        Ok(&mut self.rows[index].1)
    }
    pub(in crate::cleanup_plan::replay::factored) fn remove(
        &mut self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if let Ok(index) = self.position(function, key, budget)? {
            budget.charge(
                function,
                self.rows.len() - index,
                "borrowed dictionary actual removal shifts",
            )?;
            self.rows.remove(index);
        }
        Ok(())
    }
    pub(in crate::cleanup_plan::replay::factored) fn len(&self) -> usize {
        self.rows.len()
    }
    pub(in crate::cleanup_plan::replay::factored) fn iter(&self) -> std::slice::Iter<'_, (K, V)> {
        self.rows.iter()
    }
    pub(in crate::cleanup_plan::replay::factored) fn into_rows(self) -> Vec<(K, V)> {
        self.rows
    }
}
