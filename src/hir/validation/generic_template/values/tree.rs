//! Immutable ordered AVL directory. Every constructed changed-path/rotation
//! node is independently paid; only a fully successful root is published.
use super::*;
use std::rc::Rc;
pub(super) type Root<'value> = Option<Rc<Node<'value>>>;
pub(super) struct Node<'value> {
    key: ValueId,
    ty: &'value ResolvedType,
    left: Root<'value>,
    right: Root<'value>,
    height: usize,
}
fn height(root: &Root<'_>) -> usize {
    root.as_ref().map_or(0, |node| node.height)
}
fn node<'value>(
    key: ValueId,
    ty: &'value ResolvedType,
    left: Root<'value>,
    right: Root<'value>,
) -> Result<Rc<Node<'value>>, Diagnostic> {
    charge::node()?;
    let height = 1 + height(&left).max(height(&right));
    Ok(Rc::new(Node {
        key,
        ty,
        left,
        right,
        height,
    }))
}
fn rotate_left<'value>(root: Rc<Node<'value>>) -> Result<Rc<Node<'value>>, Diagnostic> {
    let next = root.right.as_ref().expect("AVL right child");
    let left = node(
        root.key.clone(),
        root.ty,
        root.left.clone(),
        next.left.clone(),
    )?;
    node(next.key.clone(), next.ty, Some(left), next.right.clone())
}
fn rotate_right<'value>(root: Rc<Node<'value>>) -> Result<Rc<Node<'value>>, Diagnostic> {
    let next = root.left.as_ref().expect("AVL left child");
    let right = node(
        root.key.clone(),
        root.ty,
        next.right.clone(),
        root.right.clone(),
    )?;
    node(next.key.clone(), next.ty, next.left.clone(), Some(right))
}
fn balance<'value>(root: Rc<Node<'value>>) -> Result<Rc<Node<'value>>, Diagnostic> {
    let left = height(&root.left);
    let right = height(&root.right);
    if left > right + 1 {
        let child = root.left.as_ref().expect("AVL left imbalance");
        let root = if height(&child.left) < height(&child.right) {
            let left = rotate_left(child.clone())?;
            node(root.key.clone(), root.ty, Some(left), root.right.clone())?
        } else {
            root
        };
        rotate_right(root)
    } else if right > left + 1 {
        let child = root.right.as_ref().expect("AVL right imbalance");
        let root = if height(&child.right) < height(&child.left) {
            let right = rotate_right(child.clone())?;
            node(root.key.clone(), root.ty, root.left.clone(), Some(right))?
        } else {
            root
        };
        rotate_left(root)
    } else {
        Ok(root)
    }
}
pub(super) fn insert<'value>(
    root: &Root<'value>,
    key: ValueId,
    ty: &'value ResolvedType,
) -> Result<Rc<Node<'value>>, Diagnostic> {
    let Some(root) = root else {
        return node(key, ty, None, None);
    };
    let next = match key.cmp(&root.key) {
        std::cmp::Ordering::Less => {
            let left = insert(&root.left, key, ty)?;
            node(root.key.clone(), root.ty, Some(left), root.right.clone())?
        }
        std::cmp::Ordering::Greater => {
            let right = insert(&root.right, key, ty)?;
            node(root.key.clone(), root.ty, root.left.clone(), Some(right))?
        }
        // BTreeMap preserves its existing equal key on replacement.
        std::cmp::Ordering::Equal => {
            node(root.key.clone(), ty, root.left.clone(), root.right.clone())?
        }
    };
    balance(next)
}
pub(super) fn get<'value>(
    mut root: Option<&Node<'value>>,
    id: &ValueId,
) -> Option<&'value ResolvedType> {
    while let Some(node) = root {
        match id.cmp(&node.key) {
            std::cmp::Ordering::Less => root = node.left.as_deref(),
            std::cmp::Ordering::Greater => root = node.right.as_deref(),
            std::cmp::Ordering::Equal => return Some(node.ty),
        }
    }
    None
}
#[cfg(test)]
pub(super) fn ordered_entries(root: Option<&Node<'_>>, result: &mut Vec<(ValueId, ResolvedType)>) {
    if let Some(node) = root {
        ordered_entries(node.left.as_deref(), result);
        result.push((node.key.clone(), node.ty.clone()));
        ordered_entries(node.right.as_deref(), result);
    }
}

// A published checkpoint makes at least one node on its changed path shared.
// Only the exact wholly unique search path can be edited in place. Prepare its
// sole new leaf before any mutation; rotations then move unique path nodes and
// existing child handles without allocation or a later fallible payment.
fn unique_path(mut root: &Root<'_>, key: &ValueId) -> bool {
    while let Some(node) = root {
        if Rc::strong_count(node) != 1 || Rc::weak_count(node) != 0 {
            return false;
        }
        match key.cmp(&node.key) {
            std::cmp::Ordering::Less => root = &node.left,
            std::cmp::Ordering::Greater => root = &node.right,
            std::cmp::Ordering::Equal => return true,
        }
    }
    true
}
fn update(node: &mut Node<'_>) {
    node.height = 1 + height(&node.left).max(height(&node.right));
}
fn unique_left<'value>(mut root: Rc<Node<'value>>) -> Rc<Node<'value>> {
    let mut next = Rc::get_mut(&mut root)
        .expect("unique changed AVL path")
        .right
        .take()
        .expect("AVL right child");
    Rc::get_mut(&mut root).unwrap().right = Rc::get_mut(&mut next)
        .expect("unique changed AVL child")
        .left
        .take();
    update(Rc::get_mut(&mut root).unwrap());
    Rc::get_mut(&mut next).unwrap().left = Some(root);
    update(Rc::get_mut(&mut next).unwrap());
    next
}
fn unique_right<'value>(mut root: Rc<Node<'value>>) -> Rc<Node<'value>> {
    let mut next = Rc::get_mut(&mut root)
        .expect("unique changed AVL path")
        .left
        .take()
        .expect("AVL left child");
    Rc::get_mut(&mut root).unwrap().left = Rc::get_mut(&mut next)
        .expect("unique changed AVL child")
        .right
        .take();
    update(Rc::get_mut(&mut root).unwrap());
    Rc::get_mut(&mut next).unwrap().right = Some(root);
    update(Rc::get_mut(&mut next).unwrap());
    next
}
fn unique_balance<'value>(mut root: Rc<Node<'value>>) -> Rc<Node<'value>> {
    update(Rc::get_mut(&mut root).expect("unique changed AVL path"));
    let left = height(&root.left);
    let right = height(&root.right);
    if left > right + 1 {
        let child = root.left.as_ref().expect("AVL left imbalance");
        if height(&child.left) < height(&child.right) {
            let left = Rc::get_mut(&mut root).unwrap().left.take().unwrap();
            Rc::get_mut(&mut root).unwrap().left = Some(unique_left(left));
        }
        unique_right(root)
    } else if right > left + 1 {
        let child = root.right.as_ref().expect("AVL right imbalance");
        if height(&child.right) < height(&child.left) {
            let right = Rc::get_mut(&mut root).unwrap().right.take().unwrap();
            Rc::get_mut(&mut root).unwrap().right = Some(unique_right(right));
        }
        unique_left(root)
    } else {
        root
    }
}
fn unique_insert<'value>(
    root: &mut Root<'value>,
    key: &ValueId,
    ty: &'value ResolvedType,
    leaf: &mut Root<'value>,
) {
    let Some(mut current) = root.take() else {
        *root = leaf.take();
        return;
    };
    match key.cmp(&current.key) {
        std::cmp::Ordering::Less => {
            unique_insert(&mut Rc::get_mut(&mut current).unwrap().left, key, ty, leaf)
        }
        std::cmp::Ordering::Greater => {
            unique_insert(&mut Rc::get_mut(&mut current).unwrap().right, key, ty, leaf)
        }
        std::cmp::Ordering::Equal => Rc::get_mut(&mut current).unwrap().ty = ty,
    }
    *root = Some(unique_balance(current));
}
pub(super) fn insert_current<'value>(
    root: &mut Root<'value>,
    key: ValueId,
    ty: &'value ResolvedType,
) -> Result<(), Diagnostic> {
    if unique_path(root, &key) {
        let mut leaf = if get(root.as_deref(), &key).is_none() {
            Some(node(key.clone(), ty, None, None)?)
        } else {
            None
        };
        // No payment, allocation, quota reset or fallible call follows here.
        unique_insert(root, &key, ty, &mut leaf);
    } else {
        let next = insert(root, key, ty)?;
        *root = Some(next);
    }
    Ok(())
}
