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
pub(super) fn pop_first<K, V>(root: &mut Option<Box<Node<K, V>>>) -> Option<(K, V)> {
    let mut node = root.take()?;
    if node.left.is_none() {
        *root = node.right.take();
        Some((node.key, node.value))
    } else {
        let result = pop_first(&mut node.left);
        *root = Some(balance(node));
        result
    }
}
