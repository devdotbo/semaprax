//! Indexed AVL arena: deterministic logarithmic numeric-key probes and inserts.
//! No address order, shifting sorted prefix, node allocation or traversal stack.
use super::*;
use std::cmp::Ordering;

struct Node {
    key: (usize, usize),
    value: (usize, usize),
    left: Option<usize>,
    right: Option<usize>,
    parent: Option<usize>,
    height: usize,
}
#[derive(Default)]
pub(super) struct Ordinals {
    rows: Vec<Node>,
    root: Option<usize>,
}
impl Ordinals {
    pub(super) fn new() -> Self { Self::default() }
    fn compare(function: &ResolvedFunction, budget: &mut ReplayBudget, a: (usize, usize), b: (usize, usize)) -> Result<Ordering, Diagnostic> {
        budget.charge(function, 1, "ordinal memo first key field")?;
        let first = a.0.cmp(&b.0);
        if first != Ordering::Equal { return Ok(first); }
        budget.charge(function, 1, "ordinal memo continuation field")?;
        Ok(a.1.cmp(&b.1))
    }
    fn position(&self, function: &ResolvedFunction, key: (usize, usize), budget: &mut ReplayBudget) -> Result<Result<usize, (Option<usize>, bool)>, Diagnostic> {
        budget.charge(function, 1, "ordinal memo root probe")?;
        let mut cursor = self.root;
        let mut parent = None;
        let mut left = false;
        while let Some(index) = cursor {
            budget.charge(function, 1, "ordinal memo actual node probe")?;
            let order = Self::compare(function, budget, key, self.rows[index].key)?;
            if order == Ordering::Equal { return Ok(Ok(index)); }
            parent = Some(index);
            left = order == Ordering::Less;
            cursor = if left { self.rows[index].left } else { self.rows[index].right };
        }
        Ok(Err((parent, left)))
    }
    pub(super) fn get(&self, function: &ResolvedFunction, key: &(usize, usize), budget: &mut ReplayBudget) -> Result<Option<&(usize, usize)>, Diagnostic> {
        Ok(self.position(function, *key, budget)?.ok().map(|i| &self.rows[i].value))
    }
    fn height(&self, index: Option<usize>) -> usize {
        index.map_or(0, |i| self.rows[i].height)
    }
    fn heights(&self, function: &ResolvedFunction, index: usize, budget: &mut ReplayBudget) -> Result<(usize, usize), Diagnostic> {
        budget.charge(function, 2, "ordinal memo actual child height probes")?;
        Ok((self.height(self.rows[index].left), self.height(self.rows[index].right)))
    }
    fn update(&mut self, function: &ResolvedFunction, index: usize, budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
        let (left, right) = self.heights(function, index, budget)?;
        let height = left.max(right).checked_add(1).ok_or_else(|| replay_error(function, "ordinal memo height overflowed"))?;
        budget.charge(function, 1, "ordinal memo height publication")?;
        self.rows[index].height = height;
        Ok(())
    }
    fn rotate(&mut self, function: &ResolvedFunction, root: usize, left: bool, budget: &mut ReplayBudget) -> Result<usize, Diagnostic> {
        // Pivot, middle and parent probes plus optional parent-side selection
        // precede five link publications and the optional middle parent.
        budget.charge(function, 3 + usize::from(self.rows[root].parent.is_some()), "ordinal memo rotation link probes")?;
        let pivot = if left { self.rows[root].right } else { self.rows[root].left }.expect("AVL imbalance has a child");
        let middle = if left { self.rows[pivot].left } else { self.rows[pivot].right };
        let parent = self.rows[root].parent;
        budget.charge(function, 5 + usize::from(middle.is_some()), "ordinal memo rotation link publications")?;
        if left {
            self.rows[root].right = middle;
            self.rows[pivot].left = Some(root);
        } else {
            self.rows[root].left = middle;
            self.rows[pivot].right = Some(root);
        }
        if let Some(middle) = middle { self.rows[middle].parent = Some(root); }
        self.rows[root].parent = Some(pivot);
        self.rows[pivot].parent = parent;
        if let Some(parent) = parent {
            if self.rows[parent].left == Some(root) { self.rows[parent].left = Some(pivot); }
            else { self.rows[parent].right = Some(pivot); }
        } else { self.root = Some(pivot); }
        self.update(function, root, budget)?;
        self.update(function, pivot, budget)?;
        Ok(pivot)
    }
    pub(super) fn insert(&mut self, function: &ResolvedFunction, key: (usize, usize), value: (usize, usize), budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
        let (parent, left) = match self.position(function, key, budget)? {
            Ok(index) => {
                budget.charge(function, 1, "ordinal memo value replacement")?;
                self.rows[index].value = value;
                return Ok(());
            }
            Err(place) => place,
        };
        if self.rows.len() == self.rows.capacity() {
            budget.charge(function, self.rows.len(), "ordinal memo backing relocation")?;
        }
        budget.reserve_vec(function, &mut self.rows, 1, "ordinal memo exact arena slots")?;
        budget.charge(function, 2, "ordinal memo node and parent publication")?;
        let index = self.rows.len();
        self.rows.push(Node { key, value, left: None, right: None, parent, height: 1 });
        if let Some(parent) = parent {
            if left { self.rows[parent].left = Some(index); } else { self.rows[parent].right = Some(index); }
        } else { self.root = Some(index); }
        let mut cursor = parent;
        while let Some(index) = cursor {
            budget.charge(function, 1, "ordinal memo actual ancestor")?;
            self.update(function, index, budget)?;
            let (left, right) = self.heights(function, index, budget)?;
            let mut top = index;
            if left > right && left - right > 1 {
                let child = self.rows[index].left.expect("left height is nonzero");
                let (a, b) = self.heights(function, child, budget)?;
                if a < b { self.rotate(function, child, true, budget)?; }
                top = self.rotate(function, index, false, budget)?;
            } else if right > left && right - left > 1 {
                let child = self.rows[index].right.expect("right height is nonzero");
                let (a, b) = self.heights(function, child, budget)?;
                if a > b { self.rotate(function, child, false, budget)?; }
                top = self.rotate(function, index, true, budget)?;
            }
            cursor = self.rows[top].parent;
        }
        Ok(())
    }
}
