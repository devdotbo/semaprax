//! Receiver inputs. The lowering consumes only [`Sites`]: borrowed nodes of
//! one held typed program plus producer-bound view carrier facts.
//!
//! The concrete immutable `ValidatedFusedSelection` is the accepting owner;
//! no caller-implemented trait grants emission authority. The receiver borrows
//! a fresh source/HIR join over that full owner and rederives carrier facts.
//! The default compiler does not select this private entry. The old
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
    pub(super) fn from_joined(selection: &'a crate::hir::fused_construction::Selection<'a>, carriers: &'a dyn CarrierFacts) -> Self {
        Self {
            program: selection.program(), function: selection.function(), record: selection.record(),
            vector: selection.vector(), counter: selection.counter(), item: selection.item(),
            allocation: selection.allocation(), condition: selection.condition(), body: selection.body(),
            constructor: selection.constructor(), producer: selection.producer(), push: selection.push(),
            step: selection.step(), suffix: selection.suffix(), tail: selection.tail(),
            rows: selection.rows(), width: selection.width(), carriers,
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

impl CarrierFacts for crate::construction_fusion::native_provenance::Mapping<'_> {
    fn carrier(&self, expression: &E) -> Option<Carrier> {
        self.fact(expression).map(|fact| fact.carrier())
    }
}
