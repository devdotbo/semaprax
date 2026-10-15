//! Reset every current freshness flag while sharing unchanged private storage.
use super::*;
type Root = Option<Rc<tree::Node>>;
fn reset(root: &Root) -> Result<(Root, bool), Diagnostic> {
    let Some(current) = root else {
        return Ok((None, false));
    };
    // Visit both children even if this binding is already clear. No cached
    // membership, resolver success or assumed flag state authorizes sharing.
    let (left, left_changed) = reset(&current.left)?;
    let (right, right_changed) = reset(&current.right)?;
    let changed = current.value.contract_local;
    if !changed && !left_changed && !right_changed {
        return Ok((Some(current.clone()), false));
    }
    let value = if changed {
        charge::reserve(
            charge::binding(&current.value)
                .and_then(|bytes| bytes.checked_add(charge::carrier::<ValidationBinding>())),
        )?;
        let mut value = current.value.as_ref().clone();
        value.contract_local = false;
        Rc::new(value)
    } else {
        current.value.clone()
    };
    charge::new_node()?;
    // Resetting a flag preserves keys, cardinality, shape and all other fields.
    // Only a complete staged root is returned; the source never mutates.
    Ok((
        Some(Rc::new(tree::Node {
            key: current.key.clone(),
            value,
            left,
            right,
            height: current.height,
            count: current.count,
        })),
        true,
    ))
}
pub(super) fn baseline(source: &Scope) -> Result<Scope, Diagnostic> {
    charge::reserve(Some(0))?;
    let (root, _) = reset(&source.root)?;
    Ok(Scope { root })
}
