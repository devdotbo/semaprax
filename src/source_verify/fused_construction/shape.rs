//! Destination, induction, loop, row and producer derivation over exact AST.
use super::{
    Construction, Destination, Induction, Producer, Refusal, MAX_ROW_FIELDS, MAX_ROW_OWNED_FIELDS,
    MAX_SELECTED_EXTENT_BYTES, MAX_SELECTED_ROWS,
};
use crate::ast::{
    BinaryOp, Expr, ExprKind as K, Function, Program, Statement as S, Type, TypeDeclaration,
    TypeDeclarationKind,
};

/// Free-function builtins the shape and the suffix census resolve by name.
/// A program function of the same name refuses the whole module, so a name
/// here can never reach a user helper.
pub(super) const BUILTINS: &[&str] = &[
    "vec_with_capacity",
    "vec_push",
    crate::byte_ops::ZEROED_NAME,
    "vec_len",
    "vec_sort_owned",
    "i64_from_usize",
    "str_len_bytes",
    crate::byte_ops::STR_AS_BYTES_NAME,
    crate::byte_ops::LEN_NAME,
    crate::byte_ops::GET_NAME,
];

/// Type names a row record may not reuse.
const RESERVED_TYPES: &[&str] = &[
    "Option", "Result", "Vec", "Bytes", "Box", "Iter", "IterStep", "String", "string",
];

pub(super) struct Shape<'p> {
    pub(super) record: &'p TypeDeclaration,
    pub(super) destination: Destination<'p>,
    pub(super) induction: Induction<'p>,
    pub(super) construction: Construction<'p>,
    pub(super) producer: Producer<'p>,
    pub(super) rows: u32,
    pub(super) width: u64,
    pub(super) extent: u64,
    pub(super) string_bytes: u64,
    pub(super) suffix: &'p [S],
    pub(super) tail: &'p Expr,
}

fn call<'a>(expr: &'a Expr, name: &str) -> Option<(&'a [Type], &'a [Expr])> {
    match &expr.kind {
        K::Call {
            name: actual,
            type_arguments,
            args,
        } if actual == name => Some((type_arguments, args)),
        _ => None,
    }
}

pub(super) fn var(expr: &Expr, name: &str) -> bool {
    matches!(&expr.kind, K::Var(actual) if actual == name)
}

/// The record named by `arguments`, if it is exactly one bare named type.
pub(super) fn row_type(arguments: &[Type], record: &TypeDeclaration) -> bool {
    matches!(arguments, [Type::Named { name, arguments }]
        if name == &record.name && arguments.is_empty())
}

pub(super) fn reserved_binding(name: &str) -> bool {
    BUILTINS.contains(&name)
}

fn closed_module(program: &Program) -> Result<(), Refusal> {
    if !program.module_uses.is_empty()
        || !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || !program.protocols.is_empty()
        || !program.implementations.is_empty()
        || !program.session_protocols.is_empty()
        || !program.agents.is_empty()
    {
        return Err(Refusal::OpenModule);
    }
    if program
        .functions
        .iter()
        .any(|function| BUILTINS.contains(&function.name.as_str()))
    {
        return Err(Refusal::ShadowedBuiltin);
    }
    Ok(())
}

fn closed_function(function: &Function) -> Result<(), Refusal> {
    if !function.explicit_id
        || !function.type_parameters.is_empty()
        || !function.params.is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || function.follows.is_some()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return Err(Refusal::FunctionShape);
    }
    Ok(())
}

/// The unique explicit record named by the destination's element type.
fn record<'p>(
    program: &'p Program,
    function: &Function,
    name: &str,
) -> Option<&'p TypeDeclaration> {
    let mut found = program.types.iter().filter(|ty| ty.name == name);
    let record = found.next()?;
    if found.next().is_some()
        || RESERVED_TYPES.contains(&name)
        || !record.explicit_id
        || !record.type_parameters.is_empty()
        || record.extends.is_some()
        || !record.invariants().is_empty()
        || record.stable_id == function.stable_id
    {
        return None;
    }
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        return None;
    };
    if !(2..=MAX_ROW_FIELDS).contains(&fields.len()) {
        return None;
    }
    let (mut bytes, mut owned) = (0usize, 0usize);
    for (index, field) in fields.iter().enumerate() {
        if !field.explicit_id
            || field.stable_id == record.stable_id
            || field.stable_id == function.stable_id
            || fields[..index]
                .iter()
                .any(|other| other.name == field.name || other.stable_id == field.stable_id)
        {
            return None;
        }
        match field.ty {
            Type::Bytes => {
                bytes += 1;
                owned += 1;
            }
            Type::String => owned += 1,
            Type::I64 => {}
            _ => return None,
        }
    }
    // The owned-leaf Vec slice admits at most two owned fields; a wider row
    // has no existing collection envelope to select from.
    (bytes == 1 && owned <= MAX_ROW_OWNED_FIELDS).then_some(record)
}

/// Rows the actual record shape may hold in one owned-leaf Vec: the existing
/// independent scalar-storage and owned-carrier envelopes, then this proof's
/// own selected ceiling. Payload contents keep their ordinary caps.
fn row_envelope(record: &TypeDeclaration) -> u64 {
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        return 0;
    };
    let count = |owned: bool| {
        fields
            .iter()
            .filter(|field| matches!(field.ty, Type::String | Type::Bytes) == owned)
            .count() as u64
    };
    let (owned, scalars) = (count(true), count(false));
    let scalar = crate::vec_ops::MAX_CAPACITY
        .checked_div(scalars)
        .unwrap_or(crate::vec_ops::MAX_CAPACITY);
    let carrier = owned
        .checked_mul(crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT)
        .and_then(|per_row| crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES.checked_div(per_row))
        .unwrap_or(0);
    u64::from(MAX_SELECTED_ROWS)
        .min(crate::vec_ops::MAX_CAPACITY)
        .min(scalar)
        .min(carrier)
}

pub(super) fn derive<'p>(
    program: &'p Program,
    function: &'p Function,
) -> Result<Shape<'p>, Refusal> {
    closed_module(program)?;
    closed_function(function)?;
    let K::Block { statements, tail } = &function.body.kind else {
        return Err(Refusal::BodyShape);
    };
    let [first, second, looped @ S::While {
        condition, body, ..
    }, suffix @ ..] = statements.as_slice()
    else {
        return Err(Refusal::BodyShape);
    };
    // The two fresh bindings may appear in either source order; both are
    // retained exactly where they are, so nothing is reordered.
    let fresh_vector = |statement: &S| matches!(statement, S::Let { value, .. } if matches!(&value.kind, K::Call { name, .. } if name == "vec_with_capacity"));
    let (vector_let, induction_let) = if fresh_vector(first) {
        (first, second)
    } else {
        (second, first)
    };

    let S::Let {
        name: vector,
        mutable: true,
        declared: None,
        value: allocation,
        ..
    } = vector_let
    else {
        return Err(Refusal::Destination);
    };
    let (element, args) = call(allocation, "vec_with_capacity").ok_or(Refusal::Destination)?;
    let (
        [Type::Named {
            name: element,
            arguments: element_arguments,
        }],
        [capacity @ Expr {
            kind: K::Usize(capacity_value),
            ..
        }],
    ) = (element, args)
    else {
        return Err(Refusal::Destination);
    };
    if !element_arguments.is_empty() || reserved_binding(vector) {
        return Err(Refusal::Destination);
    }

    let S::Let {
        name: induction,
        mutable: true,
        declared: None,
        value: start,
        ..
    } = induction_let
    else {
        return Err(Refusal::Induction);
    };
    if induction == vector || reserved_binding(induction) || !matches!(start.kind, K::Int(0)) {
        return Err(Refusal::Induction);
    }

    let K::Binary {
        op: BinaryOp::Lt,
        left,
        right: bound,
    } = &condition.kind
    else {
        return Err(Refusal::Condition);
    };
    let K::Int(bound_value) = bound.kind else {
        return Err(Refusal::Condition);
    };
    if !var(left, induction) {
        return Err(Refusal::Condition);
    }
    let rows = u32::try_from(bound_value).map_err(|_| Refusal::Rows)?;
    if rows == 0 || rows > MAX_SELECTED_ROWS || u64::from(rows) != *capacity_value {
        return Err(Refusal::Rows);
    }

    let record = record(program, function, element).ok_or(Refusal::Record)?;
    if u64::from(rows) > row_envelope(record) {
        return Err(Refusal::Rows);
    }

    let K::Block {
        statements: construction,
        tail: construction_tail,
    } = &body.kind
    else {
        return Err(Refusal::LoopBody);
    };
    let [item_let @ S::Let {
        name: item,
        mutable: false,
        declared: None,
        value: constructor,
        ..
    }, push @ S::Assign {
        name: destination,
        field: None,
        value: push_call,
        ..
    }, latch @ S::Assign {
        name: counter,
        field: None,
        value: step,
        ..
    }] = construction.as_slice()
    else {
        return Err(Refusal::LoopBody);
    };
    if item == vector
        || item == induction
        || reserved_binding(item)
        || destination != vector
        || counter != induction
        || !matches!(construction_tail.kind, K::Int(0))
    {
        return Err(Refusal::LoopBody);
    }
    let (push_types, push_args) = call(push_call, "vec_push").ok_or(Refusal::LoopBody)?;
    if !row_type(push_types, record)
        || !matches!(push_args, [old, fresh] if var(old, vector) && var(fresh, item))
    {
        return Err(Refusal::LoopBody);
    }
    if !matches!(&step.kind, K::Binary { op: BinaryOp::Add, left, right }
        if var(left, induction) && matches!(right.kind, K::Int(1)))
    {
        return Err(Refusal::LoopBody);
    }

    let (producer, string_row_bytes) = row(record, constructor, induction)?;
    let width = producer.width;
    let extent = u64::from(rows)
        .checked_mul(width)
        .filter(|extent| *extent <= MAX_SELECTED_EXTENT_BYTES)
        .ok_or(Refusal::Extent)?;
    let string_bytes = string_row_bytes
        .checked_mul(u64::from(rows))
        .ok_or(Refusal::RowConstructor)?;

    Ok(Shape {
        record,
        destination: Destination {
            binding: vector_let,
            name: vector,
            allocation,
            capacity,
        },
        induction: Induction {
            binding: induction_let,
            name: induction,
            bound: &**bound,
            latch,
            step,
        },
        construction: Construction {
            statement: looped,
            index: 2,
            condition: &**condition,
            body: &**body,
            item: item_let,
            constructor,
            push,
            push_call,
        },
        producer: producer.producer,
        rows,
        width,
        extent,
        string_bytes,
        suffix,
        tail,
    })
}

struct RowProducer<'p> {
    producer: Producer<'p>,
    width: u64,
}

/// Check the row constructor and return its single producer and the UTF-8
/// bytes its String literals materialize per row.
fn row<'p>(
    record: &'p TypeDeclaration,
    constructor: &'p Expr,
    induction: &str,
) -> Result<(RowProducer<'p>, u64), Refusal> {
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(Refusal::Record);
    };
    let K::ConstructRecord {
        type_name,
        type_arguments,
        fields: initializers,
        ..
    } = &constructor.kind
    else {
        return Err(Refusal::RowConstructor);
    };
    if type_name != &record.name || !type_arguments.is_empty() || initializers.len() != fields.len()
    {
        return Err(Refusal::RowConstructor);
    }
    let mut producer = None;
    let mut string_bytes = 0u64;
    for (index, (field, initializer)) in fields.iter().zip(initializers).enumerate() {
        if field.name != initializer.name {
            return Err(Refusal::RowConstructor);
        }
        let value = &initializer.value;
        match field.ty {
            Type::I64 if var(value, induction) || matches!(value.kind, K::Int(_)) => {}
            Type::String => {
                let K::String(text) = &value.kind else {
                    return Err(Refusal::RowConstructor);
                };
                string_bytes = u64::try_from(text.len())
                    .ok()
                    .and_then(|len| string_bytes.checked_add(len))
                    .ok_or(Refusal::RowConstructor)?;
            }
            Type::Bytes => {
                // Closed producer domain: the direct zeroed allocation with a
                // literal positive width. Bounded, copying, stdin-reading and
                // helper-produced Bytes have no fixed width here.
                let (types, args) =
                    call(value, crate::byte_ops::ZEROED_NAME).ok_or(Refusal::Producer)?;
                let (
                    [],
                    [width_literal @ Expr {
                        kind: K::Usize(width),
                        ..
                    }],
                ) = (types, args)
                else {
                    return Err(Refusal::Producer);
                };
                if *width == 0 {
                    return Err(Refusal::Producer);
                }
                producer = Some(RowProducer {
                    producer: Producer {
                        call: value,
                        width_literal,
                        field,
                        initializer: index,
                    },
                    width: *width,
                });
            }
            _ => return Err(Refusal::RowConstructor),
        }
    }
    Ok((producer.ok_or(Refusal::Producer)?, string_bytes))
}
