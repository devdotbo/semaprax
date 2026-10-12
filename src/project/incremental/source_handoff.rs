//! Ownership-only handoff after the unchanged current-source cache guards.
use super::SemanticWorkspaceSource;

pub(super) fn consume(sources: Vec<SemanticWorkspaceSource>) -> Vec<SemanticWorkspaceSource> {
    sources
}

#[cfg(test)]
mod tests;
