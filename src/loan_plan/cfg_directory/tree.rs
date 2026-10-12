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
pub(super) fn get<'a, K: Ord, V>(mut root: Option<&'a Node<K, V>>, key: &K) -> Option<&'a V> {
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
pub(in crate::loan_plan) struct Keys<'a, K, V> {
    root: Option<&'a Node<K, V>>,
    next: Option<&'a Node<K, V>>,
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
fn successor<'a, K: Ord, V>(mut root: Option<&'a Node<K, V>>, key: &K) -> Option<&'a Node<K, V>> {
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
pub(super) fn remove<K: Ord, V>(root: &mut Option<Box<Node<K, V>>>, key: &K) -> Option<V> {
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
