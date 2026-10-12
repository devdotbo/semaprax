//! Deterministic private AVL ownership; no authored/attached shape authority.
use super::*;
use std::rc::Rc;
#[derive(Clone)]
pub(super) struct Node {
    pub(super) key: ValueId,
    pub(super) value: Rc<ValidationBinding>,
    pub(super) left: Option<Rc<Node>>,
    pub(super) right: Option<Rc<Node>>,
    pub(super) height: u8,
    pub(super) count: usize,
}
pub(super) fn height(root: &Option<Rc<Node>>) -> u8 {
    root.as_ref().map_or(0, |n| n.height)
}
fn count(root: &Option<Rc<Node>>) -> usize {
    root.as_ref().map_or(0, |n| n.count)
}
fn update(node: &mut Node) {
    node.height = 1 + height(&node.left).max(height(&node.right));
    node.count = 1 + count(&node.left) + count(&node.right);
}
#[cfg(test)]
fn rotate_left(mut root: Rc<Node>) -> Rc<Node> {
    let mut next = Rc::make_mut(&mut root)
        .right
        .take()
        .expect("AVL right child");
    Rc::make_mut(&mut root).right = Rc::make_mut(&mut next).left.take();
    update(Rc::make_mut(&mut root));
    Rc::make_mut(&mut next).left = Some(root);
    update(Rc::make_mut(&mut next));
    next
}
#[cfg(test)]
fn rotate_right(mut root: Rc<Node>) -> Rc<Node> {
    let mut next = Rc::make_mut(&mut root).left.take().expect("AVL left child");
    Rc::make_mut(&mut root).left = Rc::make_mut(&mut next).right.take();
    update(Rc::make_mut(&mut root));
    Rc::make_mut(&mut next).right = Some(root);
    update(Rc::make_mut(&mut next));
    next
}
#[cfg(test)]
fn balance(mut root: Rc<Node>) -> Rc<Node> {
    update(Rc::make_mut(&mut root));
    let delta = i16::from(height(&root.left)) - i16::from(height(&root.right));
    if delta > 1 {
        let left = root.left.as_ref().expect("AVL left imbalance");
        if height(&left.left) < height(&left.right) {
            let left = Rc::make_mut(&mut root).left.take().unwrap();
            Rc::make_mut(&mut root).left = Some(rotate_left(left));
        }
        rotate_right(root)
    } else if delta < -1 {
        let right = root.right.as_ref().expect("AVL right imbalance");
        if height(&right.right) < height(&right.left) {
            let right = Rc::make_mut(&mut root).right.take().unwrap();
            Rc::make_mut(&mut root).right = Some(rotate_right(right));
        }
        rotate_left(root)
    } else {
        root
    }
}
#[cfg(test)]
pub(super) fn insert(
    root: Option<Rc<Node>>,
    key: ValueId,
    value: Rc<ValidationBinding>,
) -> Rc<Node> {
    let Some(mut root) = root else {
        return Rc::new(Node {
            key,
            value,
            left: None,
            right: None,
            height: 1,
            count: 1,
        });
    };
    match key.cmp(&root.key) {
        std::cmp::Ordering::Less => {
            let child = Rc::make_mut(&mut root).left.take();
            Rc::make_mut(&mut root).left = Some(insert(child, key, value));
        }
        std::cmp::Ordering::Greater => {
            let child = Rc::make_mut(&mut root).right.take();
            Rc::make_mut(&mut root).right = Some(insert(child, key, value));
        }
        std::cmp::Ordering::Equal => Rc::make_mut(&mut root).value = value,
    }
    balance(root)
}
pub(super) fn build(
    values: &mut std::collections::btree_map::IntoIter<ValueId, ValidationBinding>,
    count: usize,
) -> Option<Rc<Node>> {
    if count == 0 {
        return None;
    }
    let left = build(values, count / 2);
    let (key, value) = values.next().expect("exact owned scope count");
    let right = build(values, count - count / 2 - 1);
    let mut node = Node {
        key,
        value: Rc::new(value),
        left,
        right,
        height: 0,
        count: 0,
    };
    update(&mut node);
    Some(Rc::new(node))
}
pub(super) fn get<'a>(mut node: Option<&'a Node>, key: &ValueId) -> Option<&'a ValidationBinding> {
    while let Some(current) = node {
        match key.cmp(&current.key) {
            std::cmp::Ordering::Equal => return Some(&current.value),
            std::cmp::Ordering::Less => node = current.left.as_deref(),
            std::cmp::Ordering::Greater => node = current.right.as_deref(),
        }
    }
    None
}
pub(super) fn get_mut<'a>(
    root: &'a mut Option<Rc<Node>>,
    key: &ValueId,
) -> Option<&'a mut ValidationBinding> {
    let current = Rc::make_mut(root.as_mut()?);
    match key.cmp(&current.key) {
        std::cmp::Ordering::Equal => Some(Rc::make_mut(&mut current.value)),
        std::cmp::Ordering::Less => get_mut(&mut current.left, key),
        std::cmp::Ordering::Greater => get_mut(&mut current.right, key),
    }
}
// After the private publication guard's children/continuations have dropped,
// its one complete version owns each node/binding uniquely. No cloning or
// reservation is performed here. Output tree constructors were prepaid.
pub(super) fn materialize(
    root: Option<Rc<Node>>,
    target: &mut BTreeMap<ValueId, ValidationBinding>,
) {
    let Some(root) = root else {
        return;
    };
    let node = match Rc::try_unwrap(root) {
        Ok(node) => node,
        Err(_) => panic!("checkpoint escaped publication owner"),
    };
    materialize(node.left, target);
    let value = match Rc::try_unwrap(node.value) {
        Ok(value) => value,
        Err(_) => panic!("binding escaped publication owner"),
    };
    target.insert(node.key, value);
    materialize(node.right, target);
}

pub(super) fn value<'a>(
    mut node: Option<&'a Node>,
    key: &ValueId,
) -> Option<&'a Rc<ValidationBinding>> {
    while let Some(current) = node {
        match key.cmp(&current.key) {
            std::cmp::Ordering::Equal => return Some(&current.value),
            std::cmp::Ordering::Less => node = current.left.as_deref(),
            std::cmp::Ordering::Greater => node = current.right.as_deref(),
        }
    }
    None
}

pub(super) fn build_fresh<'a>(
    values: &mut impl Iterator<Item = (&'a ValueId, &'a ValidationBinding)>,
    count: usize,
) -> Option<Rc<Node>> {
    if count == 0 {
        return None;
    }
    let left = build_fresh(values, count / 2);
    let (key, value) = values.next().expect("exact borrowed baseline count");
    let mut value = value.clone();
    // Every clause begins a new freshness domain, including hostile internal
    // callers that retain true flags in an otherwise unchanged outer map.
    value.contract_local = false;
    let right = build_fresh(values, count - count / 2 - 1);
    let mut node = Node {
        key: key.clone(),
        value: Rc::new(value),
        left,
        right,
        height: 0,
        count: 0,
    };
    update(&mut node);
    Some(Rc::new(node))
}
