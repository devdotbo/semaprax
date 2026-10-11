//! Independent typed `FusedConstructionV1` derivation for OPT #747.
//!
//! This module rebuilds one closed finite construction loop from the current
//! resolved HIR alone. It consumes no source inventory, cache label, graph fact,
//! attached plan or binding spelling as authority. A [`Selection`] is borrowed
//! proof data for a later executor; it is not admission. Ordinary capacity keeps
//! `AllocationStrategy::Legacy`, the unchanged 32-event `FiniteOwnedLoop` and
//! SPX-T267 for every count this module selects. Every fixed prefix expression
//! and binding must carry its canonical function-scoped identity (with the
//! cached `ValueId` fingerprint); unique-but-foreign or consistently renamed
//! identities refuse. The HIR row is narrowed to exactly one i64, one String and
//! one Bytes field; other source-admitted records refuse honestly.
//!
//! [`join`] receives an authenticated source `SourceProof`, binds it to exact
//! current source and proves complete suffix correspondence in `correspond`.
//! A joined value is received proof data, never a validated publication: full
//! ordinary HIR, ownership, loan, cleanup-plan and status validation under the
//! selected strategy, capacity routing and every executor remain required.
use crate::ast::BinaryOp;
use crate::hir::{
    ExpressionId, OwnershipMode as O, ResolvedBinding, ResolvedExpr as E, ResolvedExprKind as K,
    ResolvedFieldDeclaration, ResolvedFunction, ResolvedProgram, ResolvedStatement as S,
    ResolvedType as T, ResolvedTypeDeclaration,
};
mod correspond;
pub(crate) mod join;
mod shape;
mod suffix;
#[cfg(test)]
mod tests;

/// Smallest selected row count. Counts up to the unchanged 32-event cap stay
/// with the ordinary `FiniteOwnedLoop` profile and are never co-selected.
pub(crate) const MIN_ROWS: u32 = crate::byte_data_capacity::MAX_BYTES_COPY_SITES + 1;
/// Largest selected row count: the owned-record Vec envelope for one scalar and
/// two owned fields. The actual record layout bound is checked independently.
pub(crate) const MAX_ROWS: u32 = 4_096;
/// The single proposed backing must fit the unchanged individual Bytes extent.
pub(crate) const MAX_EXTENT: u64 = crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES;

/// Why the typed derivation or the source/HIR join refused. Every refusal
/// leaves ordinary admission unchanged; none is a diagnostic code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    /// Not the closed construction family (an ordinary program).
    Shape(&'static str),
    /// Typed facts the resolver cannot produce: decoded or mutated HIR.
    Forged(&'static str),
    /// At most 32 rows: the unchanged ordinary finite profile owns it.
    Legacy,
    /// Above [`MAX_ROWS`] or the actual record layout's Vec envelope.
    Rows,
    /// Zero width, or a checked extent above [`MAX_EXTENT`] or the cumulative
    /// payload bound.
    Extent,
    /// The post-construction suffix is not the closed read-only use domain.
    Suffix(&'static str),
    /// The fixed census scratch could not be reserved; nothing is published.
    Scratch,
    /// Source and HIR disagree, or the source facts are not current source.
    Join(&'static str),
}

/// Field roles in declaration and left-to-right evaluation order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Induction,
    Text,
    Payload,
}

/// Borrowed typed facts for one selected loop. Every reference points into the
/// exact `ResolvedProgram` the derivation read; equal clones do not qualify.
pub(crate) struct Selection<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    record: &'a ResolvedTypeDeclaration,
    fields: &'a [ResolvedFieldDeclaration],
    element: &'a T,
    order: [Role; 3],
    vector: &'a ResolvedBinding,
    counter: &'a ResolvedBinding,
    item: &'a ResolvedBinding,
    allocation: &'a E,
    start: &'a E,
    condition: &'a E,
    body: &'a E,
    constructor: &'a E,
    text: &'a E,
    producer: &'a E,
    push: &'a E,
    step: &'a E,
    suffix: &'a [S],
    tail: &'a E,
    rows: u32,
    width: u64,
}

impl<'a> Selection<'a> {
    pub(crate) fn program(&self) -> &'a ResolvedProgram {
        self.program
    }
    pub(crate) fn function(&self) -> &'a ResolvedFunction {
        self.function
    }
    pub(crate) fn record(&self) -> &'a ResolvedTypeDeclaration {
        self.record
    }
    pub(crate) fn fields(&self) -> &'a [ResolvedFieldDeclaration] {
        self.fields
    }
    /// The destination's exact row element type.
    pub(crate) fn element(&self) -> &'a T {
        self.element
    }
    /// Evaluation order of the fresh row's operands. The executor issues the
    /// backing at the first `Payload` operand, after any preceding operand.
    pub(crate) fn order(&self) -> [Role; 3] {
        self.order
    }
    pub(crate) fn vector(&self) -> &'a ResolvedBinding {
        self.vector
    }
    pub(crate) fn counter(&self) -> &'a ResolvedBinding {
        self.counter
    }
    pub(crate) fn item(&self) -> &'a ResolvedBinding {
        self.item
    }
    pub(crate) fn allocation(&self) -> &'a E {
        self.allocation
    }
    pub(crate) fn start(&self) -> &'a E {
        self.start
    }
    pub(crate) fn condition(&self) -> &'a E {
        self.condition
    }
    pub(crate) fn body(&self) -> &'a E {
        self.body
    }
    pub(crate) fn constructor(&self) -> &'a E {
        self.constructor
    }
    pub(crate) fn text(&self) -> &'a E {
        self.text
    }
    /// The single `core.bytes.zeroed` site whose executions the backing replaces.
    pub(crate) fn producer(&self) -> &'a E {
        self.producer
    }
    /// The exact publication site; each row publishes before the next issue.
    pub(crate) fn push(&self) -> &'a E {
        self.push
    }
    pub(crate) fn step(&self) -> &'a E {
        self.step
    }
    pub(crate) fn suffix(&self) -> &'a [S] {
        self.suffix
    }
    pub(crate) fn tail(&self) -> &'a E {
        self.tail
    }
    pub(crate) fn rows(&self) -> u32 {
        self.rows
    }
    pub(crate) fn width(&self) -> u64 {
        self.width
    }
    /// Checked `rows * width`; derivation already refused overflow.
    pub(crate) fn extent(&self) -> u64 {
        u64::from(self.rows) * self.width
    }
    /// Pointer identity only: an equal clone or decoded copy is a different program.
    pub(crate) fn still_held(&self, program: &ResolvedProgram) -> bool {
        std::ptr::eq(self.program, program)
    }
    /// Pointer-only loop discovery for a capacity walker. It grants nothing
    /// beyond the selection already derived.
    pub(crate) fn is_loop(&self, condition: &E, body: &E) -> bool {
        std::ptr::eq(self.condition, condition) && std::ptr::eq(self.body, body)
    }
    /// SHA-256 over the canonical cache encoding of the selected construction
    /// prefix and record. It identifies the original shape for replay checks;
    /// it is not an authority token and does not cover the suffix.
    pub(crate) fn shape_digest(&self) -> Option<[u8; 32]> {
        use sha2::{Digest, Sha256};
        const DOMAIN: &[u8] = b"semaprax.private.fused-construction-shape.v1\0";
        const LIMIT: usize = 64 * 1024;
        let mut hasher = Sha256::new();
        hasher.update(DOMAIN);
        hasher.update(self.function.id.as_str().as_bytes());
        hasher.update([0]);
        for part in [
            crate::cache_codec::encode_bounded(self.record, LIMIT).ok()?,
            crate::cache_codec::encode_bounded(self.vector, LIMIT).ok()?,
            crate::cache_codec::encode_bounded(self.counter, LIMIT).ok()?,
            crate::cache_codec::encode_bounded(self.allocation, LIMIT).ok()?,
            crate::cache_codec::encode_bounded(self.start, LIMIT).ok()?,
            crate::cache_codec::encode_bounded(self.condition, LIMIT).ok()?,
            crate::cache_codec::encode_bounded(self.body, LIMIT).ok()?,
        ] {
            hasher.update(u64::try_from(part.len()).ok()?.to_le_bytes());
            hasher.update(&part);
        }
        Some(hasher.finalize().into())
    }
}

/// Derive the selection from the current resolved HIR. Ordinary programs,
/// counts at most 32 and every forged typed fact refuse.
pub(crate) fn derive(program: &ResolvedProgram) -> Result<Selection<'_>, Refusal> {
    shape::derive(program)
}

fn typed(e: &E, ty: &T, mode: O) -> bool {
    e.ty == *ty && e.ownership == mode
}
fn place(e: &E, binding: &ResolvedBinding) -> bool {
    typed(e, &binding.ty, binding.ownership)
        && matches!(&e.kind, K::Place(p) if p.root == binding.id && p.projections.is_empty())
}
fn integer(e: &E) -> Option<i64> {
    match e.kind {
        K::Int(value) if typed(e, &T::I64, O::Value) => Some(value),
        _ => None,
    }
}
fn usize_literal(e: &E) -> Option<u64> {
    match e.kind {
        K::Usize(value) if typed(e, &T::Usize, O::Value) => Some(value),
        _ => None,
    }
}
fn builtin<'a>(e: &'a E, id: &str, types: &[T]) -> Option<&'a [E]> {
    match &e.kind {
        K::Call {
            callee,
            instance: None,
            type_arguments,
            args,
        } if callee.as_str() == id && type_arguments.as_slice() == types => Some(args),
        _ => None,
    }
}
fn distinct(ids: &[&ExpressionId]) -> bool {
    ids.iter()
        .enumerate()
        .all(|(index, id)| ids[..index].iter().all(|other| other != id))
}
fn binary(e: &E, op: BinaryOp) -> Option<(&E, &E)> {
    match &e.kind {
        K::Binary {
            op: actual,
            left,
            right,
        } if *actual == op => Some((left.as_ref(), right.as_ref())),
        _ => None,
    }
}
