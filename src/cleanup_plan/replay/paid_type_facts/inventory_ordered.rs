//! Invocation-local ordered memo for the private streaming-inventory classifier.
//! An iterative AVL keyed by the complete owned `String`, with lexical UTF-8 byte
//! order (embedded null included) matching `BTreeMap<String, V>`. Every key
//! comparison, node visit, rotation, unlink and backing growth is charged to the
//! caller's existing replay work authority. Nothing is cached by fingerprint or
//! stable id alone, and no quota or session reset is introduced.
use super::*;
use std::cmp::Ordering;

// Literal bounded field-group work, prepaid before each complete fixed-size
// action. Rotation covers two links, two refreshed heights and their child-link
// and height reads (at most sixteen groups). Rebalance covers both candidate
// heights, heavy-child selection, four fixed link/height groups, and refresh.
// These are conservative operation bounds, not byte counts or new allowances.
const ROTATION_UNITS: usize = 16;
const VISIT_UNITS: usize = 16;
// An unlink takes and replaces entries (2), writes the parent link (1), writes
// three slot fields (3) and the free head (1).
const UNLINK_UNITS: usize = 7;
const CORRUPT: &str = "inventory ordered map link was inconsistent";

struct Node<V> {
    entry: Option<(String, V)>,
    // A free slot reuses `left` as the next free index.
    left: Option<usize>,
    right: Option<usize>,
    height: u32,
}
impl<V> Node<V> {
    fn new(key: String, value: V) -> Self {
        Self {
            entry: Some((key, value)),
            left: None,
            right: None,
            height: 1,
        }
    }
}

pub(super) struct Map<V> {
    nodes: Vec<Node<V>>,
    root: Option<usize>,
    free: Option<usize>,
    // Reusable paid ancestor frontier; `true` means the descent went right.
    path: Vec<(usize, bool)>,
    poisoned: std::cell::Cell<bool>,
}

fn corrupt(payer: &mut Payer<'_>) -> Diagnostic {
    payer.construction_refusal(CORRUPT)
}

fn reserve_one<T>(payer: &mut Payer<'_>, items: &mut Vec<T>) -> Result<(), Diagnostic> {
    if items.len() < items.capacity() {
        return Ok(());
    }
    // The physical payer pays full replacement backing while old backing lives.
    // Separately prepay initialized elements copied and the old buffer release;
    // capacity bytes are never converted into work.
    inventory_metadata::step(payer, items.len())?;
    if items.capacity() != 0 {
        inventory_metadata::step(payer, 1)?;
    }
    let additional = items.len().max(4);
    payer.grow(items, additional)?;
    if items.len() < items.capacity() {
        Ok(())
    } else {
        Err(payer.construction_refusal("inventory ordered map growth provided no capacity"))
    }
}

// Explicit fallible lexical comparison: one visit, then one unit before each
// next UTF-8 byte pair, including the pair that observes end of input.
fn compare_keys(a: &str, b: &str, payer: &mut Payer<'_>) -> Result<Ordering, Diagnostic> {
    inventory_metadata::step(payer, 1)?;
    let mut left = a.bytes();
    let mut right = b.bytes();
    loop {
        inventory_metadata::step(payer, 1)?;
        match (left.next(), right.next()) {
            (None, None) => return Ok(Ordering::Equal),
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
            (Some(x), Some(y)) if x != y => return Ok(x.cmp(&y)),
            _ => {}
        }
    }
}

impl<V> Map<V> {
    /// Pays the actual header, then prepays the finite release of both vectors
    /// before anything is constructed. Slots prepay their own release visit.
    pub(super) fn acquire(payer: &mut Payer<'_>) -> Result<Self, Diagnostic> {
        inventory_metadata::step(payer, 1)?;
        let function = payer.function;
        payer.budget.reserve_bytes(
            function,
            std::mem::size_of::<Self>(),
            "inventory ordered map header",
        )?;
        inventory_metadata::step(payer, 2)?;
        Ok(Self {
            nodes: Vec::new(),
            root: None,
            free: None,
            path: Vec::new(),
            poisoned: std::cell::Cell::new(false),
        })
    }

    pub(super) fn get(&self, key: &str, payer: &mut Payer<'_>) -> Result<Option<&V>, Diagnostic> {
        let found = self.check(payer).and_then(|()| self.find(key, payer));
        let slot = self.settle(found)?;
        Ok(slot.and_then(|i| self.nodes[i].entry.as_ref().map(|(_, value)| value)))
    }

    /// Returns the previous value. A duplicate keeps the original stored key and
    /// the incoming key is dropped; its cost was prepaid by the caller.
    pub(super) fn insert(
        &mut self,
        key: String,
        value: V,
        payer: &mut Payer<'_>,
    ) -> Result<Option<V>, Diagnostic> {
        let result = self.insert_inner(key, value, payer);
        self.settle(result)
    }

    /// Returns the previous value and releases the removed key.
    pub(super) fn remove(
        &mut self,
        key: &str,
        payer: &mut Payer<'_>,
    ) -> Result<Option<V>, Diagnostic> {
        let result = self.remove_inner(key, payer);
        self.settle(result)
    }

    fn settle<R>(&self, result: Result<R, Diagnostic>) -> Result<R, Diagnostic> {
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }

    // Authenticates the live payer first so a sticky first refusal is returned
    // even for an empty or previously refused map.
    fn check(&self, payer: &mut Payer<'_>) -> Result<(), Diagnostic> {
        inventory_text::current(payer)?;
        if self.poisoned.get() {
            return Err(
                payer.construction_refusal("inventory ordered map was used after a refusal")
            );
        }
        Ok(())
    }

    fn begin(&mut self, payer: &mut Payer<'_>) -> Result<(), Diagnostic> {
        self.check(payer)?;
        inventory_metadata::step(payer, 1)?;
        self.path.clear();
        Ok(())
    }

    fn key_at(&self, index: usize, payer: &mut Payer<'_>) -> Result<&str, Diagnostic> {
        match self.nodes[index].entry.as_ref() {
            Some((key, _)) => Ok(key.as_str()),
            None => Err(corrupt(payer)),
        }
    }

    fn child(&self, index: usize, right: bool) -> Option<usize> {
        if right {
            self.nodes[index].right
        } else {
            self.nodes[index].left
        }
    }

    fn set_child(&mut self, index: usize, right: bool, link: Option<usize>) {
        if right {
            self.nodes[index].right = link;
        } else {
            self.nodes[index].left = link;
        }
    }

    fn height(&self, link: Option<usize>) -> u32 {
        link.map_or(0, |index| self.nodes[index].height)
    }

    fn find(&self, key: &str, payer: &mut Payer<'_>) -> Result<Option<usize>, Diagnostic> {
        let mut cursor = self.root;
        while let Some(index) = cursor {
            inventory_metadata::step(payer, 1)?;
            match compare_keys(key, self.key_at(index, payer)?, payer)? {
                Ordering::Equal => return Ok(Some(index)),
                Ordering::Less => cursor = self.nodes[index].left,
                Ordering::Greater => cursor = self.nodes[index].right,
            }
        }
        Ok(None)
    }

    fn push_frame(
        &mut self,
        frame: (usize, bool),
        payer: &mut Payer<'_>,
    ) -> Result<(), Diagnostic> {
        inventory_metadata::step(payer, 1)?;
        reserve_one(payer, &mut self.path)?;
        self.path.push(frame);
        Ok(())
    }

    // Like `find`, but records the ancestors. Every comparator call finishes
    // before any map mutation; the frontier is scratch, not map state.
    fn locate(&mut self, key: &str, payer: &mut Payer<'_>) -> Result<Option<usize>, Diagnostic> {
        let mut cursor = self.root;
        while let Some(index) = cursor {
            inventory_metadata::step(payer, 1)?;
            let order = compare_keys(key, self.key_at(index, payer)?, payer)?;
            match order {
                Ordering::Equal => return Ok(Some(index)),
                Ordering::Less => {
                    self.push_frame((index, false), payer)?;
                    cursor = self.nodes[index].left;
                }
                Ordering::Greater => {
                    self.push_frame((index, true), payer)?;
                    cursor = self.nodes[index].right;
                }
            }
        }
        Ok(None)
    }

    fn insert_inner(
        &mut self,
        key: String,
        value: V,
        payer: &mut Payer<'_>,
    ) -> Result<Option<V>, Diagnostic> {
        self.begin(payer)?;
        if let Some(found) = self.locate(&key, payer)? {
            inventory_metadata::step(payer, 1)?;
            let Some((_, slot)) = self.nodes[found].entry.as_mut() else {
                return Err(corrupt(payer));
            };
            return Ok(Some(std::mem::replace(slot, value)));
        }
        let fresh = self.acquire_slot(key, value, payer)?;
        inventory_metadata::step(payer, 1)?;
        self.set_parent_link(Some(fresh));
        self.retrace(payer)?;
        Ok(None)
    }

    fn acquire_slot(
        &mut self,
        key: String,
        value: V,
        payer: &mut Payer<'_>,
    ) -> Result<usize, Diagnostic> {
        inventory_metadata::step(payer, 1)?;
        if let Some(index) = self.free {
            let next = self.nodes[index].left;
            self.free = next;
            self.nodes[index] = Node::new(key, value);
            return Ok(index);
        }
        reserve_one(payer, &mut self.nodes)?;
        // Prepay this new slot's finite release visit before publishing it.
        inventory_metadata::step(payer, 1)?;
        let index = self.nodes.len();
        self.nodes.push(Node::new(key, value));
        Ok(index)
    }

    fn set_parent_link(&mut self, link: Option<usize>) {
        match self.path.last().copied() {
            Some((parent, right)) => self.set_child(parent, right, link),
            None => self.root = link,
        }
    }

    fn successor(&mut self, found: usize, payer: &mut Payer<'_>) -> Result<usize, Diagnostic> {
        self.push_frame((found, true), payer)?;
        let Some(mut cursor) = self.nodes[found].right else {
            return Err(corrupt(payer));
        };
        loop {
            inventory_metadata::step(payer, 1)?;
            let Some(next) = self.nodes[cursor].left else {
                break;
            };
            self.push_frame((cursor, false), payer)?;
            cursor = next;
        }
        Ok(cursor)
    }

    fn remove_inner(&mut self, key: &str, payer: &mut Payer<'_>) -> Result<Option<V>, Diagnostic> {
        self.begin(payer)?;
        let Some(found) = self.locate(key, payer)? else {
            return Ok(None);
        };
        inventory_metadata::step(payer, 1)?;
        let two_children = self.nodes[found].left.is_some() && self.nodes[found].right.is_some();
        let target = if two_children {
            self.successor(found, payer)?
        } else {
            found
        };
        inventory_metadata::step(payer, UNLINK_UNITS)?;
        let removed = if target == found {
            self.nodes[found].entry.take()
        } else {
            // Move the successor payload without cloning; only the replaced
            // (removed) key is physically released below.
            let moved = self.nodes[target].entry.take();
            std::mem::replace(&mut self.nodes[found].entry, moved)
        };
        let Some((old_key, old_value)) = removed else {
            return Err(corrupt(payer));
        };
        let replacement = self.nodes[target].left.or(self.nodes[target].right);
        self.set_parent_link(replacement);
        self.nodes[target].left = self.free;
        self.nodes[target].right = None;
        self.nodes[target].height = 0;
        self.free = Some(target);
        drop(old_key);
        self.retrace(payer)?;
        Ok(Some(old_value))
    }

    fn refresh(&mut self, index: usize, payer: &mut Payer<'_>) -> Result<(), Diagnostic> {
        let tallest = self
            .height(self.nodes[index].left)
            .max(self.height(self.nodes[index].right));
        let Some(height) = tallest.checked_add(1) else {
            return Err(payer.construction_refusal("inventory ordered map height overflowed"));
        };
        self.nodes[index].height = height;
        Ok(())
    }

    // Promotes the child on the side opposite the rotation direction.
    fn rotate(
        &mut self,
        top: usize,
        right: bool,
        payer: &mut Payer<'_>,
    ) -> Result<usize, Diagnostic> {
        inventory_metadata::step(payer, ROTATION_UNITS)?;
        let side = !right;
        let Some(pivot) = self.child(top, side) else {
            return Err(corrupt(payer));
        };
        let inner = self.child(pivot, !side);
        self.set_child(top, side, inner);
        self.set_child(pivot, !side, Some(top));
        self.refresh(top, payer)?;
        self.refresh(pivot, payer)?;
        Ok(pivot)
    }

    fn rebalance(&mut self, index: usize, payer: &mut Payer<'_>) -> Result<usize, Diagnostic> {
        inventory_metadata::step(payer, VISIT_UNITS)?;
        let left = self.height(self.nodes[index].left);
        let right = self.height(self.nodes[index].right);
        if left.abs_diff(right) < 2 {
            self.refresh(index, payer)?;
            return Ok(index);
        }
        let heavy_right = right > left;
        let Some(heavy) = self.child(index, heavy_right) else {
            return Err(corrupt(payer));
        };
        let outer = self.height(self.child(heavy, heavy_right));
        let inner = self.height(self.child(heavy, !heavy_right));
        if inner > outer {
            // The extra parent-to-heavy link write of a double rotation.
            inventory_metadata::step(payer, 1)?;
            let rotated = self.rotate(heavy, heavy_right, payer)?;
            self.set_child(index, heavy_right, Some(rotated));
        }
        self.rotate(index, !heavy_right, payer)
    }

    fn retrace(&mut self, payer: &mut Payer<'_>) -> Result<(), Diagnostic> {
        loop {
            inventory_metadata::step(payer, 1)?;
            let Some((index, _)) = self.path.pop() else {
                break;
            };
            let top = self.rebalance(index, payer)?;
            self.set_parent_link(Some(top));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
