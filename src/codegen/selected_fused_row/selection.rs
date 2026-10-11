//! Receiver inputs. The lowering consumes only [`Sites`]: borrowed nodes of
//! one held typed program plus producer-bound view carrier facts.
//!
//! [`ValidatedOwner`] is the explicit accepting API for the full validated
//! selected owner: the independent `source_verify::fused_construction`
//! `SourceProof` joined by `hir::fused_construction::join` with the current
//! HIR, under complete selected validation. Its accessor names follow that
//! join's `Selection`. Nothing in this tree implements it yet; until the
//! validated type lands the receiver has no production caller. The old
//! `construction_fusion` descriptor reaches the lowering only through the
//! test-only unvalidated precursor and confers no artifact authority.
use crate::construction_fusion::native_provenance::Carrier;
use crate::hir::{
    ResolvedBinding, ResolvedExpr as E, ResolvedFunction, ResolvedProgram, ResolvedStatement as S,
    ResolvedTypeDeclaration,
};

/// Producer-bound representation of each view-valued expression, keyed by the
/// exact borrowed expression node. Never answered from `ResolvedType` alone.
pub(crate) trait CarrierFacts {
    fn carrier(&self, expression: &E) -> Option<Carrier>;
}

/// Accepting API for the validated selected owner. Implementations must only
/// wrap a value produced by the full validated join; equal clones, decoded
/// programs, rescaled counts or cached facts are never implementations.
pub(crate) trait ValidatedOwner<'a> {
    /// Both joined halves are still exactly the supplied objects.
    fn still_current(&self, source: &crate::ast::Program, program: &ResolvedProgram) -> bool;
    fn program(&self) -> &'a ResolvedProgram;
    fn function(&self) -> &'a ResolvedFunction;
    fn record(&self) -> &'a ResolvedTypeDeclaration;
    fn vector(&self) -> &'a ResolvedBinding;
    fn counter(&self) -> &'a ResolvedBinding;
    fn item(&self) -> &'a ResolvedBinding;
    fn allocation(&self) -> &'a E;
    fn condition(&self) -> &'a E;
    fn body(&self) -> &'a E;
    fn constructor(&self) -> &'a E;
    /// The single `core.bytes.zeroed` site whose executions the backing replaces.
    fn producer(&self) -> &'a E;
    fn push(&self) -> &'a E;
    fn step(&self) -> &'a E;
    fn suffix(&self) -> &'a [S];
    fn tail(&self) -> &'a E;
    fn rows(&self) -> u32;
    fn width(&self) -> u64;
    /// Carrier facts rederived for this joined selection's suffix.
    fn carriers(&self) -> &dyn CarrierFacts;
}

/// Borrowed sites of one selection. The count and width are the selection's
/// own; nothing here rescales them.
pub(super) struct Sites<'a> {
    pub(super) program: &'a ResolvedProgram,
    pub(super) function: &'a ResolvedFunction,
    pub(super) record: &'a ResolvedTypeDeclaration,
    pub(super) vector: &'a ResolvedBinding,
    pub(super) counter: &'a ResolvedBinding,
    pub(super) item: &'a ResolvedBinding,
    pub(super) allocation: &'a E,
    pub(super) condition: &'a E,
    pub(super) body: &'a E,
    pub(super) constructor: &'a E,
    pub(super) producer: &'a E,
    pub(super) push: &'a E,
    pub(super) step: &'a E,
    pub(super) suffix: &'a [S],
    pub(super) tail: &'a E,
    pub(super) rows: u32,
    pub(super) width: u64,
    pub(super) carriers: &'a dyn CarrierFacts,
}

impl<'a> Sites<'a> {
    pub(super) fn from_validated<O: ValidatedOwner<'a>>(owner: &'a O) -> Self {
        Self {
            program: owner.program(),
            function: owner.function(),
            record: owner.record(),
            vector: owner.vector(),
            counter: owner.counter(),
            item: owner.item(),
            allocation: owner.allocation(),
            condition: owner.condition(),
            body: owner.body(),
            constructor: owner.constructor(),
            producer: owner.producer(),
            push: owner.push(),
            step: owner.step(),
            suffix: owner.suffix(),
            tail: owner.tail(),
            rows: owner.rows(),
            width: owner.width(),
            carriers: owner.carriers(),
        }
    }
    /// The selected function must be a member of the held program.
    pub(super) fn held(&self) -> bool {
        self.program
            .functions
            .iter()
            .any(|function| std::ptr::eq(function, self.function))
    }
    pub(super) fn carrier(&self, expression: &E) -> Option<Carrier> {
        self.carriers.carrier(expression)
    }
}
