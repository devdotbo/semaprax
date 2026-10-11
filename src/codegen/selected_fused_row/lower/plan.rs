//! Plan-driven ownership code. Liveness flags follow the canonical executor's
//! rules exactly (`cleanup_plan::execute`): Initialize requires dead
//! destinations; Transfer and Renew require live sources and dead
//! destinations; ReserveRenewal changes nothing; CallCommit consumes every
//! argument flag atomically; a finalizer acts only on a live guard. Each exit's
//! `finalize_in_order` is emitted verbatim against the physical carrier the
//! selected lowering registered for that exact plan storage.
use super::*;
use crate::cleanup_plan::{CleanupPlace, CleanupTransition, ExitTarget, StorageId};
use crate::hir::{DeclarationId, ExpressionId};
use std::fmt::Write as _;

/// Physical selected carrier backing one plan storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Physical {
    Rows(String),
    Staged(String),
    Field(String, Role),
}

fn flag(id: crate::cleanup::LivenessFlagId) -> String {
    format!("spx_flag_{}", id.0)
}

impl Lower<'_, '_> {
    pub(super) fn flag_declarations(&self) -> String {
        let mut out = String::new();
        for leaf in self.cleanup.leaves() {
            writeln!(out, "    bool {} = false;", flag(leaf.flag))
                .expect("writing to a string cannot fail");
        }
        out
    }
    pub(super) fn register(
        &mut self,
        storage: StorageId,
        physical: Physical,
    ) -> Result<(), Diagnostic> {
        match self.registry.iter().find(|(known, _)| *known == storage) {
            Some((_, known)) if *known == physical => Ok(()),
            Some(_) => Err(error("one plan storage maps to two selected carriers")),
            None => {
                self.registry.push((storage, physical));
                Ok(())
            }
        }
    }
    /// Apply every canonical transition keyed by `key`, exactly once.
    pub(super) fn apply(&mut self, key: &ExpressionId) -> Result<(), Diagnostic> {
        let cleanup = self.cleanup;
        let Some(transitions) = cleanup.transitions(key) else {
            return Ok(());
        };
        if self.applied.contains(key) {
            return Err(error("a canonical transition key is reached twice"));
        }
        self.applied.push(key.clone());
        for transition in transitions {
            match transition {
                CleanupTransition::ReserveRenewal { .. } => {}
                CleanupTransition::Initialize { destination, .. } => {
                    let flags = self.flags(destination)?;
                    self.guard_flags(&[], &flags);
                    self.set_flags(&[], &flags);
                }
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                }
                | CleanupTransition::Renew {
                    source,
                    destination,
                    ..
                } => {
                    let from = self.flags(source)?;
                    let to = self.flags(destination)?;
                    if from.len() != to.len() {
                        return Err(error("a canonical transfer has unequal leaf counts"));
                    }
                    self.guard_flags(&from, &to);
                    self.set_flags(&from, &to);
                }
                CleanupTransition::CallCommit { arguments, .. } => {
                    let mut consumed = Vec::new();
                    for argument in arguments {
                        for name in self.flags(&argument.source)? {
                            if consumed.contains(&name) {
                                return Err(error("a call consumes one flag twice"));
                            }
                            consumed.push(name);
                        }
                    }
                    self.guard_flags(&consumed, &[]);
                    self.set_flags(&consumed, &[]);
                }
                _ => return Err(error("a transition kind outside the selected domain")),
            }
        }
        Ok(())
    }
    fn flags(&self, place: &CleanupPlace) -> Result<Vec<String>, Diagnostic> {
        let leaves = self.cleanup.leaves_under(place);
        for leaf in &leaves {
            self.physical(&leaf.place, leaf.lifecycle)?;
        }
        Ok(leaves.iter().map(|leaf| flag(leaf.flag)).collect())
    }
    fn guard_flags(&mut self, live: &[String], dead: &[String]) {
        let mut tests: Vec<String> = live.iter().map(|name| format!("!{name}")).collect();
        tests.extend(dead.iter().cloned());
        if !tests.is_empty() {
            self.line(&format!(
                "if ({}) spx_runtime_invariant_failure(\"selected cleanup transition precondition\");",
                tests.join(" || ")
            ));
        }
    }
    fn set_flags(&mut self, cleared: &[String], set: &[String]) {
        for name in cleared {
            self.line(&format!("{name} = false;"));
        }
        for name in set {
            self.line(&format!("{name} = true;"));
        }
    }
    /// The physical drop for one plan leaf; refuses any unbound pairing.
    fn physical(
        &self,
        place: &CleanupPlace,
        lifecycle: &DeclarationId,
    ) -> Result<String, Diagnostic> {
        let (_, physical) = self
            .registry
            .iter()
            .find(|(storage, _)| *storage == place.storage)
            .ok_or_else(|| error("a canonical owner has no selected carrier"))?;
        let field = match (physical, place.projections.as_slice()) {
            (Physical::Rows(name), []) => {
                return if lifecycle.as_str() == crate::cleanup::VEC_DROP_LIFECYCLE_ID {
                    Ok(format!(
                        "if (!spx_fused_rows_drop(spx_domain, &{name}, spx_fused_order, spx_string_drop)) spx_runtime_invariant_failure(\"selected Vec finalizer\");"
                    ))
                } else {
                    Err(error("a row Vec owner has a foreign lifecycle"))
                };
            }
            (Physical::Field(name, role), []) => (name.clone(), *role),
            (Physical::Staged(name), [projection]) => (name.clone(), self.role_of(projection)?),
            _ => {
                return Err(error(
                    "a canonical owner projection has no selected carrier",
                ))
            }
        };
        match (field.1, lifecycle.as_str()) {
            (Role::Text, crate::cleanup::STRING_DROP_LIFECYCLE_ID) => Ok(format!(
                "if (!{0}.string) spx_runtime_invariant_failure(\"selected String finalizer\"); spx_string_drop({0}.string); {0}.string = NULL;",
                field.0
            )),
            (Role::Payload, crate::cleanup::BYTES_DROP_LIFECYCLE_ID) => Ok(format!(
                "if (!spx_fused_drop(spx_domain, &{}.bytes)) spx_runtime_invariant_failure(\"selected Bytes finalizer\");",
                field.0
            )),
            _ => Err(error("a staged row owner has a foreign lifecycle")),
        }
    }
    fn role_of(&self, field: &DeclarationId) -> Result<Role, Diagnostic> {
        let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &self.sites.record.kind
        else {
            return Err(error("selected record declaration is not a record"));
        };
        fields
            .iter()
            .zip(self.order)
            .find(|(declared, _)| &declared.id == field)
            .map(|(_, role)| role)
            .ok_or_else(|| error("a canonical projection names a foreign field"))
    }
    /// One exit's finalizers, verbatim and guarded.
    pub(super) fn finalizers(&self, exit: &ExitTarget) -> Result<String, Diagnostic> {
        let mut out = String::new();
        for action in &exit.finalize_in_order {
            let leaf = self
                .cleanup
                .leaf(action.guard_flag)
                .ok_or_else(|| error("a finalizer references an unknown guard"))?;
            if leaf.place != action.source || leaf.lifecycle != &action.lifecycle_id {
                return Err(error("a finalizer disagrees with its cleanup leaf"));
            }
            let name = flag(action.guard_flag);
            let drop = self.physical(&action.source, &action.lifecycle_id)?;
            writeln!(out, "    if ({name}) {{ {name} = false; {drop} }}")
                .expect("writing to a string cannot fail");
        }
        Ok(out)
    }
    pub(super) fn all_dead(&self) -> String {
        let names: Vec<String> = self
            .cleanup
            .leaves()
            .iter()
            .map(|leaf| flag(leaf.flag))
            .collect();
        if names.is_empty() {
            return String::new();
        }
        format!(
            "    if ({}) spx_runtime_invariant_failure(\"selected exit left a live owner\");\n",
            names.join(" || ")
        )
    }
    /// The plan's failure exit for an operation status at `expression`.
    pub(super) fn fail(
        &mut self,
        expression: &ExpressionId,
        status: &str,
    ) -> Result<String, Diagnostic> {
        let exit = self
            .cleanup
            .failure(expression)
            .ok_or_else(|| error("an ordinary failure has no canonical status source"))?;
        if !self.failures_used.contains(&exit.id.0) {
            self.failures_used.push(exit.id.0);
        }
        Ok(format!("{{ {status} goto spx_exit_{}; }}", exit.id.0))
    }
    /// Normal scope end of the region owning this block's direct owners.
    pub(super) fn place_region(&mut self, block: &E) -> Result<(), Diagnostic> {
        let anchors = self.anchors(block);
        let Some(exit) = self.cleanup.region_exit(&anchors)? else {
            return Ok(());
        };
        if exit.id == self.cleanup.success().id || self.placed.contains(&exit.id.0) {
            return Ok(());
        }
        self.placed.push(exit.id.0);
        let code = self.finalizers(exit)?;
        for line in code.lines() {
            self.line(line.trim_start());
        }
        Ok(())
    }
    /// Owning storages of a block's own region: owned `let` bindings and the
    /// temporaries of its statement values and tail, not entering nested
    /// blocks or loops (which own their regions).
    fn anchors(&self, block: &E) -> Vec<StorageId> {
        let K::Block { statements, tail } = &block.kind else {
            return Vec::new();
        };
        let mut pending: Vec<&E> = vec![tail];
        let mut anchors = Vec::new();
        let keep = |storage: StorageId, anchors: &mut Vec<StorageId>| {
            if self
                .cleanup
                .leaves()
                .iter()
                .any(|leaf| leaf.place.storage == storage)
                && !anchors.contains(&storage)
            {
                anchors.push(storage);
            }
        };
        for statement in statements {
            match statement {
                S::Let { binding, value, .. } => {
                    keep(StorageId::Value(binding.id.clone()), &mut anchors);
                    pending.push(value);
                }
                S::Assign { value, .. } => pending.push(value),
                _ => {}
            }
        }
        while let Some(expression) = pending.pop() {
            keep(StorageId::Temporary(expression.id.clone()), &mut anchors);
            if !matches!(expression.kind, K::Block { .. }) {
                crate::hir::push_resolved_expression_children_in_authored_order(
                    expression,
                    &mut pending,
                );
            }
        }
        anchors
    }
    /// Every keyed transition reached, every failure exit selected, and every
    /// finalizing exit placed. Otherwise the order is not proved.
    pub(super) fn complete(&self) -> Result<(), Diagnostic> {
        if self.cleanup.keys().any(|key| !self.applied.contains(key)) {
            return Err(error(
                "cleanup order cannot be proved: a canonical transition is never reached",
            ));
        }
        if self
            .cleanup
            .failures()
            .any(|exit| !self.failures_used.contains(&exit.id.0))
        {
            return Err(error(
                "cleanup order cannot be proved: an ordinary failure is not preserved",
            ));
        }
        let success = self.cleanup.success().id.0;
        for exit in &self.cleanup.plan().exits {
            let id = exit.id.0;
            if !exit.finalize_in_order.is_empty()
                && id != success
                && !self.placed.contains(&id)
                && !self.failures_used.contains(&id)
            {
                return Err(error(
                    "cleanup order cannot be proved: a finalizing exit has no selected position",
                ));
            }
        }
        Ok(())
    }
}
