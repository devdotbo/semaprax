//! Closed typed construction prefix: declarations, count, induction, fresh row,
//! producer, publication and latch. Every fact is read from current HIR nodes.
use super::*;
use crate::hir::{DeclarationKind, FunctionExecutionId, IdentityOrigin, ResolvedTypeDeclarationKind};

const fn shape(reason: &'static str) -> Refusal {
    Refusal::Shape(reason)
}
const fn forged(reason: &'static str) -> Refusal {
    Refusal::Forged(reason)
}

fn same_binding(left: &ResolvedBinding, right: &ResolvedBinding) -> bool {
    left.id == right.id
        && left.name == right.name
        && left.ty == right.ty
        && left.ownership == right.ownership
}

/// An authenticated source Vec constructor is an ordinary allocating block, not
/// a fixed-capacity fresh destination; a disagreeing expansion is forged.
pub(super) fn vec_constructor(e: &E) -> Option<Refusal> {
    crate::hir::vec_constructor::expansion(e)?;
    Some(match crate::hir::vec_constructor::authenticate(e) {
        Ok(()) => shape("an authenticated Vec constructor allocates outside the fused row"),
        Err(_) => forged("Vec constructor expansion disagrees"),
    })
}

fn closed_program(program: &ResolvedProgram) -> Result<&ResolvedFunction, Refusal> {
    let [function] = program.functions.as_slice() else {
        return Err(shape("not exactly one function"));
    };
    if !program.function_instances.is_empty()
        || !program.function_templates.is_empty()
        || !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || !program.agents.is_empty()
    {
        return Err(shape("module carries generic, permit, interface or agent declarations"));
    }
    if !function.params.is_empty()
        || function.return_type != T::I64
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return Err(shape("function is not closed, pure and scalar-result"));
    }
    if program.entrypoint != function.id {
        return Err(forged("entrypoint is not the only function"));
    }
    let fact = program
        .declarations
        .declaration(&function.id)
        .ok_or(forged("function declaration fact is missing"))?;
    if fact.kind != DeclarationKind::Function
        || fact.identity_origin != IdentityOrigin::Explicit
        || fact.name != function.name
        || fact.owner.is_some()
    {
        return Err(forged("function declaration fact disagrees"));
    }
    Ok(function)
}

/// Exactly one explicit nongeneric record with one i64, one String and one
/// Bytes field, agreeing with the declaration index field by field.
fn record<'a>(
    program: &'a ResolvedProgram,
    element: &T,
) -> Result<(&'a ResolvedTypeDeclaration, &'a [ResolvedFieldDeclaration], u64), Refusal> {
    let T::Nominal {
        declaration: id,
        arguments,
    } = element
    else {
        return Err(shape("Vec element is not a nominal record"));
    };
    if !arguments.is_empty() {
        return Err(shape("Vec element is generic"));
    }
    let mut found = program.types.iter().filter(|d| d.id == *id);
    let declaration = found.next().ok_or(forged("record declaration is missing"))?;
    if found.next().is_some() {
        return Err(forged("duplicate record declaration"));
    }
    if !declaration.type_parameters.is_empty() {
        return Err(shape("record is generic"));
    }
    let fact = program
        .declarations
        .declaration(id)
        .ok_or(forged("record declaration fact is missing"))?;
    if fact.kind != DeclarationKind::Record
        || fact.identity_origin != IdentityOrigin::Explicit
        || fact.name != declaration.name
        || fact.owner.is_some()
    {
        return Err(forged("record declaration fact disagrees"));
    }
    let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
        return Err(forged("record fact names a non-record declaration"));
    };
    if program.declarations.record_fields(id) != Some(fields.as_slice()) {
        return Err(forged("record field index disagrees"));
    }
    if fields.len() != 3
        || fields.iter().filter(|f| f.ty == T::I64).count() != 1
        || fields.iter().filter(|f| f.ty == T::String).count() != 1
        || fields.iter().filter(|f| f.ty == T::Bytes).count() != 1
    {
        return Err(shape("record is not one i64, one String and one Bytes field"));
    }
    for (index, field) in fields.iter().enumerate() {
        let fact = program
            .declarations
            .declaration(&field.id)
            .ok_or(forged("field declaration fact is missing"))?;
        if u32::try_from(index).ok() != Some(field.index)
            || fields[..index]
                .iter()
                .any(|other| other.id == field.id || other.name == field.name)
            || fact.kind != DeclarationKind::Field
            || fact.identity_origin != IdentityOrigin::Explicit
            || fact.owner.as_ref() != Some(id)
            || fact.name != field.name
        {
            return Err(forged("field identity, order or owner disagrees"));
        }
    }
    let layout = crate::hir::owned_leaf_collection::layout(&program.declarations, element)
        .ok_or(forged("owned record layout is missing"))?;
    if (layout.scalar_count, layout.owned_count) != (1, 2) {
        return Err(forged("owned record layout disagrees"));
    }
    Ok((declaration, fields.as_slice(), layout.capacity()))
}

pub(super) fn derive(program: &ResolvedProgram) -> Result<Selection<'_>, Refusal> {
    let function = closed_program(program)?;
    let K::Block { statements, tail } = &function.body.kind else {
        return Err(shape("function body is not a block"));
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
        return Err(shape("body does not begin with destination, induction and while"));
    };
    if let Some(refusal) = vec_constructor(allocation) {
        return Err(refusal);
    }
    // Destination freshness: one owned Vec<Record> binding initialised by the
    // direct fixed-capacity builtin, distinct from the induction binding.
    let T::Nominal {
        declaration: vec_id,
        arguments,
    } = &vector.ty
    else {
        return Err(shape("destination is not a Vec"));
    };
    let [element] = arguments.as_slice() else {
        return Err(shape("destination is not a Vec"));
    };
    if vec_id.as_str() != crate::prelude::VEC_ID {
        return Err(shape("destination is not a Vec"));
    }
    if vector.ownership != O::Own || !typed(allocation, &vector.ty, O::Own) {
        return Err(forged("destination ownership disagrees"));
    }
    if vector.id == counter.id {
        return Err(forged("destination and induction share one value"));
    }
    let (record, fields, envelope) = record(program, element)?;
    let [capacity_literal] = builtin(
        allocation,
        crate::vec_ops::WITH_CAPACITY_ID,
        std::slice::from_ref(element),
    )
    .ok_or(shape("destination is not a fixed-capacity allocation"))?
    else {
        return Err(forged("capacity call arity disagrees"));
    };
    let capacity =
        usize_literal(capacity_literal).ok_or(shape("capacity is not a usize literal"))?;
    // Induction: a distinct mutable i64 initialised to literal zero.
    if counter.ty != T::I64 || counter.ownership != O::Value {
        return Err(shape("induction is not an i64 value"));
    }
    if integer(start) != Some(0) {
        return Err(shape("induction does not start at zero"));
    }
    // Exact polarity: `induction < count`, induction on the left.
    let (bound_left, bound_right) =
        binary(condition, BinaryOp::Lt).ok_or(shape("condition is not induction < count"))?;
    if !typed(condition, &T::Bool, O::Value) {
        return Err(forged("condition type disagrees"));
    }
    if !place(bound_left, counter) {
        return Err(shape("condition does not read the induction"));
    }
    let bound = integer(bound_right).ok_or(shape("condition bound is not an i64 literal"))?;
    let rows = u64::try_from(bound).map_err(|_| shape("condition bound is negative"))?;
    if rows != capacity {
        return Err(shape("condition bound differs from the capacity"));
    }
    if rows == 0 {
        return Err(shape("zero rows"));
    }
    if rows < u64::from(MIN_ROWS) {
        return Err(Refusal::Legacy);
    }
    if rows > u64::from(MAX_ROWS) || rows > envelope {
        return Err(Refusal::Rows);
    }
    let rows = u32::try_from(rows).map_err(|_| Refusal::Rows)?;
    // Repeated block: fresh immutable row, same-Vec push, unit latch, tail 0.
    if let Some(refusal) = vec_constructor(body) {
        return Err(refusal);
    }
    let K::Block {
        statements: construction,
        tail: zero,
    } = &body.kind
    else {
        return Err(shape("loop body is not a block"));
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
        return Err(shape("loop body is not row, push and latch"));
    };
    if !typed(body, &T::I64, O::Value) || integer(zero) != Some(0) {
        return Err(shape("loop body result is not zero"));
    }
    if item.id == vector.id || item.id == counter.id {
        return Err(forged("row binding reuses a loop value"));
    }
    if item.ty != *element || item.ownership != O::Own || !typed(constructor, element, O::Own) {
        return Err(forged("row binding type or ownership disagrees"));
    }
    if destination.id != vector.id || latch.id != counter.id {
        return Err(shape("push or latch targets another binding"));
    }
    if !same_binding(destination, vector) || !same_binding(latch, counter) {
        return Err(forged("assignment target disagrees with its binding"));
    }
    let K::ConstructRecord {
        record: constructed,
        fields: values,
    } = &constructor.kind
    else {
        return Err(shape("row is not a direct record construction"));
    };
    if *constructed != record.id || values.len() != fields.len() {
        return Err(forged("row constructs another record"));
    }
    // Evaluation order is the HIR initializer order. It must be declaration
    // order with strictly increasing source positions, so no reordering hides.
    let mut order = [Role::Induction; 3];
    let mut text = None;
    let mut producer = None;
    let mut width = None;
    let mut previous_end = constructor.span.start;
    for (index, (field, value)) in fields.iter().zip(values).enumerate() {
        if field.id != value.field
            || value.value.span.start < previous_end
            || value.value.span.end > constructor.span.end
        {
            return Err(shape("row operands are not in declaration and source order"));
        }
        previous_end = value.value.span.end;
        order[index] = match field.ty {
            T::I64 if place(&value.value, counter) => Role::Induction,
            T::String
                if typed(&value.value, &T::String, O::Own)
                    && matches!(value.value.kind, K::String(_)) =>
            {
                text = Some(&value.value);
                Role::Text
            }
            T::Bytes if typed(&value.value, &T::Bytes, O::Own) => {
                let [extent] = builtin(&value.value, crate::byte_ops::ZEROED_ID, &[])
                    .ok_or(shape("Bytes field is not direct bytes_zeroed"))?
                else {
                    return Err(forged("bytes_zeroed arity disagrees"));
                };
                width = Some(usize_literal(extent).ok_or(shape("width is not a usize literal"))?);
                producer = Some(&value.value);
                Role::Payload
            }
            _ => return Err(shape("field initializer is outside the closed row")),
        };
    }
    let (Some(text), Some(producer), Some(width)) = (text, producer, width) else {
        return Err(forged("row lacks its String or Bytes operand"));
    };
    if width == 0 || width > crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES {
        return Err(Refusal::Extent);
    }
    let extent = width
        .checked_mul(u64::from(rows))
        .ok_or(Refusal::Extent)?;
    if extent > MAX_EXTENT || extent > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES {
        return Err(Refusal::Extent);
    }
    // Exact publication: `destination = vec_push(destination, row)`.
    if !typed(push, &vector.ty, O::Own) {
        return Err(forged("push result type disagrees"));
    }
    let [old, fresh] = builtin(push, crate::vec_ops::PUSH_ID, std::slice::from_ref(element))
        .ok_or(shape("publication is not vec_push"))?
    else {
        return Err(forged("vec_push arity disagrees"));
    };
    if !place(old, vector) || !place(fresh, item) {
        return Err(shape("publication does not move the row into the same Vec"));
    }
    // Exact unit latch: `induction = induction + 1`.
    let (step_left, step_right) =
        binary(step, BinaryOp::Add).ok_or(shape("latch is not induction + 1"))?;
    if !typed(step, &T::I64, O::Value) {
        return Err(forged("latch type disagrees"));
    }
    if !place(step_left, counter) || integer(step_right) != Some(1) {
        return Err(shape("latch is not induction + 1"));
    }
    // Canonical function-scoped identities at every fixed prefix position.
    // Unique but foreign, or consistently renamed, identities refuse.
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let expressions: [(&E, &str); 17] = [
        (&function.body, "body"),
        (allocation, "body.s0.value"),
        (capacity_literal, "body.s0.value.arg.0"),
        (start, "body.s1.value"),
        (condition.as_ref(), "body.s2.condition"),
        (bound_left, "body.s2.condition.left"),
        (bound_right, "body.s2.condition.right"),
        (body.as_ref(), "body.s2.body"),
        (zero.as_ref(), "body.s2.body.tail"),
        (constructor, "body.s2.body.s0.value"),
        (push, "body.s2.body.s1.value"),
        (old, "body.s2.body.s1.value.arg.0"),
        (fresh, "body.s2.body.s1.value.arg.1"),
        (step, "body.s2.body.s2.value"),
        (step_left, "body.s2.body.s2.value.left"),
        (step_right, "body.s2.body.s2.value.right"),
        (tail.as_ref(), "body.tail"),
    ];
    let bindings = [
        (vector, "body.s0"),
        (counter, "body.s1"),
        (item, "body.s2.body.s0"),
    ];
    if !function.result_id.matches_result(&execution)
        || expressions
            .iter()
            .any(|(expression, path)| !expression.id.matches(&execution, path))
        || bindings
            .iter()
            .any(|(binding, path)| !binding.id.matches_local(&execution, path))
    {
        return Err(forged("construction identity is not its canonical position"));
    }
    // Static paths: the derivation allocates nothing before its census scratch.
    const FIELDS: [(&str, &str); 3] = [
        (
            "body.s2.body.s0.value.field.0.value",
            "body.s2.body.s0.value.field.0.value.arg.0",
        ),
        (
            "body.s2.body.s0.value.field.1.value",
            "body.s2.body.s0.value.field.1.value.arg.0",
        ),
        (
            "body.s2.body.s0.value.field.2.value",
            "body.s2.body.s0.value.field.2.value.arg.0",
        ),
    ];
    for (value, (path, width_path)) in values.iter().zip(FIELDS) {
        if !value.value.id.matches(&execution, path) {
            return Err(forged("row operand identity is not its canonical position"));
        }
        if std::ptr::eq(&value.value, producer) {
            let K::Call { args, .. } = &producer.kind else {
                return Err(forged("producer identity disagrees"));
            };
            if !args[0].id.matches(&execution, width_path) {
                return Err(forged("producer width identity is not its canonical position"));
            }
        }
    }
    if producer.id.as_str().is_empty()
        || !distinct(&[
            &allocation.id,
            &start.id,
            &condition.id,
            &body.id,
            &constructor.id,
            &text.id,
            &producer.id,
            &push.id,
            &step.id,
        ])
    {
        return Err(forged("construction expression identities collide"));
    }
    // Callee closure: every call above is a prelude builtin, so the only
    // user function must not claim one of those identities.
    for id in [
        crate::vec_ops::WITH_CAPACITY_ID,
        crate::vec_ops::PUSH_ID,
        crate::byte_ops::ZEROED_ID,
    ] {
        if function.id.as_str() == id {
            return Err(forged("user function claims a builtin identity"));
        }
    }
    suffix::validate(
        program,
        &suffix::Owners {
            function: function.id.as_str(),
            vector,
            counter,
            item,
            element,
        },
        suffix,
        tail,
    )?;
    Ok(Selection {
        program,
        function,
        record,
        fields,
        element,
        order,
        vector,
        counter,
        item,
        allocation,
        start,
        condition,
        body,
        constructor,
        text,
        producer,
        push,
        step,
        suffix,
        tail,
        rows,
        width,
    })
}
