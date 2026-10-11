//! Source-side selection proof for the `FusedConstructionV1` allocation
//! strategy (OPT #747).
//!
//! This module independently derives, from the exact current AST, a borrowed
//! proof that one function builds a fresh empty row Vec with an exact bounded
//! counted induction, and that each row constructor issues exactly one
//! fixed-width Bytes-producing call. Such a function is the only kind the
//! fused capacity rule may ever select.
//!
//! The proof is provisional. It is not a capacity flow, admission decision,
//! cleanup certificate, schema or backend artifact. Whole-program source and
//! HIR validation, an independent typed-HIR proof of the construction, closed
//! suffix and current owners, and an equal [`RawJoinKey`] must all hold before
//! any consumer may construct a fused capacity selection; the raw key alone is
//! never authority. Default admission stays on the
//! legacy strategy until every executor implements the lease domain. Counts at
//! or below the legacy 32-event cap derive [`Derivation::Legacy32`] and keep
//! the ordinary `FiniteOwnedLoop` route; String charges, left-to-right order
//! and canonical cleanup are untouched because nothing here rewrites a flow.
//!
//! Names select only exact lexical bindings and declarations. No filename,
//! benchmark identity or helper-function name is a selector. Every builtin
//! this proof relies on must be unshadowed by a program declaration.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "consumed only by the pending HIR join and fused capacity selector"
    )
)]

use crate::ast::{Expr, FieldDeclaration, Function, Program, Span, Statement, TypeDeclaration};

mod shape;
mod suffix;
#[cfg(test)]
mod tests;

/// Highest row count still owned by the legacy finite route. Restated here,
/// not imported, so the selection boundary is an independent source constant;
/// the owning tests pin it against `byte_data_capacity::MAX_BYTES_COPY_SITES`.
pub(crate) const LEGACY_FINITE_ROWS: u32 = 32;
/// Inclusive row ceiling for one selected construction: the owned-record Vec
/// envelope with two owned fields, 131072 / (2 * 16).
pub(crate) const MAX_SELECTED_ROWS: u32 = 4_096;
/// Inclusive backing ceiling: the unchanged single owned Bytes extent.
pub(crate) const MAX_SELECTED_EXTENT_BYTES: u64 = 131_072;
/// Inclusive declared field bound for an admissible row record.
pub(crate) const MAX_ROW_FIELDS: usize = 8;
/// Inclusive owned (String or Bytes) field bound, matching the existing
/// owned-leaf Vec slice; exactly one of them is the Bytes producer field.
pub(crate) const MAX_ROW_OWNED_FIELDS: usize = 2;

/// Why a function has no selected source proof. Variants are ordered by the
/// derivation step that refuses; the first failing step wins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    /// Imports, permits, interfaces, protocols, implementations, sessions or
    /// agents make the module open.
    OpenModule,
    /// A program declaration shadows a builtin this proof relies on.
    ShadowedBuiltin,
    /// The function is generic, parameterized, effectful, yielding, follows a
    /// protocol, carries contracts, or has no explicit identity.
    FunctionShape,
    /// The body is not a block opening with the destination, the induction and
    /// the construction loop.
    BodyShape,
    /// The destination is not a fresh empty `vec_with_capacity<Row>(N)`.
    Destination,
    /// The induction is not a fresh mutable binding initialized to zero.
    Induction,
    /// The loop condition is not exactly `induction < N` with literal `N`.
    Condition,
    /// Rows are zero, exceed the actual record's scalar-storage or
    /// owned-carrier Vec envelope or [`MAX_SELECTED_ROWS`], or differ from the
    /// destination capacity.
    Rows,
    /// The record is not one explicit, nongeneric, uniquely named record with
    /// admissible field types, exactly one Bytes field and at most
    /// [`MAX_ROW_OWNED_FIELDS`] owned fields.
    Record,
    /// The loop body is not exactly construct, push into the destination and
    /// unit latch, with a unit-valued tail.
    LoopBody,
    /// The row constructor or one of its non-Bytes initializers is not closed.
    RowConstructor,
    /// The Bytes initializer is not the selected fixed-width producer.
    Producer,
    /// `rows * width` overflows or exceeds [`MAX_SELECTED_EXTENT_BYTES`].
    Extent,
    /// A remaining use of the destination, induction or a view escapes the
    /// closed suffix domain.
    Suffix,
}

/// Result of a successful source derivation.
pub(crate) enum Derivation<'p> {
    /// The shape is exact but the row count stays on the legacy finite route.
    Legacy32 { rows: u32 },
    /// The provisional source half of a `FusedConstructionV1` selection.
    Selected(SourceProof<'p>),
}

/// Statement and expression identities of the destination row Vec.
pub(crate) struct Destination<'p> {
    pub(crate) binding: &'p Statement,
    pub(crate) name: &'p str,
    pub(crate) allocation: &'p Expr,
    pub(crate) capacity: &'p Expr,
}

/// Statement and expression identities of the counted induction.
pub(crate) struct Induction<'p> {
    pub(crate) binding: &'p Statement,
    pub(crate) name: &'p str,
    pub(crate) bound: &'p Expr,
    pub(crate) latch: &'p Statement,
    pub(crate) step: &'p Expr,
}

/// Identities of the construction loop and its single row.
pub(crate) struct Construction<'p> {
    pub(crate) statement: &'p Statement,
    /// Index of the loop among the function body's statements.
    pub(crate) index: usize,
    pub(crate) condition: &'p Expr,
    pub(crate) body: &'p Expr,
    pub(crate) item: &'p Statement,
    pub(crate) constructor: &'p Expr,
    pub(crate) push: &'p Statement,
    pub(crate) push_call: &'p Expr,
}

/// The single selected Bytes producer inside the row constructor.
pub(crate) struct Producer<'p> {
    pub(crate) call: &'p Expr,
    pub(crate) width_literal: &'p Expr,
    pub(crate) field: &'p FieldDeclaration,
    /// Initializer position in the constructor, which the source capacity walk
    /// uses for the producer's flow site.
    pub(crate) initializer: usize,
}

/// One owner transition recorded by the closed suffix census, in source order.
#[derive(Clone, Copy)]
pub(crate) enum OwnerEvent<'p> {
    /// `let next = owner;` invalidates the prior binding.
    Move(&'p Statement),
    /// `let next = vec_sort_owned<Row>(owner);` consumes and renews.
    Sort(&'p Statement),
    /// `for borrow row in owner { .. }` lends iteration-local views.
    Borrow(&'p Statement),
}

/// Bound on recorded owner transitions after the loop.
pub(crate) const MAX_OWNER_EVENTS: usize = 16;

/// Borrowed, provisional source proof. Every reference points into the exact
/// `Program` it was derived from; [`SourceProof::still_held`] and
/// [`SourceProof::authenticate`] refuse a clone or a re-parsed module.
pub(crate) struct SourceProof<'p> {
    program: &'p Program,
    function: &'p Function,
    record: &'p TypeDeclaration,
    destination: Destination<'p>,
    induction: Induction<'p>,
    construction: Construction<'p>,
    producer: Producer<'p>,
    events: [Option<OwnerEvent<'p>>; MAX_OWNER_EVENTS],
    rows: u32,
    width: u64,
    extent: u64,
    string_bytes: u64,
}

impl<'p> SourceProof<'p> {
    pub(crate) fn function(&self) -> &'p Function {
        self.function
    }
    pub(crate) fn record(&self) -> &'p TypeDeclaration {
        self.record
    }
    pub(crate) fn destination(&self) -> &Destination<'p> {
        &self.destination
    }
    pub(crate) fn induction(&self) -> &Induction<'p> {
        &self.induction
    }
    pub(crate) fn construction(&self) -> &Construction<'p> {
        &self.construction
    }
    pub(crate) fn producer(&self) -> &Producer<'p> {
        &self.producer
    }
    /// Owner transitions after the loop, in source order.
    pub(crate) fn owner_events(&self) -> impl Iterator<Item = OwnerEvent<'p>> + '_ {
        self.events.iter().map_while(|event| *event)
    }
    pub(crate) fn rows(&self) -> u32 {
        self.rows
    }
    pub(crate) fn width(&self) -> u64 {
        self.width
    }
    /// The proposed single backing extent, `rows * width`.
    pub(crate) fn extent(&self) -> u64 {
        self.extent
    }
    /// Owned UTF-8 bytes the source still materializes as ordinary Strings.
    /// This is an inventory fact, never a discount.
    pub(crate) fn string_bytes(&self) -> u64 {
        self.string_bytes
    }

    /// Flow site the source capacity walk assigns the producer: the function
    /// body, the loop statement, the item `let` and the constructor field.
    pub(crate) fn producer_site(&self) -> String {
        format!(
            "{}.body.s{}.body.s0.value.field.{}",
            self.function.stable_id, self.construction.index, self.producer.initializer
        )
    }

    /// Allocation-free comparison against a capacity flow site.
    pub(crate) fn producer_site_matches(&self, site: &str) -> bool {
        use std::fmt::Write;
        struct Matcher<'s>(&'s str);
        impl Write for Matcher<'_> {
            fn write_str(&mut self, part: &str) -> std::fmt::Result {
                self.0 = self.0.strip_prefix(part).ok_or(std::fmt::Error)?;
                Ok(())
            }
        }
        let mut rest = Matcher(site);
        write!(
            rest,
            "{}.body.s{}.body.s0.value.field.{}",
            self.function.stable_id, self.construction.index, self.producer.initializer
        )
        .is_ok()
            && rest.0.is_empty()
    }

    /// True only for the exact borrowed module this proof was derived from.
    pub(crate) fn still_held(&self, program: &Program) -> bool {
        std::ptr::eq(self.program, program)
    }

    /// Re-derive from the held module and require every borrowed identity to
    /// be the same node. A stale or foreign proof refuses.
    pub(crate) fn authenticate(&self, program: &Program) -> bool {
        if !self.still_held(program) {
            return false;
        }
        let Ok(Derivation::Selected(fresh)) = derive(program, self.function) else {
            return false;
        };
        self.same_identities(&fresh)
    }

    fn same_identities(&self, other: &SourceProof<'_>) -> bool {
        let e = |left: &Expr, right: &Expr| std::ptr::eq(left, right);
        let s = |left: &Statement, right: &Statement| std::ptr::eq(left, right);
        let (d, od) = (&self.destination, &other.destination);
        let (i, oi) = (&self.induction, &other.induction);
        let (c, oc) = (&self.construction, &other.construction);
        let (p, op) = (&self.producer, &other.producer);
        std::ptr::eq(self.program, other.program)
            && std::ptr::eq(self.function, other.function)
            && std::ptr::eq(self.record, other.record)
            && s(d.binding, od.binding)
            && e(d.allocation, od.allocation)
            && e(d.capacity, od.capacity)
            && s(i.binding, oi.binding)
            && e(i.bound, oi.bound)
            && s(i.latch, oi.latch)
            && e(i.step, oi.step)
            && s(c.statement, oc.statement)
            && c.index == oc.index
            && e(c.condition, oc.condition)
            && e(c.body, oc.body)
            && s(c.item, oc.item)
            && e(c.constructor, oc.constructor)
            && s(c.push, oc.push)
            && e(c.push_call, oc.push_call)
            && e(p.call, op.call)
            && e(p.width_literal, op.width_literal)
            && std::ptr::eq(p.field, op.field)
            && p.initializer == op.initializer
            && self
                .events
                .iter()
                .zip(&other.events)
                .all(|pair| match pair {
                    (None, None) => true,
                    (Some(OwnerEvent::Move(a)), Some(OwnerEvent::Move(b)))
                    | (Some(OwnerEvent::Sort(a)), Some(OwnerEvent::Sort(b)))
                    | (Some(OwnerEvent::Borrow(a)), Some(OwnerEvent::Borrow(b))) => s(*a, *b),
                    _ => false,
                })
            && (self.rows, self.width, self.extent, self.string_bytes)
                == (other.rows, other.width, other.extent, other.string_bytes)
    }

    /// Raw comparison data for a later join; see [`RawJoinKey`]. Building or
    /// comparing it grants nothing.
    pub(crate) fn raw_join_key(&self) -> RawJoinKey {
        let span = statement_span;
        RawJoinKey {
            module: self.program.module.clone(),
            function: self.function.stable_id.clone(),
            function_span: self.function.span,
            record: self.record.stable_id.clone(),
            byte_field: self.producer.field.stable_id.clone(),
            byte_field_span: self.producer.field.span,
            destination: self.destination.name.to_owned(),
            destination_span: span(self.destination.binding),
            allocation_span: self.destination.allocation.span,
            induction: self.induction.name.to_owned(),
            induction_span: span(self.induction.binding),
            loop_span: span(self.construction.statement),
            condition_span: self.construction.condition.span,
            body_span: self.construction.body.span,
            constructor_span: self.construction.constructor.span,
            producer_span: self.producer.call.span,
            push_span: self.construction.push_call.span,
            latch_span: self.induction.step.span,
            rows: self.rows,
            width: self.width,
            extent: self.extent,
            string_bytes: self.string_bytes,
            producer_site: self.producer_site(),
        }
    }
}

/// Plain, freely constructible comparison data; never authority.
///
/// Anyone can build one, so equality between two keys proves nothing about
/// either side's AST or HIR. It also omits the suffix owner events and every
/// current-owner fact after the loop. A join therefore requires, in addition
/// to equal keys: a held, authenticated [`SourceProof`] over the actual AST;
/// whole-program source and HIR validation of the same module; and an
/// independent typed-HIR proof of the construction, the closed suffix and
/// the current owner at every transfer. Only after all of that may a
/// consumer build a fused capacity selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RawJoinKey {
    pub(crate) module: String,
    pub(crate) function: String,
    pub(crate) function_span: Span,
    pub(crate) record: String,
    pub(crate) byte_field: String,
    pub(crate) byte_field_span: Span,
    pub(crate) destination: String,
    pub(crate) destination_span: Span,
    pub(crate) allocation_span: Span,
    pub(crate) induction: String,
    pub(crate) induction_span: Span,
    pub(crate) loop_span: Span,
    pub(crate) condition_span: Span,
    pub(crate) body_span: Span,
    pub(crate) constructor_span: Span,
    pub(crate) producer_span: Span,
    pub(crate) push_span: Span,
    pub(crate) latch_span: Span,
    pub(crate) rows: u32,
    pub(crate) width: u64,
    pub(crate) extent: u64,
    pub(crate) string_bytes: u64,
    pub(crate) producer_site: String,
}

fn statement_span(statement: &Statement) -> Span {
    match statement {
        Statement::Let { span, .. }
        | Statement::Assign { span, .. }
        | Statement::Unsafe { span, .. }
        | Statement::While { span, .. }
        | Statement::For { span, .. }
        | Statement::ForBorrow { span, .. }
        | Statement::ForOwn { span, .. } => *span,
    }
}

/// Derive the source proof for one function of `program`. `function` must be
/// an element of `program.functions`; any other reference refuses.
pub(crate) fn derive<'p>(
    program: &'p Program,
    function: &'p Function,
) -> Result<Derivation<'p>, Refusal> {
    if !program
        .functions
        .iter()
        .any(|candidate| std::ptr::eq(candidate, function))
    {
        return Err(Refusal::FunctionShape);
    }
    let shape = shape::derive(program, function)?;
    let events = suffix::census(&shape)?;
    if shape.rows <= LEGACY_FINITE_ROWS {
        return Ok(Derivation::Legacy32 { rows: shape.rows });
    }
    Ok(Derivation::Selected(SourceProof {
        program,
        function,
        record: shape.record,
        destination: shape.destination,
        induction: shape.induction,
        construction: shape.construction,
        producer: shape.producer,
        events,
        rows: shape.rows,
        width: shape.width,
        extent: shape.extent,
        string_bytes: shape.string_bytes,
    }))
}

/// The selected proof whose construction loop owns `expression`, which must be
/// the exact borrowed loop condition or body node. Legacy counts, refusals and
/// clones of either node select nothing.
pub(crate) fn select<'p>(program: &'p Program, expression: &Expr) -> Option<SourceProof<'p>> {
    program.functions.iter().find_map(|function| {
        let Ok(Derivation::Selected(proof)) = derive(program, function) else {
            return None;
        };
        let construction = proof.construction();
        (std::ptr::eq(expression, construction.condition)
            || std::ptr::eq(expression, construction.body))
        .then_some(proof)
    })
}
