//! Paid staging of a shared suffix; the unique prefix commits infallibly.
use super::*;
type Root = Option<Rc<tree::Node>>;
fn count(root: &Root) -> usize {
    root.as_ref().map_or(0, |node| node.count)
}
fn update(node: &mut tree::Node) {
    node.height = 1 + tree::height(&node.left).max(tree::height(&node.right));
    node.count = 1 + count(&node.left) + count(&node.right);
}
fn node(
    key: ValueId,
    value: Rc<ValidationBinding>,
    left: Root,
    right: Root,
) -> Result<Rc<tree::Node>, Diagnostic> {
    let count = count(&left)
        .checked_add(count(&right))
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| charge::failure(usize::MAX, "scope node count overflowed"))?;
    let height = tree::height(&left)
        .max(tree::height(&right))
        .checked_add(1)
        .ok_or_else(|| charge::failure(usize::MAX, "scope node height overflowed"))?;
    charge::new_node()?;
    Ok(Rc::new(tree::Node {
        key,
        value,
        left,
        right,
        height,
        count,
    }))
}
fn left(mut root: Rc<tree::Node>) -> Rc<tree::Node> {
    let mut next = Rc::get_mut(&mut root)
        .expect("unique scope prefix")
        .right
        .take()
        .expect("AVL right child");
    Rc::get_mut(&mut root).unwrap().right = Rc::get_mut(&mut next)
        .expect("unique scope pivot")
        .left
        .take();
    update(Rc::get_mut(&mut root).unwrap());
    Rc::get_mut(&mut next).unwrap().left = Some(root);
    update(Rc::get_mut(&mut next).unwrap());
    next
}
fn right(mut root: Rc<tree::Node>) -> Rc<tree::Node> {
    let mut next = Rc::get_mut(&mut root)
        .expect("unique scope prefix")
        .left
        .take()
        .expect("AVL left child");
    Rc::get_mut(&mut root).unwrap().left = Rc::get_mut(&mut next)
        .expect("unique scope pivot")
        .right
        .take();
    update(Rc::get_mut(&mut root).unwrap());
    Rc::get_mut(&mut next).unwrap().right = Some(root);
    update(Rc::get_mut(&mut next).unwrap());
    next
}
fn balance(mut root: Rc<tree::Node>) -> Rc<tree::Node> {
    update(Rc::get_mut(&mut root).expect("unique scope path"));
    let lh = tree::height(&root.left);
    let rh = tree::height(&root.right);
    if lh > rh + 1 {
        let child = root.left.as_ref().expect("AVL left imbalance");
        if tree::height(&child.left) < tree::height(&child.right) {
            let child = Rc::get_mut(&mut root).unwrap().left.take().unwrap();
            Rc::get_mut(&mut root).unwrap().left = Some(left(child));
        }
        right(root)
    } else if rh > lh + 1 {
        let child = root.right.as_ref().expect("AVL right imbalance");
        if tree::height(&child.right) < tree::height(&child.left) {
            let child = Rc::get_mut(&mut root).unwrap().right.take().unwrap();
            Rc::get_mut(&mut root).unwrap().right = Some(right(child));
        }
        left(root)
    } else {
        root
    }
}
fn rebuild(
    root: &Root,
    key: &ValueId,
    value: Rc<ValidationBinding>,
) -> Result<Rc<tree::Node>, Diagnostic> {
    let Some(current) = root else {
        return node(key.clone(), value, None, None);
    };
    let next = match key.cmp(&current.key) {
        std::cmp::Ordering::Less => {
            let changed = rebuild(&current.left, key, value)?;
            node(
                current.key.clone(),
                current.value.clone(),
                Some(changed),
                current.right.clone(),
            )?
        }
        std::cmp::Ordering::Greater => {
            let changed = rebuild(&current.right, key, value)?;
            node(
                current.key.clone(),
                current.value.clone(),
                current.left.clone(),
                Some(changed),
            )?
        }
        std::cmp::Ordering::Equal => node(
            current.key.clone(),
            value,
            current.left.clone(),
            current.right.clone(),
        )?,
    };
    // Height-changing heavy children/pivots belong to the fresh changed path.
    // Equal-key replacement keeps height, so it never rotates old siblings.
    Ok(balance(next))
}
enum Change {
    Root(Root),
    Replace(Rc<ValidationBinding>),
}
fn stage(
    root: &Root,
    key: &ValueId,
    value: Rc<ValidationBinding>,
) -> Result<(usize, Change), Diagnostic> {
    let mut depth = 0;
    let mut current = root;
    loop {
        let Some(node) = current else {
            return Ok((depth, Change::Root(Some(rebuild(current, key, value)?))));
        };
        if Rc::strong_count(node) != 1 || Rc::weak_count(node) != 0 {
            return Ok((depth, Change::Root(Some(rebuild(current, key, value)?))));
        }
        match key.cmp(&node.key) {
            std::cmp::Ordering::Equal => return Ok((depth, Change::Replace(value))),
            std::cmp::Ordering::Less => current = &node.left,
            std::cmp::Ordering::Greater => current = &node.right,
        }
        depth += 1;
    }
}
fn commit(root: &mut Root, key: &ValueId, depth: usize, change: Change) {
    if depth == 0 {
        match change {
            Change::Root(next) => *root = next,
            Change::Replace(value) => {
                Rc::get_mut(root.as_mut().expect("existing replacement"))
                    .expect("unique replacement")
                    .value = value
            }
        }
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
pub(super) fn insert_current(
    root: &mut Root,
    key: ValueId,
    value: Rc<ValidationBinding>,
) -> Result<(), Diagnostic> {
    // Includes allocation-free existing-Rc replacement and sticky refusal.
    charge::reserve(Some(0))?;
    if tree::get(root.as_deref(), &key).is_none() && count(root).checked_add(1).is_none() {
        return Err(charge::failure(usize::MAX, "scope node count overflowed"));
    }
    let (depth, change) = stage(root, &key, value)?;
    // Every payment and allocation has succeeded. No clone allocation,
    // reservation or fallible operation follows the first prefix mutation.
    commit(root, &key, depth, change);
    Ok(())
}
