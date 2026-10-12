//! Real carrier/clone backing, charged before checkpoint construction.
use super::*;
use crate::cleanup_plan::ReplayClonePayload;
use std::alloc::Layout;
#[repr(C)]
#[allow(
    dead_code,
    reason = "layout mirror of the two reference counts and the retained value"
)]
struct Carrier<T> {
    strong: usize,
    weak: usize,
    value: T,
}
pub(super) fn carrier<T>() -> usize {
    Layout::new::<Carrier<T>>().size()
}
pub(in crate::hir::validation) fn failure(requested: usize, message: &'static str) -> Diagnostic {
    crate::bounded_output::refuse_active_required(requested);
    hir_error_static(message)
}
pub(super) fn reserve(bytes: Option<usize>) -> Result<(), Diagnostic> {
    let Some(bytes) = bytes else {
        return Err(failure(
            usize::MAX,
            "ownership scope checkpoint layout overflowed",
        ));
    };
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(hir_error_static(
            "ownership scope checkpoint exceeds the active validation budget",
        ))
    }
}

fn tree<K, V>(len: usize) -> Option<usize> {
    crate::cleanup_plan::replay_btree_node_upper::<K, V>()?.checked_mul(len)
}
fn projections(values: &[PlaceProjection]) -> Option<usize> {
    values
        .len()
        .checked_mul(std::mem::size_of::<PlaceProjection>())
}
pub(super) fn binding(value: &ValidationBinding) -> Option<usize> {
    let mut bytes = value.ty.clone_payload_bytes()?;
    bytes = bytes.checked_add(tree::<Vec<PlaceProjection>, Availability>(
        value.moved_places.len(),
    )?)?;
    for path in value.moved_places.keys() {
        bytes = bytes.checked_add(projections(path)?)?;
    }
    bytes = bytes.checked_add(tree::<Vec<PlaceProjection>, ()>(
        value.definitely_partial.len(),
    )?)?;
    for path in &value.definitely_partial {
        bytes = bytes.checked_add(projections(path)?)?;
    }
    bytes.checked_add(tree::<LoanId, ()>(value.active_loans.len())?)
}
pub(super) fn publication_slots(count: usize) -> Result<(), Diagnostic> {
    reserve(tree::<ValueId, ValidationBinding>(count))
}
pub(super) fn checkpoint_carriers(count: usize) -> Result<(), Diagnostic> {
    let carriers = carrier::<tree::Node>().checked_add(carrier::<ValidationBinding>());
    reserve(carriers.and_then(|bytes| bytes.checked_mul(count)))
}
pub(super) fn initial(count: usize) -> Result<(), Diagnostic> {
    checkpoint_carriers(count)?;
    publication_slots(count)
}
pub(super) fn edit(
    root: &Option<std::rc::Rc<tree::Node>>,
    key: &ValueId,
) -> Result<(), Diagnostic> {
    let mut current = root.as_deref();
    let mut shared = false;
    let mut nodes = 0usize;
    let mut bytes = 0usize;
    let mut owner = root.as_ref();
    while let Some(node) = current {
        shared |= owner.is_some_and(|owner| std::rc::Rc::strong_count(owner) > 1);
        if shared {
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| failure(usize::MAX, "scope path layout overflowed"))?;
        }
        match key.cmp(&node.key) {
            std::cmp::Ordering::Equal => {
                if shared || std::rc::Rc::strong_count(&node.value) > 1 {
                    let payload = binding(&node.value).ok_or_else(|| {
                        failure(usize::MAX, "scope binding clone layout overflowed")
                    })?;
                    bytes = carrier::<ValidationBinding>()
                        .checked_add(payload)
                        .ok_or_else(|| {
                            failure(usize::MAX, "scope binding carrier layout overflowed")
                        })?;
                }
                break;
            }
            std::cmp::Ordering::Less => {
                owner = node.left.as_ref();
                current = node.left.as_deref();
            }
            std::cmp::Ordering::Greater => {
                owner = node.right.as_ref();
                current = node.right.as_deref();
            }
        }
    }
    reserve(
        nodes
            .checked_mul(carrier::<tree::Node>())
            .and_then(|nodes| nodes.checked_add(bytes)),
    )
}

pub(in crate::hir::validation) fn clone_type(value: &ResolvedType) -> Result<(), Diagnostic> {
    reserve(value.clone_payload_bytes())
}

pub(super) fn fresh_baseline(values: &dyn ScopeRead) -> Result<(), Diagnostic> {
    checkpoint_carriers(values.len())?;
    for value in values.values() {
        reserve(binding(value))?;
    }
    Ok(())
}
