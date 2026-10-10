//! Independently recovered finite fresh-row capacity; never partition authority.
use crate::ast::BinaryOp;
use crate::hir::{
    OwnershipMode as O, ResolvedBinding, ResolvedExpr as E, ResolvedExprKind as K, ResolvedProgram,
    ResolvedStatement as S, ResolvedType as T,
};
mod consumers;
#[cfg(test)]
mod tests;

struct Proof<'a> {
    condition: &'a E,
    body: &'a E,
    iterations: u32,
}
fn typed(e: &E, ty: &T, mode: O) -> bool {
    e.ty == *ty && e.ownership == mode
}
fn place(e: &E, binding: &ResolvedBinding) -> bool {
    typed(e, &binding.ty, binding.ownership)
        && matches!(&e.kind, K::Place(p) if p.root == binding.id && p.projections.is_empty())
}
fn same_binding(left: &ResolvedBinding, right: &ResolvedBinding) -> bool {
    left.id == right.id
        && left.name == right.name
        && left.ty == right.ty
        && left.ownership == right.ownership
}
fn integer(e: &E, value: i64) -> bool {
    typed(e, &T::I64, O::Value) && matches!(e.kind,K::Int(v) if v==value)
}
fn usize_value(e: &E) -> Option<u64> {
    if !typed(e, &T::Usize, O::Value) {
        return None;
    }
    if let K::Usize(v) = e.kind {
        Some(v)
    } else {
        None
    }
}
fn call<'a>(e: &'a E, id: &str, types: &[T]) -> Option<&'a [E]> {
    match &e.kind {
        K::Call {
            callee,
            instance: None,
            type_arguments,
            args,
        } if callee.as_str() == id && type_arguments == types => Some(args),
        _ => None,
    }
}
fn inspect(program: &ResolvedProgram) -> Option<Proof<'_>> {
    if program.functions.len() != 1
        || !program.function_instances.is_empty()
        || !program.function_templates.is_empty()
        || !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || !program.agents.is_empty()
    {
        return None;
    }
    let f = &program.functions[0];
    if !f.params.is_empty()
        || f.return_type != T::I64
        || !f.effects.is_empty()
        || f.yields.is_some()
        || !f.requires.is_empty()
        || !f.ensures.is_empty()
    {
        return None;
    }
    let K::Block { statements, tail } = &f.body.kind else {
        return None;
    };
    let [S::Let {
        binding: vector,
        mutable: true,
        value: allocation,
        ..
    }, S::Let {
        binding: counter,
        mutable: true,
        value: start,
        ..
    }, S::While {
        condition, body, ..
    }, suffix @ ..] = statements.as_slice()
    else {
        return None;
    };
    if vector.id == counter.id
        || counter.ty != T::I64
        || counter.ownership != O::Value
        || !integer(start, 0)
        || vector.ownership != O::Own
        || !typed(allocation, &vector.ty, O::Own)
    {
        return None;
    }
    let T::Nominal {
        declaration,
        arguments,
    } = &vector.ty
    else {
        return None;
    };
    let [element] = arguments.as_slice() else {
        return None;
    };
    if declaration.as_str() != crate::prelude::VEC_ID {
        return None;
    }
    let T::Nominal {
        declaration: record,
        arguments,
    } = element
    else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let declaration = program.types.iter().find(|d| d.id == *record)?;
    if !declaration.type_parameters.is_empty() {
        return None;
    }
    let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    if fields.len() != 3
        || fields.iter().filter(|v| v.ty == T::I64).count() != 1
        || fields.iter().filter(|v| v.ty == T::String).count() != 1
        || fields.iter().filter(|v| v.ty == T::Bytes).count() != 1
    {
        return None;
    }
    for (index, field) in fields.iter().enumerate() {
        if field.index != u32::try_from(index).ok()?
            || fields[..index]
                .iter()
                .any(|v| v.id == field.id || v.name == field.name)
        {
            return None;
        }
    }
    let [capacity] = call(
        allocation,
        crate::vec_ops::WITH_CAPACITY_ID,
        std::slice::from_ref(element),
    )?
    else {
        return None;
    };
    let count = usize_value(capacity)?;
    if count == 0 || count > u64::from(crate::byte_data_capacity::MAX_BYTES_COPY_SITES) {
        return None;
    }
    let K::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return None;
    };
    if !typed(condition, &T::Bool, O::Value)
        || !place(left, counter)
        || !integer(right, i64::try_from(count).ok()?)
    {
        return None;
    }
    let K::Block {
        statements: construction,
        tail: zero,
    } = &body.kind
    else {
        return None;
    };
    let [S::Let {
        binding: item,
        mutable: false,
        value: constructor,
        ..
    }, S::Assign {
        binding: destination,
        field: None,
        value: push,
        ..
    }, S::Assign {
        binding: latch,
        field: None,
        value: step,
        ..
    }] = construction.as_slice()
    else {
        return None;
    };
    if item.id == vector.id
        || item.id == counter.id
        || item.ty != *element
        || item.ownership != O::Own
        || !same_binding(destination, vector)
        || !same_binding(latch, counter)
        || !integer(zero, 0)
        || !typed(body, &T::I64, O::Value)
        || !typed(constructor, element, O::Own)
        || !typed(push, &vector.ty, O::Own)
    {
        return None;
    }
    let K::ConstructRecord {
        record: actual,
        fields: values,
    } = &constructor.kind
    else {
        return None;
    };
    if actual != record || values.len() != fields.len() {
        return None;
    }
    let mut width = None;
    for (field, value) in fields.iter().zip(values) {
        if field.id != value.field {
            return None;
        }
        match field.ty {
            T::I64 if place(&value.value, counter) => {}
            T::String
                if typed(&value.value, &T::String, O::Own)
                    && matches!(value.value.kind, K::String(_)) => {}
            T::Bytes if typed(&value.value, &T::Bytes, O::Own) => {
                let [extent] = call(&value.value, crate::byte_ops::ZEROED_ID, &[])? else {
                    return None;
                };
                width = Some(usize_value(extent)?);
            }
            _ => return None,
        }
    }
    let width = width?;
    if width == 0
        || width.checked_mul(count)? > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
        || width.checked_mul(count)? > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
    {
        return None;
    }
    let [old, fresh] = call(push, crate::vec_ops::PUSH_ID, std::slice::from_ref(element))? else {
        return None;
    };
    if !place(old, vector) || !place(fresh, item) {
        return None;
    }
    let K::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &step.kind
    else {
        return None;
    };
    if !typed(step, &T::I64, O::Value) || !place(left, counter) || !integer(right, 1) {
        return None;
    }
    consumers::validate(program, suffix, tail)?;
    Some(Proof {
        condition,
        body,
        iterations: u32::try_from(count).ok()?,
    })
}
pub(crate) fn resolved(program: &ResolvedProgram, condition: &E, body: &E) -> Option<u32> {
    let p = inspect(program)?;
    (std::ptr::eq(condition, p.condition) && std::ptr::eq(body, p.body)).then_some(p.iterations)
}
