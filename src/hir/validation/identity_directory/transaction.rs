//! Paid staging of a shared suffix; the unique prefix commits infallibly.
use super::*;
fn update<K>(node: &mut Node<K>) {
    node.height = 1 + height(&node.left).max(height(&node.right));
    node.count = 1 + count(&node.left) + count(&node.right);
}
fn node<K: Key>(key: K, left: Root<K>, right: Root<K>) -> Result<Rc<Node<K>>, Diagnostic> {
    let count = count(&left)
        .checked_add(count(&right))
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| {
            scope_checkpoint::charge::failure(usize::MAX, "identity node count overflowed")
        })?;
    let height = height(&left)
        .max(height(&right))
        .checked_add(1)
        .ok_or_else(|| {
            scope_checkpoint::charge::failure(usize::MAX, "identity node height overflowed")
        })?;
    // Exact owning Rc carrier plus the literal Node initializer. The key clone
    // shares its existing immutable backing and creates no payload allocation.
    #[repr(C)]
    struct Carrier<T> {
        strong: usize,
        weak: usize,
        value: T,
    }
    scope_checkpoint::charge::reserve(
        std::alloc::Layout::new::<Carrier<Node<K>>>()
            .size()
            .checked_add(std::mem::size_of::<Node<K>>()),
    )?;
    Ok(Rc::new(Node {
        key,
        left,
        right,
        height,
        count,
    }))
}
fn left<K>(mut root: Rc<Node<K>>) -> Rc<Node<K>> {
    let mut next = Rc::get_mut(&mut root)
        .expect("unique identity prefix")
        .right
        .take()
        .expect("AVL right child");
    Rc::get_mut(&mut root).unwrap().right = Rc::get_mut(&mut next)
        .expect("unique identity pivot")
        .left
        .take();
    update(Rc::get_mut(&mut root).unwrap());
    Rc::get_mut(&mut next).unwrap().left = Some(root);
    update(Rc::get_mut(&mut next).unwrap());
    next
}
fn right<K>(mut root: Rc<Node<K>>) -> Rc<Node<K>> {
    let mut next = Rc::get_mut(&mut root)
        .expect("unique identity prefix")
        .left
        .take()
        .expect("AVL left child");
    Rc::get_mut(&mut root).unwrap().left = Rc::get_mut(&mut next)
        .expect("unique identity pivot")
        .right
        .take();
    update(Rc::get_mut(&mut root).unwrap());
    Rc::get_mut(&mut next).unwrap().right = Some(root);
    update(Rc::get_mut(&mut next).unwrap());
    next
}
fn balance<K>(mut root: Rc<Node<K>>) -> Rc<Node<K>> {
    update(Rc::get_mut(&mut root).expect("unique identity path"));
    let lh = height(&root.left);
    let rh = height(&root.right);
    if lh > rh + 1 {
        let child = root.left.as_ref().expect("AVL left imbalance");
        if height(&child.left) < height(&child.right) {
            let child = Rc::get_mut(&mut root).unwrap().left.take().unwrap();
            Rc::get_mut(&mut root).unwrap().left = Some(left(child));
        }
        right(root)
    } else if rh > lh + 1 {
        let child = root.right.as_ref().expect("AVL right imbalance");
        if height(&child.right) < height(&child.left) {
            let child = Rc::get_mut(&mut root).unwrap().right.take().unwrap();
            Rc::get_mut(&mut root).unwrap().right = Some(right(child));
        }
        left(root)
    } else {
        root
    }
}
fn rebuild<K: Key>(root: &Root<K>, key: &K) -> Result<Rc<Node<K>>, Diagnostic> {
    let Some(current) = root else {
        return node(key.clone(), None, None);
    };
    let next = match key.cmp(&current.key) {
        std::cmp::Ordering::Less => {
            let changed = rebuild(&current.left, key)?;
            node(current.key.clone(), Some(changed), current.right.clone())?
        }
        std::cmp::Ordering::Greater => {
            let changed = rebuild(&current.right, key)?;
            node(current.key.clone(), current.left.clone(), Some(changed))?
        }
        std::cmp::Ordering::Equal => unreachable!("absent identity was preflighted"),
    };
    // Height-changing heavy children/pivots belong to the fresh changed path.
    Ok(balance(next))
}
fn stage<K: Key>(root: &Root<K>, key: &K) -> Result<(usize, Root<K>), Diagnostic> {
    let mut depth = 0;
    let mut current = root;
    loop {
        let Some(node) = current else {
            return Ok((depth, Some(rebuild(current, key)?)));
        };
        if Rc::strong_count(node) != 1 || Rc::weak_count(node) != 0 {
            return Ok((depth, Some(rebuild(current, key)?)));
        }
        match key.cmp(&node.key) {
            std::cmp::Ordering::Equal => unreachable!("absent identity was preflighted"),
            std::cmp::Ordering::Less => current = &node.left,
            std::cmp::Ordering::Greater => current = &node.right,
        }
        depth += 1;
    }
}
fn commit<K>(root: &mut Root<K>, key: &K, depth: usize, change: Root<K>)
where
    K: Ord,
{
    if depth == 0 {
        *root = change;
        return;
    }
    let mut current = root.take().expect("unique prefix member");
    match key.cmp(&current.key) {
        std::cmp::Ordering::Less => commit(
            &mut Rc::get_mut(&mut current).expect("unique prefix").left,
            key,
            depth - 1,
            change,
        ),
        std::cmp::Ordering::Greater => commit(
            &mut Rc::get_mut(&mut current).expect("unique prefix").right,
            key,
            depth - 1,
            change,
        ),
        std::cmp::Ordering::Equal => unreachable!("staged prefix stops at the equal key"),
    }
    *root = Some(balance(current));
}
pub(super) fn insert<K: Key>(root: &mut Root<K>, key: &K) -> Result<(), Diagnostic> {
    scope_checkpoint::charge::reserve(Some(0))?;
    if count(root).checked_add(1).is_none() {
        return Err(scope_checkpoint::charge::failure(
            usize::MAX,
            "identity node count overflowed",
        ));
    }
    let (depth, change) = stage(root, key)?;
    // Every payment and constructor completed before the first mutation.
    // The root was borrowed throughout staging, never cloned for rollback.
    commit(root, key, depth, change);
    Ok(())
}
