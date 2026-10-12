//! Paid borrowed emissions for the unchanged String/byte-set family predicates.
use super::{Admission, Directory, Entry, Family, Refusal, Scratch};
use crate::hir::{
    ResolvedExpr, ResolvedExprKind as K, ResolvedFunction, ResolvedStatement as S, ValueId,
};
use std::cmp::Ordering;

pub(super) fn compare(
    left: &ValueId,
    right: &ValueId,
    admission: &mut Admission<'_>,
) -> Result<Ordering, Refusal> {
    let bytes =
        admission.arithmetic(left.as_str().len().min(right.as_str().len()).checked_add(2))?;
    admission.units(bytes)?;
    // Full ValueId equality includes its retained fingerprint.
    Ok(left.cmp(right))
}
pub(super) fn mutable(
    ids: &[&ValueId],
    id: &ValueId,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    let (mut begin, mut end) = (0, ids.len());
    while begin < end {
        let middle = begin + (end - begin) / 2;
        match compare(ids[middle], id, admission)? {
            Ordering::Less => begin = middle + 1,
            Ordering::Greater => end = middle,
            Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}
fn children<'a>(
    expression: &'a ResolvedExpr,
    pending: &mut Scratch<&'a ResolvedExpr>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let start = pending.as_slice().len();
    if let K::Closure { captures, .. } = &expression.kind {
        for capture in captures {
            admission.units(1)?;
            let child = &capture.value;
            pending.push(child, admission)?;
        }
    } else {
        let mut children = crate::hir::lexical_exit::ChildCursor::new(expression);
        loop {
            admission.units(1)?;
            let Some(child) = children.next() else {
                break;
            };
            pending.push(child, admission)?;
        }
    }
    pending.reverse_suffix(start, admission)
}
fn string_reopen(
    value: &ResolvedExpr,
    owner: &ValueId,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    let id = crate::string_ops::StringOp::Concat.id();
    admission.units(callee.as_str().len().min(id.len()) + 1)?;
    // This receiver is called only after admitted(String,String). Every Map
    // reopen returns a collection, so the old generic branch cannot succeed.
    if callee.as_str() != id || !type_arguments.is_empty() || args.len() != 2 {
        return Ok(false);
    }
    let K::Place(place) = &args[0].kind else {
        return Ok(false);
    };
    if compare(&place.root, owner, admission)? != Ordering::Equal || !place.projections.is_empty() {
        return Ok(false);
    }
    let needle = super::debug_contains::Needle::new(owner, admission)?;
    Ok(!needle.contains(&args[1], admission)?)
}
fn byte_reopen(
    value: &ResolvedExpr,
    owner: &ValueId,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    use crate::byte_ops::ByteOp;
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    let mut selected = None;
    for op in [
        ByteOp::Set,
        ByteOp::Set5,
        ByteOp::Set1Or5,
        ByteOp::Set1Or6Or48,
    ] {
        admission.units(callee.as_str().len().min(op.id().len()) + 1)?;
        if callee.as_str() == op.id() {
            selected = Some(op);
            break;
        }
    }
    let Some(op) = selected else {
        return Ok(false);
    };
    if !type_arguments.is_empty() || args.len() != op.arity() {
        return Ok(false);
    }
    let K::Place(place) = &args[0].kind else {
        return Ok(false);
    };
    Ok(compare(&place.root, owner, admission)? == Ordering::Equal && place.projections.is_empty())
}
pub(super) fn collect<'a>(
    function: &'a ResolvedFunction,
    family: Family,
    output: &mut Scratch<Entry<'a>>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let mut ids = Scratch::new(admission)?;
    let mut updates = Scratch::new(admission)?;
    let mut pending = Scratch::new(admission)?;
    pending.push(&function.body, admission)?;
    while let Some(expression) = pending.pop(admission)? {
        if matches!(expression.kind, K::Closure { .. }) {
            continue;
        }
        if let K::Block { statements, .. } | K::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                admission.units(1)?;
                match statement {
                    S::Let {
                        binding,
                        mutable: true,
                        ..
                    } => ids.push(&binding.id, admission)?,
                    S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } => {
                        let accepted = match family {
                            Family::String => {
                                crate::string_ops::replacement::admitted(binding, value)
                                    && !string_reopen(value, &binding.id, admission)?
                            }
                            Family::ByteSet => byte_reopen(value, &binding.id, admission)?,
                            _ => return admission.refuse(Refusal::Subject),
                        };
                        if accepted {
                            updates.push((&value.id, binding), admission)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        children(expression, &mut pending, admission)?;
    }
    let mut ids = ids.into_vec();
    super::sort::heap(&mut ids, admission, |left, right, admission| {
        compare(left, right, admission)
    })?;
    for &(at, binding) in updates.as_slice() {
        admission.units(1)?;
        if mutable(&ids, &binding.id, admission)? {
            Directory::emit(output, family, at, binding, admission)?;
        }
    }
    Ok(())
}
