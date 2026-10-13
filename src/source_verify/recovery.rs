//! Lexical recovery for declarations whose initializer produced no checked value.
//!
//! Names borrow immutable authored source. This directory never supplies a type,
//! binding, capture, ownership state, or backend admission. Ordinary source
//! verifier storage has no physical payer; these error-only allocations remain
//! ordinary storage, not paid replay authority.

use std::collections::BTreeMap;

use crate::ast::Function;
use crate::diagnostic::Diagnostic;

#[derive(Default)]
struct Scope<'p> {
    parent: Option<usize>,
    names: BTreeMap<&'p str, (usize, bool)>,
}

#[derive(Clone, Copy)]
pub(super) struct Checkpoint {
    pub(super) diagnostics: usize,
    reads: usize,
}

#[derive(Default)]
pub(super) struct Recovery<'p> {
    scopes: Vec<Scope<'p>>,
    last_read: Option<usize>,
    reads: usize,
    #[cfg(test)]
    pub(super) visited: usize,
}

impl<'p> Recovery<'p> {
    /// Recheck each affected declaration once in this invocation, only after an
    /// existing unknown-type refusal. Equal byte spans alone are insufficient
    /// for externally constructed source trees or duplicate parameters.
    pub(super) fn signature(
        program: &'p dyn crate::ast::program_source::ProgramSource,
        current: &'p Function,
        variables: &std::collections::HashMap<String, super::binding::Binding>,
        types: &super::type_table::TypeTable<'_>,
        diagnostics: &[Diagnostic],
    ) -> Self {
        let mut recovery = Self::default();
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T001")
        {
            return recovery;
        }
        let mut parameters = None;
        let mut scratch = Vec::new();
        for parameter in &current.params {
            if types.needs_drop(&parameter.ty)
                || variables.get(&parameter.name).is_none_or(|binding| {
                    binding.ty != parameter.ty || binding.mode != parameter.mode
                })
                || !diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "SPX-T001" && diagnostic.span == Some(parameter.span)
                })
            {
                continue;
            }
            let parameters = parameters.get_or_insert_with(|| {
                current
                    .type_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect()
            });
            scratch.clear();
            super::declared_type::check_declared_type(
                program,
                &parameter.ty,
                parameter.span,
                types,
                parameters,
                &mut scratch,
            );
            if let Some(origin) = scratch
                .iter()
                .filter(|refusal| refusal.code == "SPX-T001")
                .find_map(|refusal| {
                    diagnostics.iter().position(|original| {
                        original.code == refusal.code
                            && original.severity == refusal.severity
                            && original.message == refusal.message
                            && original.path == refusal.path
                            && original.span == refusal.span
                            && original.help == refusal.help
                    })
                })
            {
                recovery.ensure(0);
                recovery.scopes[0]
                    .names
                    .insert(parameter.name.as_str(), (origin, true));
            }
        }
        recovery
    }

    pub(super) fn clear(&mut self, scope: usize, name: &str) {
        if let Some(row) = self.scopes.get_mut(scope) {
            row.names.remove(name);
        }
    }

    /// Scope ordinals are reused after a branch or block ends. Reset the row
    /// before entering that ordinal; a former sibling must never be an ancestor.
    /// Before the first refused declaration there are no inherited facts, so
    /// successful programs do not acquire a directory merely to fork scopes.
    pub(super) fn fork(&mut self, parent: Option<usize>, child: usize) {
        if self.scopes.is_empty() {
            return;
        }
        self.ensure(child);
        self.scopes[child] = Scope {
            parent,
            names: BTreeMap::new(),
        };
    }

    fn ensure(&mut self, scope: usize) {
        if self.scopes.len() <= scope {
            self.scopes.resize_with(scope + 1, Scope::default);
        }
    }

    /// Only an absent checked binding/function reaches this lookup. Existing
    /// availability, loan, move, and function-value diagnostics run first.
    pub(super) fn reads_refused(&mut self, scope: usize, name: &str) -> bool {
        self.read(scope, name, false)
    }

    pub(super) fn reads_known_refused(&mut self, scope: usize, name: &str) -> bool {
        self.read(scope, name, true)
    }

    fn read(&mut self, mut scope: usize, name: &str, known: bool) -> bool {
        while let Some(row) = self.scopes.get(scope) {
            #[cfg(test)]
            {
                self.visited += 1;
            }
            if let Some(&(origin, declaration_known)) = row.names.get(name) {
                // A missing declaration never outranks a real function value
                // or checked binding. Known markers are inference-only.
                if declaration_known != known {
                    return false;
                }
                self.last_read = Some(origin);
                self.reads += 1;
                return true;
            }
            let Some(parent) = row.parent else {
                break;
            };
            scope = parent;
        }
        false
    }

    pub(super) fn checkpoint(&self, diagnostics: &[Diagnostic]) -> Checkpoint {
        Checkpoint {
            diagnostics: diagnostics.len(),
            reads: self.reads,
        }
    }

    /// Publish only after the complete initializer. A continuation captures the
    /// diagnostic/read frontier before evaluation, including nested blocks;
    /// source spans or old unrelated diagnostics cannot manufacture this fact.
    pub(super) fn refused_let(
        &mut self,
        scope: usize,
        name: &'p str,
        checkpoint: Checkpoint,
        diagnostics: &[Diagnostic],
    ) {
        self.publish(scope, name, checkpoint, diagnostics, false);
    }

    pub(super) fn refused_annotation(
        &mut self,
        scope: usize,
        name: &'p str,
        checkpoint: Checkpoint,
        diagnostics: &[Diagnostic],
    ) {
        self.publish(scope, name, checkpoint, diagnostics, true);
    }

    fn publish(
        &mut self,
        scope: usize,
        name: &'p str,
        checkpoint: Checkpoint,
        diagnostics: &[Diagnostic],
        known: bool,
    ) {
        let origin = diagnostics
            .iter()
            .enumerate()
            .skip(checkpoint.diagnostics)
            .find(|(_, diagnostic)| diagnostic.severity.is_error())
            .map(|(index, _)| index)
            .or_else(|| {
                (self.reads > checkpoint.reads)
                    .then_some(self.last_read)
                    .flatten()
            });
        if let Some(origin) = origin {
            self.ensure(scope);
            self.scopes[scope].names.insert(name, (origin, known));
        }
    }
}
