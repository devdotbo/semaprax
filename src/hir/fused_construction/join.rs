//! Exact source/HIR join for one typed fused-construction selection.
//!
//! The join receives the source half as an actual
//! [`source_verify::fused_construction::SourceProof`](crate::source_verify::fused_construction::SourceProof),
//! authenticates it over the exact current AST, binds each borrowed source node
//! to its position in that AST, rereads every literal and compares count,
//! polarity, destination, row fields, producer, publication and latch with the
//! independently derived HIR [`Selection`]. It then proves the complete suffix
//! and tail correspondence (see `correspond`). The source raw join key is never
//! consulted: anyone can build one.
//!
//! A [`Joined`] value is received proof data, not a validated publication or
//! artifact admission. Ordinary `check`/`resolve` keep SPX-T267, and eventual
//! admission still requires full ordinary HIR, ownership, loan, cleanup-plan
//! and status validation under the selected strategy plus every executor.
use super::{Refusal, Selection};
use crate::ast::{self, BinaryOp, ExprKind as A, Statement as AS};
use crate::hir::{ResolvedExpr as E, ResolvedExprKind as K, ResolvedType as T};
use crate::source_verify::fused_construction::{self as source_fusion, Derivation, SourceProof};

/// Derive the actual source proof for the only function of `program`.
pub(crate) fn current_source(program: &ast::Program) -> Result<SourceProof<'_>, Refusal> {
    let [function] = program.functions.as_slice() else {
        return Err(join_refusal("source is not exactly one function"));
    };
    match source_fusion::derive(program, function) {
        Ok(Derivation::Selected(proof)) => Ok(proof),
        Ok(Derivation::Legacy32 { .. }) => Err(Refusal::Legacy),
        Err(_) => Err(join_refusal("source derivation refused")),
    }
}

/// The selected allocation flow, borrowed from the joined HIR. A capacity
/// walker under `FusedConstructionV1` folds `condition` and `body` itself and
/// names `producer` as the body's single Bytes site; this value supplies no
/// folded summary, scan debit or lease metadata, and selects nothing by itself.
#[derive(Clone, Copy)]
pub(crate) struct SelectedFlow<'a> {
    pub(crate) producer: &'a str,
    pub(crate) rows: u32,
    pub(crate) width: u64,
    pub(crate) condition: &'a E,
    pub(crate) body: &'a E,
}

pub(crate) struct Joined<'a> {
    source: &'a ast::Program,
    proof: SourceProof<'a>,
    selection: Selection<'a>,
}

impl<'a> Joined<'a> {
    pub(crate) fn selection(&self) -> &Selection<'a> {
        &self.selection
    }
    pub(crate) fn proof(&self) -> &SourceProof<'a> {
        &self.proof
    }
    pub(crate) fn flow(&self) -> SelectedFlow<'a> {
        SelectedFlow {
            producer: self.selection.producer().id.as_str(),
            rows: self.selection.rows(),
            width: self.selection.width(),
            condition: self.selection.condition(),
            body: self.selection.body(),
        }
    }
    /// Pointer-only discovery while walking the joined HIR.
    pub(crate) fn selects(&self, condition: &E, body: &E) -> Option<SelectedFlow<'a>> {
        self.selection.is_loop(condition, body).then(|| self.flow())
    }
    /// Both halves must still be the exact objects that were joined.
    pub(crate) fn still_current(
        &self,
        source: &ast::Program,
        program: &crate::hir::ResolvedProgram,
    ) -> bool {
        std::ptr::eq(self.source, source)
            && self.proof.still_held(source)
            && self.selection.still_held(program)
    }
}

const fn join_refusal(reason: &'static str) -> Refusal {
    Refusal::Join(reason)
}

fn check(condition: bool, reason: &'static str) -> Result<(), Refusal> {
    if condition {
        Ok(())
    } else {
        Err(join_refusal(reason))
    }
}

fn var(e: &ast::Expr) -> Option<&str> {
    match &e.kind {
        A::Var(name) => Some(name.as_str()),
        _ => None,
    }
}

fn call<'s>(e: &'s ast::Expr, name: &str) -> Option<(&'s [ast::Type], &'s [ast::Expr])> {
    match &e.kind {
        A::Call {
            name: actual,
            type_arguments,
            args,
        } if actual == name => Some((type_arguments.as_slice(), args.as_slice())),
        _ => None,
    }
}

/// `types` is exactly the joined record, named by the HIR record's spelling.
pub(super) fn row_type(types: &[ast::Type], record: &crate::hir::ResolvedTypeDeclaration) -> bool {
    matches!(types, [ast::Type::Named { name, arguments }]
        if *name == record.name && arguments.is_empty())
}

fn hir_operands(e: &E) -> Option<(&E, &E)> {
    match &e.kind {
        K::Binary { left, right, .. } => Some((left.as_ref(), right.as_ref())),
        _ => None,
    }
}

/// Join an authenticated source proof over exact current source with the
/// independently derived current HIR.
pub(crate) fn join<'a>(
    source: &'a ast::Program,
    proof: SourceProof<'a>,
    program: &'a crate::hir::ResolvedProgram,
) -> Result<Joined<'a>, Refusal> {
    check(
        proof.authenticate(source),
        "source proof is not authenticated over current source",
    )?;
    let selection = super::derive(program)?;
    check(source.module == program.module, "module differs")?;
    let [function] = source.functions.as_slice() else {
        return Err(join_refusal("source is not exactly one function"));
    };
    let record = proof.record();
    check(
        std::ptr::eq(function, proof.function())
            && source
                .types
                .iter()
                .any(|candidate| std::ptr::eq(candidate, record)),
        "source proof is not over current source declarations",
    )?;
    let hir_function = selection.function();
    check(
        function.explicit_id
            && function.stable_id == hir_function.id.as_str()
            && function.name == hir_function.name
            && function.span == hir_function.span,
        "function identity differs",
    )?;
    check(
        record.explicit_id
            && record.stable_id == selection.record().id.as_str()
            && record.name == selection.record().name,
        "record identity differs",
    )?;
    let A::Block { statements, tail } = &function.body.kind else {
        return Err(join_refusal("source body is not a block"));
    };
    let [destination_statement, induction_statement, loop_statement, suffix @ ..] =
        statements.as_slice()
    else {
        return Err(join_refusal("source prefix differs"));
    };
    let AS::Let {
        name: vector,
        mutable: true,
        declared: None,
        value: allocation,
        ..
    } = destination_statement
    else {
        return Err(join_refusal("source destination differs"));
    };
    let AS::Let {
        name: counter,
        mutable: true,
        declared: None,
        value: start,
        ..
    } = induction_statement
    else {
        return Err(join_refusal("source induction differs"));
    };
    let AS::While {
        condition, body, ..
    } = loop_statement
    else {
        return Err(join_refusal("source loop differs"));
    };
    // Every borrowed proof node is the node at its exact source position.
    let (destination, induction, construction) =
        (proof.destination(), proof.induction(), proof.construction());
    check(
        std::ptr::eq(destination.binding, destination_statement)
            && std::ptr::eq(destination.allocation, allocation)
            && destination.name == vector
            && std::ptr::eq(induction.binding, induction_statement)
            && induction.name == counter
            && std::ptr::eq(construction.statement, loop_statement)
            && construction.index == 2
            && std::ptr::eq(construction.condition, condition.as_ref())
            && std::ptr::eq(construction.body, body.as_ref()),
        "source proof nodes are not at their current positions",
    )?;
    check(
        tail.span == selection.tail().span,
        "function result differs",
    )?;
    // Destination and count operands.
    check(
        *vector == selection.vector().name && *counter == selection.counter().name,
        "destination or induction binding differs",
    )?;
    let (types, args) =
        call(allocation, "vec_with_capacity").ok_or(join_refusal("allocation differs"))?;
    let [capacity] = args else {
        return Err(join_refusal("allocation arity differs"));
    };
    check(
        std::ptr::eq(destination.capacity, capacity)
            && row_type(types, selection.record())
            && allocation.span == selection.allocation().span,
        "allocation differs",
    )?;
    let A::Usize(capacity) = capacity.kind else {
        return Err(join_refusal("capacity operand differs"));
    };
    check(
        matches!(start.kind, A::Int(0)) && start.span == selection.start().span,
        "induction start differs",
    )?;
    // Polarity: `induction < count` with the induction on the left.
    let A::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return Err(join_refusal("condition polarity differs"));
    };
    let A::Int(bound) = right.kind else {
        return Err(join_refusal("condition operand differs"));
    };
    // Literal operands are compared by value; their enclosing spans bind position.
    let (hir_left, _) =
        hir_operands(selection.condition()).ok_or(join_refusal("condition differs"))?;
    check(
        std::ptr::eq(induction.bound, right.as_ref())
            && var(left) == Some(counter.as_str())
            && condition.span == selection.condition().span
            && left.span == hir_left.span,
        "condition differs",
    )?;
    let count = u64::from(selection.rows());
    check(
        capacity == count
            && u64::try_from(bound).ok() == Some(count)
            && u64::from(proof.rows()) == count,
        "row count differs",
    )?;
    // Repeated row, publication and latch.
    let A::Block {
        statements: rows,
        tail: zero,
    } = &body.kind
    else {
        return Err(join_refusal("loop body differs"));
    };
    let [item_statement, push_statement, latch_statement] = rows.as_slice() else {
        return Err(join_refusal("loop body differs"));
    };
    let (
        AS::Let {
            name: item,
            mutable: false,
            declared: None,
            value: constructor,
            ..
        },
        AS::Assign {
            name: target,
            field: None,
            value: push,
            ..
        },
        AS::Assign {
            name: latch,
            field: None,
            value: step,
            ..
        },
    ) = (item_statement, push_statement, latch_statement)
    else {
        return Err(join_refusal("loop body differs"));
    };
    check(
        std::ptr::eq(construction.item, item_statement)
            && std::ptr::eq(construction.constructor, constructor)
            && std::ptr::eq(construction.push, push_statement)
            && std::ptr::eq(construction.push_call, push)
            && std::ptr::eq(induction.latch, latch_statement)
            && std::ptr::eq(induction.step, step),
        "source proof row nodes are not at their current positions",
    )?;
    check(
        body.span == selection.body().span
            && matches!(zero.kind, A::Int(0))
            && *item == selection.item().name
            && target == vector
            && latch == counter,
        "loop body differs",
    )?;
    fields(&proof, record, constructor, counter, &selection)?;
    let (types, args) = call(push, "vec_push").ok_or(join_refusal("publication differs"))?;
    check(
        row_type(types, selection.record())
            && push.span == selection.push().span
            && matches!(args, [old, fresh]
                if var(old) == Some(vector.as_str()) && var(fresh) == Some(item.as_str())),
        "publication differs",
    )?;
    check(
        step.span == selection.step().span
            && matches!(&step.kind, A::Binary { op: BinaryOp::Add, left, right }
                if var(left) == Some(counter.as_str()) && matches!(right.kind, A::Int(1))),
        "latch differs",
    )?;
    check(
        proof.width() == selection.width() && proof.extent() == selection.extent(),
        "producer extent differs",
    )?;
    super::correspond::suffix(program, &selection, &proof, suffix, tail)?;
    Ok(Joined {
        source,
        proof,
        selection,
    })
}

/// Row fields in declaration order: identity, type, operand and producer.
/// The HIR row is the narrowed exact i64/String/Bytes profile; any other
/// source-admitted record shape refuses here.
fn fields(
    proof: &SourceProof<'_>,
    record: &ast::TypeDeclaration,
    constructor: &ast::Expr,
    counter: &str,
    selection: &Selection<'_>,
) -> Result<(), Refusal> {
    let A::ConstructRecord {
        type_name,
        type_arguments,
        fields: initializers,
        ..
    } = &constructor.kind
    else {
        return Err(join_refusal("row constructor differs"));
    };
    let ast::TypeDeclarationKind::Record { fields: declared } = &record.kind else {
        return Err(join_refusal("record kind differs"));
    };
    let K::ConstructRecord { fields: values, .. } = &selection.constructor().kind else {
        return Err(join_refusal("row constructor differs"));
    };
    let typed = selection.fields();
    check(
        *type_name == record.name
            && type_arguments.is_empty()
            && constructor.span == selection.constructor().span
            && initializers.len() == typed.len()
            && declared.len() == typed.len()
            && values.len() == typed.len(),
        "row constructor differs",
    )?;
    let producer = proof.producer();
    let mut found = false;
    for (index, (((source_field, initializer), field), value)) in declared
        .iter()
        .zip(initializers)
        .zip(typed)
        .zip(values)
        .enumerate()
    {
        check(
            source_field.explicit_id
                && source_field.stable_id == field.id.as_str()
                && source_field.name == field.name
                && source_field.span == field.span
                && initializer.name == field.name
                && initializer.value.span == value.value.span,
            "row field differs",
        )?;
        match (&source_field.ty, &field.ty) {
            (ast::Type::I64, T::I64) => check(
                var(&initializer.value) == Some(counter),
                "row induction operand differs",
            )?,
            (ast::Type::String, T::String) => check(
                matches!((&initializer.value.kind, &value.value.kind),
                    (A::String(left), K::String(right)) if left == right),
                "row text operand differs",
            )?,
            (ast::Type::Bytes, T::Bytes) => {
                let (types, args) = call(&initializer.value, "bytes_zeroed")
                    .ok_or(join_refusal("producer differs"))?;
                check(
                    std::ptr::eq(&initializer.value, producer.call)
                        && std::ptr::eq(source_field, producer.field)
                        && producer.initializer == index
                        && types.is_empty()
                        && std::ptr::eq(&value.value, selection.producer())
                        && matches!(args, [width @ ast::Expr { kind: A::Usize(literal), .. }]
                            if std::ptr::eq(width, producer.width_literal)
                                && *literal == selection.width()),
                    "producer differs",
                )?;
                found = true;
            }
            _ => return Err(join_refusal("row field type differs")),
        }
    }
    check(found, "producer differs")
}
