//! One invocation's explicitly paid insertion receivers and terminal report.
use super::super::physical_storage::btree_node_upper;
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::{collections::BTreeSet, marker::PhantomData, mem::size_of};

pub(super) struct Insertions<T> {
    pub(super) failed: bool,
    refusal: Option<Diagnostic>,
    entry: bool,
    function: *const ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    owned: usize,
    held: usize,
    marker: PhantomData<fn() -> T>,
}
impl<T: Ord> Insertions<T> {
    // Aggregate initializer, named mutable result and caller receiving owner;
    // the Result<Self> wrapper remains separate, plus constructor/payment Results,
    // inert ledger return, refusal None/Some/take and terminal Diagnostic moves.
    pub(super) fn controls() -> usize {
        3 * size_of::<Self>()
            + size_of::<Result<Self, Diagnostic>>()
            + size_of::<Result<(), Diagnostic>>()
            + size_of::<ClassifierLedger>()
            + 3 * size_of::<Option<Diagnostic>>()
            + 2 * size_of::<Diagnostic>()
            + size_of::<&mut Self>()
            + size_of::<&mut Option<Diagnostic>>()
            + 5 * size_of::<Option<usize>>()
            + ClassifierLedger::capture_into_controls()
            + ClassifierLedger::insertion_terminal_controls()
            // At most one positive-payment failure reaches conversion. Pay its
            // actual/formal compact values and Diagnostic construction/return
            // once, before any entry; sticky failure forbids reuse.
            + 2 * size_of::<ByteFailure>()
            + 2 * size_of::<Diagnostic>()
    }
    // A single entry's actual method/gateway/positive-reservation receivers.
    // Each entry consumes this bank and pays the next bank afresh. The final
    // unused entry remains reserved: no reset, refund or allowance donation.
    pub(super) fn entry_controls() -> usize {
        2 * size_of::<&mut Self>()
            + 4 * size_of::<&ResolvedFunction>()
            + 4 * size_of::<&mut ReplayBudget>()
            + size_of::<&mut Option<Diagnostic>>()
            + size_of::<Result<(), ByteFailure>>()
            + ClassifierLedger::current_receiving_controls()
    }
    pub(super) fn acquire(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                Self::controls() + Self::entry_controls(),
                "ownership insertion receiving controls",
            )?;
        }
        let node = btree_node_upper::<Box<T>, ()>().unwrap_or(usize::MAX);
        let owned = node
            .checked_add(size_of::<T>())
            .and_then(|n| n.checked_add(2 * size_of::<Box<T>>()))
            .and_then(|n| n.checked_add(Self::entry_controls()))
            .unwrap_or(usize::MAX);
        let held = node.saturating_add(Self::entry_controls());
        let mut result = Self {
            failed: false,
            refusal: None,
            entry: true,
            function: std::ptr::from_ref(function),
            budget: std::ptr::from_ref(budget),
            ledger: ClassifierLedger::empty_receiving(),
            owned,
            held,
            marker: PhantomData,
        };
        result.ledger.capture_current_into();
        Ok(result)
    }
    fn authenticate(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget) {
        if self.failed {
            return;
        }
        assert!(self.entry);
        self.entry = false;
        // Compare exact receiver identity before querying a possibly foreign
        // payer. The held ledger poisons only its original owner on mismatch.
        if self.function != std::ptr::from_ref(function)
            || self.budget != std::ptr::from_ref(budget)
            || !self.ledger.current()
        {
            self.ledger.reject_storage();
            self.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
            self.failed = true;
            return;
        }
        budget.authenticate_zero_into(
            function,
            "incoming ownership state authority",
            &mut self.refusal,
        );
        self.failed = self.refusal.is_some();
    }
    pub(super) fn insert_owned(
        &mut self,
        function: &ResolvedFunction,
        target: &mut BTreeSet<Box<T>>,
        value: T,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        if self.failed {
            return;
        }
        if target.contains(&value) {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                Self::entry_controls(),
                "ownership duplicate receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
        } else {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                self.owned,
                "incoming ownership state node, box and receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
            target.insert(Box::new(value));
        }
        self.entry = true;
    }
    pub(super) fn insert_held(
        &mut self,
        function: &ResolvedFunction,
        target: &mut BTreeSet<Box<T>>,
        value: Box<T>,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        if self.failed {
            return;
        }
        if target.contains(value.as_ref()) {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                Self::entry_controls(),
                "ownership held duplicate receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
        } else {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                self.held,
                "incoming held ownership state node and receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
            target.insert(value);
        }
        self.entry = true;
    }
    // One terminal publication: callers return immediately. The owner remains
    // failed after the report moves, so neither backing nor a bank can revive.
    pub(super) fn failure(&mut self) -> Diagnostic {
        assert!(self.failed);
        self.refusal
            .take()
            .expect("ownership insertion report moves once")
    }
}

#[cfg(test)]
#[path = "boxed_states/before.rs"]
mod before;
#[cfg(test)]
use before::{insert_held, insert_owned};
#[cfg(test)]
#[path = "boxed_states/receiving_tests.rs"]
mod receiving_tests;
#[cfg(test)]
mod tests;

pub(in crate::cleanup_plan::replay::path_join) mod avl {
    pub(in crate::cleanup_plan::replay::path_join) use self::states::Set;
    // One invocation's explicitly paid insertion receivers and terminal report.
    use self::states::Set as StateSet;
    use super::{Diagnostic, ReplayBudget, ResolvedFunction};
    use crate::bounded_output::classifier_string::ClassifierLedger;
    use crate::cleanup_plan::replay::physical_budget::ByteFailure;
    use std::{marker::PhantomData, mem::size_of};

    pub(in crate::cleanup_plan::replay::path_join) struct Insertions<T> {
        pub(in crate::cleanup_plan::replay::path_join) failed: bool,
        refusal: Option<Diagnostic>,
        entry: bool,
        function: *const ResolvedFunction,
        budget: *const ReplayBudget,
        ledger: ClassifierLedger,
        owned: usize,
        held: usize,
        marker: PhantomData<fn() -> T>,
    }
    impl<T: Ord> Insertions<T> {
        // Aggregate initializer, named mutable result and caller receiving owner;
        // the Result<Self> wrapper remains separate, plus constructor/payment Results,
        // inert ledger return, refusal None/Some/take and terminal Diagnostic moves.
        pub(in crate::cleanup_plan::replay::path_join) fn controls() -> usize {
            3 * size_of::<Self>()
            + size_of::<Result<Self, Diagnostic>>()
            + size_of::<Result<(), Diagnostic>>()
            + size_of::<ClassifierLedger>()
            + 3 * size_of::<Option<Diagnostic>>()
            + 2 * size_of::<Diagnostic>()
            + size_of::<&mut Self>()
            + size_of::<&mut Option<Diagnostic>>()
            + 5 * size_of::<Option<usize>>()
            + ClassifierLedger::capture_into_controls()
            + ClassifierLedger::insertion_terminal_controls()
            // At most one positive-payment failure reaches conversion. Pay its
            // actual/formal compact values and Diagnostic construction/return
            // once, before any entry; sticky failure forbids reuse.
            + 2 * size_of::<ByteFailure>()
            + 2 * size_of::<Diagnostic>()
        }
        // A single entry's actual method/gateway/positive-reservation receivers.
        // Each entry consumes this bank and pays the next bank afresh. The final
        // unused entry remains reserved: no reset, refund or allowance donation.
        pub(in crate::cleanup_plan::replay::path_join) fn entry_controls() -> usize {
            2 * size_of::<&mut Self>()
                + 4 * size_of::<&ResolvedFunction>()
                + 4 * size_of::<&mut ReplayBudget>()
                + size_of::<&mut Option<Diagnostic>>()
                + size_of::<Result<(), ByteFailure>>()
                + ClassifierLedger::current_receiving_controls()
                + StateSet::<T>::receiving_controls()
        }
        pub(in crate::cleanup_plan::replay::path_join) fn acquire(
            function: &ResolvedFunction,
            budget: &mut ReplayBudget,
        ) -> Result<Self, Diagnostic> {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    Self::controls() + Self::entry_controls(),
                    "ownership insertion receiving controls",
                )?;
            }
            let node = StateSet::<T>::node_allocation_bytes();
            let owned = node
                .checked_add(size_of::<T>())
                .and_then(|n| n.checked_add(2 * size_of::<Box<T>>()))
                .and_then(|n| n.checked_add(Self::entry_controls()))
                .unwrap_or(usize::MAX);
            let held = node.saturating_add(Self::entry_controls());
            let mut result = Self {
                failed: false,
                refusal: None,
                entry: true,
                function: std::ptr::from_ref(function),
                budget: std::ptr::from_ref(budget),
                ledger: ClassifierLedger::empty_receiving(),
                owned,
                held,
                marker: PhantomData,
            };
            result.ledger.capture_current_into();
            Ok(result)
        }
        fn authenticate(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget) {
            if self.failed {
                return;
            }
            assert!(self.entry);
            self.entry = false;
            // Compare exact receiver identity before querying a possibly foreign
            // payer. The held ledger poisons only its original owner on mismatch.
            if self.function != std::ptr::from_ref(function)
                || self.budget != std::ptr::from_ref(budget)
                || !self.ledger.current()
            {
                self.ledger.reject_storage();
                self.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
                self.failed = true;
                return;
            }
            budget.authenticate_zero_into(
                function,
                "incoming ownership state authority",
                &mut self.refusal,
            );
            self.failed = self.refusal.is_some();
        }
        pub(in crate::cleanup_plan::replay::path_join) fn insert_owned(
            &mut self,
            function: &ResolvedFunction,
            target: &mut StateSet<T>,
            value: T,
            budget: &mut ReplayBudget,
        ) {
            self.authenticate(function, budget);
            if self.failed {
                return;
            }
            if target.contains(&value) {
                if let Err(error) = budget.reserve_compact_bytes(
                    function,
                    Self::entry_controls(),
                    "ownership duplicate receiving renewal",
                ) {
                    self.refusal = Some(error.into_diagnostic());
                    self.failed = true;
                    return;
                }
            } else {
                if let Err(error) = budget.reserve_compact_bytes(
                    function,
                    self.owned,
                    "incoming AVL ownership state node, box and receiving renewal",
                ) {
                    self.refusal = Some(error.into_diagnostic());
                    self.failed = true;
                    return;
                }
                target.insert(Box::new(value));
            }
            self.entry = true;
        }
        pub(in crate::cleanup_plan::replay::path_join) fn insert_held(
            &mut self,
            function: &ResolvedFunction,
            target: &mut StateSet<T>,
            value: Box<T>,
            budget: &mut ReplayBudget,
        ) {
            self.authenticate(function, budget);
            if self.failed {
                return;
            }
            if target.contains(value.as_ref()) {
                if let Err(error) = budget.reserve_compact_bytes(
                    function,
                    Self::entry_controls(),
                    "ownership held duplicate receiving renewal",
                ) {
                    self.refusal = Some(error.into_diagnostic());
                    self.failed = true;
                    return;
                }
            } else {
                if let Err(error) = budget.reserve_compact_bytes(
                    function,
                    self.held,
                    "incoming AVL held ownership state node and receiving renewal",
                ) {
                    self.refusal = Some(error.into_diagnostic());
                    self.failed = true;
                    return;
                }
                target.insert(value);
            }
            self.entry = true;
        }
        // One terminal publication: callers return immediately. The owner remains
        // failed after the report moves, so neither backing nor a bank can revive.
        pub(in crate::cleanup_plan::replay::path_join) fn failure(&mut self) -> Diagnostic {
            assert!(self.failed);
            self.refusal
                .take()
                .expect("ownership insertion report moves once")
        }
    }

    mod states {
        //! Ordered unique complete boxed ownership states; no shortened state/key type.
        use std::mem::size_of;
        #[allow(
            dead_code,
            reason = "Retain the complete original generic AVL implementation."
        )]
        mod tree {
            //! Unique AVL nodes preserve key order without sorting canonical vectors.
            //! Preparation pays the only new node before mutation; rotations and drain
            //! move unique boxes without allocations, fallible calls or budget resets.
            use std::cmp::Ordering;
            pub(super) struct Node<K, V> {
                key: K,
                value: V,
                left: Option<Box<Self>>,
                right: Option<Box<Self>>,
                height: usize,
            }
            impl<K, V> Node<K, V> {
                pub(super) fn leaf(key: K, value: V) -> Self {
                    Self {
                        key,
                        value,
                        left: None,
                        right: None,
                        height: 1,
                    }
                }
            }
            pub(super) fn height<K, V>(root: &Option<Box<Node<K, V>>>) -> usize {
                root.as_ref().map_or(0, |node| node.height)
            }
            fn update<K, V>(node: &mut Node<K, V>) {
                node.height = 1 + height(&node.left).max(height(&node.right));
            }
            fn left<K, V>(mut root: Box<Node<K, V>>) -> Box<Node<K, V>> {
                let mut next = root.right.take().expect("AVL right child");
                root.right = next.left.take();
                update(&mut root);
                next.left = Some(root);
                update(&mut next);
                next
            }
            fn right<K, V>(mut root: Box<Node<K, V>>) -> Box<Node<K, V>> {
                let mut next = root.left.take().expect("AVL left child");
                root.left = next.right.take();
                update(&mut root);
                next.right = Some(root);
                update(&mut next);
                next
            }
            fn balance<K, V>(mut root: Box<Node<K, V>>) -> Box<Node<K, V>> {
                update(&mut root);
                let a = height(&root.left);
                let b = height(&root.right);
                if a > b + 1 {
                    let child = root.left.as_ref().expect("AVL left imbalance");
                    if height(&child.left) < height(&child.right) {
                        root.left = Some(left(root.left.take().unwrap()));
                    }
                    right(root)
                } else if b > a + 1 {
                    let child = root.right.as_ref().expect("AVL right imbalance");
                    if height(&child.right) < height(&child.left) {
                        root.right = Some(right(root.right.take().unwrap()));
                    }
                    left(root)
                } else {
                    root
                }
            }
            pub(super) fn get<'a, K: Ord, V>(
                mut root: Option<&'a Node<K, V>>,
                key: &K,
            ) -> Option<&'a V> {
                while let Some(node) = root {
                    match key.cmp(&node.key) {
                        Ordering::Less => root = node.left.as_deref(),
                        Ordering::Greater => root = node.right.as_deref(),
                        Ordering::Equal => return Some(&node.value),
                    }
                }
                None
            }
            pub(super) fn replace<K: Ord, V>(
                root: &mut Option<Box<Node<K, V>>>,
                key: &K,
                value: V,
            ) -> Option<V> {
                let node = root.as_mut()?;
                match key.cmp(&node.key) {
                    Ordering::Less => replace(&mut node.left, key, value),
                    Ordering::Greater => replace(&mut node.right, key, value),
                    Ordering::Equal => Some(std::mem::replace(&mut node.value, value)),
                }
            }
            pub(super) fn insert<K: Ord, V>(
                root: &mut Option<Box<Node<K, V>>>,
                leaf: &mut Option<Box<Node<K, V>>>,
            ) {
                let Some(mut node) = root.take() else {
                    *root = leaf.take();
                    return;
                };
                match leaf
                    .as_ref()
                    .expect("prepared absent key")
                    .key
                    .cmp(&node.key)
                {
                    Ordering::Less => insert(&mut node.left, leaf),
                    Ordering::Greater => insert(&mut node.right, leaf),
                    Ordering::Equal => unreachable!("prepared key was absent"),
                }
                *root = Some(balance(node));
            }
            // One in-order visit per node, with recursion bounded by the AVL height.
            // Output capacity was paid before the caller detached its root. No sorting,
            // rebalancing, allocation, payment failure or refund occurs while draining.
            pub(super) fn drain<K>(root: Option<Box<Node<K, ()>>>, values: &mut Vec<K>) {
                if let Some(mut node) = root {
                    let left = node.left.take();
                    let right = node.right.take();
                    drain(left, values);
                    values.push(node.key);
                    drain(right, values);
                }
            }

            // Two borrowed references, no heap or unbounded traversal stack. Each
            // successor lookup is O(log n); consuming drain above remains O(n).
            pub(super) struct Keys<'a, K, V> {
                root: Option<&'a Node<K, V>>,
                next: Option<&'a Node<K, V>>,
            }
            impl<K, V> Clone for Keys<'_, K, V> {
                fn clone(&self) -> Self {
                    Self {
                        root: self.root,
                        next: self.next,
                    }
                }
            }
            fn minimum<K, V>(mut root: Option<&Node<K, V>>) -> Option<&Node<K, V>> {
                while let Some(node) = root {
                    if node.left.is_none() {
                        return Some(node);
                    }
                    root = node.left.as_deref();
                }
                None
            }
            fn successor<'a, K: Ord, V>(
                mut root: Option<&'a Node<K, V>>,
                key: &K,
            ) -> Option<&'a Node<K, V>> {
                let mut next = None;
                while let Some(node) = root {
                    if key < &node.key {
                        next = Some(node);
                        root = node.left.as_deref();
                    } else {
                        root = node.right.as_deref();
                    }
                }
                next
            }
            pub(super) fn keys<K, V>(root: Option<&Node<K, V>>) -> Keys<'_, K, V> {
                Keys {
                    root,
                    next: minimum(root),
                }
            }
            impl<'a, K: Ord, V> Iterator for Keys<'a, K, V> {
                type Item = &'a K;
                fn next(&mut self) -> Option<Self::Item> {
                    let node = self.next?;
                    self.next = successor(self.root, &node.key);
                    Some(&node.key)
                }
            }
            fn take_min<K, V>(root: &mut Option<Box<Node<K, V>>>) -> Box<Node<K, V>> {
                let mut node = root.take().expect("nonempty minimum subtree");
                if node.left.is_none() {
                    *root = node.right.take();
                    return node;
                }
                let first = take_min(&mut node.left);
                *root = Some(balance(node));
                first
            }
            // No allocation or fallible work follows the caller's sticky reserve(0).
            // Only owned nodes are moved; failed lookup leaves values and key identity intact.
            pub(super) fn remove<K: Ord, V>(
                root: &mut Option<Box<Node<K, V>>>,
                key: &K,
            ) -> Option<V> {
                let mut node = root.take()?;
                let removed = match key.cmp(&node.key) {
                    Ordering::Less => remove(&mut node.left, key),
                    Ordering::Greater => remove(&mut node.right, key),
                    Ordering::Equal => {
                        if node.left.is_none() {
                            *root = node.right.take();
                        } else if node.right.is_none() {
                            *root = node.left.take();
                        } else {
                            let mut next = take_min(&mut node.right);
                            next.left = node.left.take();
                            next.right = node.right.take();
                            *root = Some(balance(next));
                        }
                        return Some(node.value);
                    }
                };
                *root = Some(balance(node));
                removed
            }

            // The new ownership set stores complete boxed values; only lookup borrows T.
            pub(super) fn contains_boxed<T: Ord>(
                mut root: Option<&Node<Box<T>, ()>>,
                key: &T,
            ) -> bool {
                while let Some(node) = root {
                    match key.cmp(node.key.as_ref()) {
                        Ordering::Less => root = node.left.as_deref(),
                        Ordering::Greater => root = node.right.as_deref(),
                        Ordering::Equal => return true,
                    }
                }
                false
            }
            pub(super) fn pop_first_key<K, V>(root: &mut Option<Box<Node<K, V>>>) -> Option<K> {
                root.as_ref()?;
                Some(take_min(root).key)
            }
        }

        pub(in crate::cleanup_plan::replay::path_join) struct Set<T> {
            root: Option<Box<tree::Node<Box<T>, ()>>>,
            len: usize,
        }
        impl<T> Default for Set<T> {
            fn default() -> Self {
                Self { root: None, len: 0 }
            }
        }
        impl<T: Ord> Set<T> {
            pub(in crate::cleanup_plan::replay::path_join) fn new() -> Self {
                Self::default()
            }
            pub(in crate::cleanup_plan::replay::path_join) fn len(&self) -> usize {
                self.len
            }
            pub(in crate::cleanup_plan::replay::path_join) fn is_empty(&self) -> bool {
                self.len == 0
            }
            pub(in crate::cleanup_plan::replay::path_join) fn contains(&self, value: &T) -> bool {
                tree::contains_boxed(self.root.as_deref(), value)
            }
            pub(in crate::cleanup_plan::replay::path_join) fn node_allocation_bytes() -> usize {
                // The entire actual node initializer and its entire heap allocation.
                2 * size_of::<tree::Node<Box<T>, ()>>()
            }
            pub(in crate::cleanup_plan::replay::path_join) fn receiving_controls() -> usize {
                2 * size_of::<&Self>()
                    + 2 * size_of::<&mut Self>()
                    + 2 * size_of::<Option<Box<tree::Node<Box<T>, ()>>>>()
                    + 4 * size_of::<usize>()
                    + 2 * size_of::<bool>()
            }
            // The affine insertion owner has paid before this method is entered.
            // No allocation, fallible operation or payer call follows the prepared leaf.
            pub(in crate::cleanup_plan::replay::path_join) fn insert(
                &mut self,
                value: Box<T>,
            ) -> bool {
                if self.contains(value.as_ref()) {
                    return false;
                }
                let mut leaf = Some(Box::new(tree::Node::leaf(value, ())));
                tree::insert(&mut self.root, &mut leaf);
                debug_assert!(leaf.is_none());
                self.len += 1;
                true
            }
            pub(in crate::cleanup_plan::replay::path_join) fn iter(&self) -> Iter<'_, T> {
                Iter {
                    inner: tree::keys(self.root.as_deref()),
                    remaining: self.len,
                }
            }
        }
        pub(in crate::cleanup_plan::replay::path_join) struct Iter<'a, T> {
            inner: tree::Keys<'a, Box<T>, ()>,
            remaining: usize,
        }
        impl<T> Clone for Iter<'_, T> {
            fn clone(&self) -> Self {
                Self {
                    inner: self.inner.clone(),
                    remaining: self.remaining,
                }
            }
        }
        impl<'a, T: Ord> Iterator for Iter<'a, T> {
            type Item = &'a Box<T>;
            fn next(&mut self) -> Option<Self::Item> {
                let value = self.inner.next()?;
                self.remaining -= 1;
                Some(value)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.remaining, Some(self.remaining))
            }
        }
        impl<T: Ord> ExactSizeIterator for Iter<'_, T> {}
        pub(in crate::cleanup_plan::replay::path_join) struct IntoIter<T> {
            root: Option<Box<tree::Node<Box<T>, ()>>>,
            remaining: usize,
        }
        impl<T: Ord> Iterator for IntoIter<T> {
            type Item = Box<T>;
            fn next(&mut self) -> Option<Self::Item> {
                let value = tree::pop_first_key(&mut self.root)?;
                self.remaining -= 1;
                Some(value)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.remaining, Some(self.remaining))
            }
        }
        impl<T: Ord> ExactSizeIterator for IntoIter<T> {}
        impl<T: Ord> IntoIterator for Set<T> {
            type Item = Box<T>;
            type IntoIter = IntoIter<T>;
            fn into_iter(mut self) -> Self::IntoIter {
                let remaining = self.len;
                self.len = 0;
                IntoIter {
                    root: self.root.take(),
                    remaining,
                }
            }
        }
        impl<T> Drop for Set<T> {
            fn drop(&mut self) {
                // Preserve ascending-key destruction without allocation or a payer call.
                while let Some(value) = tree::pop_first_key(&mut self.root) {
                    drop(value);
                }
            }
        }
        impl<T> Drop for IntoIter<T> {
            fn drop(&mut self) {
                while let Some(value) = tree::pop_first_key(&mut self.root) {
                    drop(value);
                }
            }
        }
    }
}
