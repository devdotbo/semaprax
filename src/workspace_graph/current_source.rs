// src/workspace_graph/current_source.rs
//! Immutable current source and AST owners, held before dependency pruning.
//!
//! Source origin facts live in `SourceMetadata`, which holds the complete
//! `WorkspaceSource` and never points at an AST. `CurrentParsedSource` pairs
//! one metadata `Arc` with one authored `Program` and the optional original
//! function ordinals, so file facts cannot retain obsolete AST generations.
//!
//! Every constructor is prepaid by the caller's current canonical payer. The
//! `reserve` callback is that payer, not a new quota: it is invoked once with
//! the actual `Arc` carrier bytes before `Arc::new`. This module neither
//! credits nor refunds charges, and it neither infers verdicts nor clones
//! `Program`. Heap bytes owned by the `WorkspaceSource` strings, the
//! `Program` and the ordinal vector are charged by their original owners, not
//! here.
use std::mem::size_of;
use std::sync::Arc;

use super::WorkspaceSource;
pub(crate) type ParsedSources = Arc<Vec<Arc<CurrentParsedSource>>>;
pub(crate) type SourceFacts = Vec<Arc<SourceMetadata>>;
use crate::ast::Program;

/// Mirrors the `Arc` allocation: two counters followed by the value.
#[repr(C)]
struct ArcCarrier<T> {
    _strong: usize,
    _weak: usize,
    _value: T,
}

/// Actual requested bytes of the `Arc` carrier allocation for `T`.
pub(crate) const fn arc_carrier_bytes<T>() -> usize {
    size_of::<ArcCarrier<T>>()
}

/// Source origin facts. The complete existing `WorkspaceSource` is stored, so
/// every present or later-added field is preserved without being listed here.
pub(crate) struct SourceMetadata {
    input: WorkspaceSource,
}

impl SourceMetadata {
    /// Reserves the actual carrier bytes, then moves `input` into a new `Arc`.
    /// On reserve failure no `Arc` is created and the error is returned as is.
    pub(crate) fn new<E, F>(input: WorkspaceSource, reserve: &mut F) -> Result<Arc<Self>, E>
    where
        F: FnMut(usize) -> Result<(), E>,
    {
        reserve(arc_carrier_bytes::<Self>())?;
        Ok(Arc::new(Self { input }))
    }

    /// The complete original source value.
    pub(crate) fn input(&self) -> &WorkspaceSource {
        &self.input
    }
}

/// Failure of a parsed-owner constructor.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CurrentParsedSourceError<E> {
    /// The caller's payer refused the carrier bytes.
    Reserve(E),
    /// Original invariant failure: the program path differs from the metadata
    /// path. Detected before any reservation is attempted.
    PathInconsistent,
}

/// One authored `Program` associated with its source metadata.
pub(crate) struct CurrentParsedSource {
    metadata: Arc<SourceMetadata>,
    program: Program,
    ordinals: Option<Vec<usize>>,
}

impl CurrentParsedSource {
    /// Checks the original path invariant, reserves the actual carrier bytes,
    /// then moves `metadata`, `program` and `ordinals` (no clone) into a new
    /// `Arc`. `ordinals` maps each pruned-order function to its original
    /// position and is `None` for a module with nothing pruned.
    pub(crate) fn new<E, F>(
        metadata: Arc<SourceMetadata>,
        program: Program,
        ordinals: Option<Vec<usize>>,
        reserve: &mut F,
    ) -> Result<Arc<Self>, CurrentParsedSourceError<E>>
    where
        F: FnMut(usize) -> Result<(), E>,
    {
        if metadata.input().path != program.path {
            return Err(CurrentParsedSourceError::PathInconsistent);
        }
        reserve(arc_carrier_bytes::<Self>()).map_err(CurrentParsedSourceError::Reserve)?;
        Ok(Arc::new(Self {
            metadata,
            program,
            ordinals,
        }))
    }

    /// The exact stored metadata `Arc`.
    pub(crate) fn metadata(&self) -> &Arc<SourceMetadata> {
        &self.metadata
    }

    pub(crate) fn input(&self) -> &WorkspaceSource {
        self.metadata.input()
    }

    pub(crate) fn program(&self) -> &Program {
        &self.program
    }

    /// Original function positions, present only when functions were pruned.
    pub(crate) fn original_ordinals(&self) -> Option<&[usize]> {
        self.ordinals.as_deref()
    }

    /// Strong association: true only for the same actual metadata allocation.
    /// Equal source or AST values are never authority.
    pub(crate) fn is_associated_with(&self, metadata: &Arc<SourceMetadata>) -> bool {
        Arc::ptr_eq(&self.metadata, metadata)
    }
}

mod source_bytes;
pub(crate) use source_bytes::SourceBytes;

impl std::ops::Deref for SourceMetadata {
    type Target = WorkspaceSource;
    fn deref(&self) -> &WorkspaceSource {
        &self.input
    }
}
