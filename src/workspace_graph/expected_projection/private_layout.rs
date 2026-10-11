//! Invocation-local private layout closure; never a source import or graph edge.
use super::super::{graph_error, reserve_builder_structure, AuthoredDeclaration};
use crate::ast::{ModuleUseKind, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::collections::BTreeMap;
use std::fmt::Write;
#[path = "private_layout/names.rs"]
mod names;
#[path = "private_layout/shape.rs"]
mod shape;
use shape::{debit, explicit_only, scalar, validate_declaration, validate_root, Absence};

pub(super) struct Entry<'a> {
    pub(super) module: &'a str,
    pub(super) declaration: &'a TypeDeclaration,
    pub(super) alias: String,
}
pub(in crate::workspace_graph) struct LayoutClosure<'a> {
    pub(super) entries: Vec<Entry<'a>>,
}
struct Frame<'a> {
    module: &'a str,
    declaration: &'a TypeDeclaration,
    next: usize,
}
const DEPTH: usize = crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH;
const WORK: usize = 1_048_576;

impl<'a> LayoutClosure<'a> {
    pub(in crate::workspace_graph) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn derive(
        caller: &Program,
        authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut remaining = WORK;
        if explicit_only(caller, authored, programs, &mut remaining)? != Absence::Possible {
            return Ok(Self {
                entries: Vec::new(),
            });
        }
        // Borrowed root traversal precedes naming. Explicit-only programs do
        // not acquire the new identifier census or private String backing.
        reserve_builder_structure(DEPTH * std::mem::size_of::<Frame<'_>>())?;
        let mut frames = Vec::with_capacity(DEPTH);
        if frames.capacity() != DEPTH {
            return Err(refusal());
        }
        let mut count = 0usize;
        visit(
            caller,
            authored,
            programs,
            &mut frames,
            &mut remaining,
            |_, _, _| {
                count = count.checked_add(1usize).ok_or_else(refusal)?;
                Ok(())
            },
        )?;
        if count == 0 {
            return Ok(Self {
                entries: Vec::new(),
            });
        }
        let mut ordinal = names::first(caller)?;
        debit(&mut remaining, programs.len())?;
        let capacity = programs
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
            .ok_or_else(refusal)?;
        reserve_builder_structure(
            capacity
                .checked_mul(std::mem::size_of::<Entry<'_>>())
                .ok_or_else(refusal)?,
        )?;
        let mut entries: Vec<Entry<'a>> = Vec::with_capacity(capacity);
        if frames.capacity() != DEPTH || entries.capacity() != capacity {
            return Err(refusal());
        }
        visit(
            caller,
            authored,
            programs,
            &mut frames,
            &mut remaining,
            |module, declaration, remaining| {
                let scan = entries
                    .iter()
                    .try_fold(0usize, |n, e| {
                        n.checked_add(
                            e.declaration
                                .stable_id
                                .len()
                                .checked_add(declaration.stable_id.len())?,
                        )
                    })
                    .ok_or_else(refusal)?;
                *remaining = remaining.checked_sub(scan).ok_or_else(refusal)?;
                if entries
                    .iter()
                    .any(|e| e.declaration.stable_id == declaration.stable_id)
                {
                    return Ok(());
                }
                if entries.len() == capacity {
                    return Err(refusal());
                }
                let alias = private_name(ordinal)?;
                ordinal = ordinal.checked_add(1).ok_or_else(refusal)?;
                entries.push(Entry {
                    module,
                    declaration,
                    alias,
                });
                Ok(())
            },
        )?;
        remaining = remaining
            .checked_sub(
                entries
                    .len()
                    .checked_mul(entries.len())
                    .and_then(|n| {
                        n.checked_mul(
                            entries
                                .iter()
                                .map(|e| e.declaration.stable_id.len())
                                .max()
                                .unwrap_or(0)
                                .checked_add(1)?,
                        )
                    })
                    .ok_or_else(refusal)?,
            )
            .ok_or_else(refusal)?;
        let _ = remaining;
        entries.sort_unstable_by(|a, b| a.declaration.stable_id.cmp(&b.declaration.stable_id));
        Ok(Self { entries })
    }
    pub(super) fn add_costs(
        &self,
        caller: &Program,
        programs: &[Program],
        raw: &mut super::StructuralCost,
        runtime: &mut super::StructuralCost,
        identities: &mut usize,
    ) -> Result<(), Vec<Diagnostic>> {
        for e in &self.entries {
            super::ast_type_declaration_cost(e.declaration, raw)?;
            *identities = super::checked_builder_sum(
                *identities,
                super::identity_slots::ast_type_declaration_identity_slots(e.declaration)?,
            )?;
            runtime.string(&e.alias)?;
            super::super::generic_type_import::rewrite_declaration_runtime_cost_with_layout(
                e.declaration,
                e.module,
                caller,
                programs,
                runtime,
                Some(self),
            )?;
        }
        Ok(())
    }
    pub(super) fn append_types(
        &self,
        synthetic: &mut Program,
        caller: &Program,
        programs: &[Program],
    ) -> Result<(), Vec<Diagnostic>> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let length = synthetic
            .types
            .len()
            .checked_add(self.entries.len())
            .ok_or_else(refusal)?;
        if length > synthetic.types.capacity() {
            // Charge the full replacement while the cloned original directory
            // remains live, not only the added declaration count.
            reserve_builder_structure(
                length
                    .checked_mul(std::mem::size_of::<TypeDeclaration>())
                    .ok_or_else(refusal)?,
            )?;
            synthetic
                .types
                .try_reserve_exact(length - synthetic.types.len())
                .map_err(|_| refusal())?;
            if synthetic.types.capacity() != length {
                return Err(refusal());
            }
        }
        for e in &self.entries {
            let mut declaration = e.declaration.clone();
            declaration.name = crate::bounded_output::budgeted_clone(&e.alias);
            super::super::generic_type_import::rewrite_declaration_with_layout(
                &mut declaration,
                e.module,
                caller,
                programs,
                Some(self),
            )?;
            synthetic.types.push(declaration);
        }
        Ok(())
    }
    pub(super) fn alias<'b>(&'b self, caller: &'b Program, id: &str) -> Option<&'b str> {
        caller
            .module_uses
            .iter()
            .find(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id)
            .map(|u| u.alias.as_str())
            .or_else(|| {
                caller
                    .types
                    .iter()
                    .find(|t| t.stable_id == id)
                    .map(|t| t.name.as_str())
            })
            .or_else(|| {
                self.entries
                    .binary_search_by(|e| e.declaration.stable_id.as_str().cmp(id))
                    .ok()
                    .map(|index| self.entries[index].alias.as_str())
            })
    }
}
fn private_name(ordinal: u64) -> Result<String, Vec<Diagnostic>> {
    let mut digits = 1;
    let mut number = ordinal;
    while number >= 10 {
        digits += 1;
        number /= 10;
    }
    let bytes = "_spx_layout_".len() + digits;
    reserve_builder_structure(bytes)?;
    let mut name = String::new();
    name.try_reserve_exact(bytes).map_err(|_| refusal())?;
    if name.capacity() != bytes {
        return Err(refusal());
    }
    write!(&mut name, "_spx_layout_{ordinal}").map_err(|_| refusal())?;
    Ok(name)
}

fn refusal() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G172",
        "private layout closure exceeds its independently bounded nominal profile",
    )]
}

fn visit<'a>(
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    frames: &mut Vec<Frame<'a>>,
    remaining: &mut usize,
    mut found: impl FnMut(&'a str, &'a TypeDeclaration, &mut usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    debit(remaining, caller.module_uses.len())?;
    for imported in caller
        .module_uses
        .iter()
        .filter(|u| u.kind == ModuleUseKind::Type)
    {
        *remaining = remaining.checked_sub(1).ok_or_else(refusal)?;
        let Some(target) = authored.get(imported.persistent_id.as_str()) else {
            return Err(refusal());
        };
        let root = target.ty.ok_or_else(refusal)?;
        if imported.target_module != target.module || !target.explicit {
            return Err(refusal());
        }
        if !needs_private(
            caller,
            target.module,
            root,
            authored,
            programs,
            frames,
            remaining,
        )? {
            continue;
        }
        let outcome = matches!(root.kind, TypeDeclarationKind::Variant { .. });
        validate_root(root, target.module, authored, programs, remaining)?;
        let mut fields = 0usize;
        let mut leaves = 0usize;
        validate_declaration(root, &mut fields, remaining)?;
        frames.push(Frame {
            module: target.module,
            declaration: root,
            next: 0,
        });
        while let Some(frame) = frames.last_mut() {
            *remaining = remaining.checked_sub(1).ok_or_else(refusal)?;
            if let TypeDeclarationKind::Variant { cases } = &frame.declaration.kind {
                debit(
                    remaining,
                    cases.iter().map(|c| c.fields.len()).sum::<usize>(),
                )?;
            }
            let child = field_type(frame.declaration, frame.next);
            frame.next += 1;
            let module = frame.module;
            let Some(ty) = child else {
                frames.pop();
                continue;
            };
            if scalar(ty) {
                continue;
            }
            if matches!(ty, Type::String | Type::Bytes)
                || (!outcome && crate::map_ops::ast_collection(ty))
            {
                leaves = leaves.checked_add(1).ok_or_else(refusal)?;
                if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                    return Err(refusal());
                }
                continue;
            }
            let Type::Named { name, arguments } = ty else {
                return Err(refusal());
            };
            let id = identity(module, name, programs, remaining)?;
            if id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec") {
                debit(remaining, caller.types.len() + caller.module_uses.len())?;
                if caller.types.iter().any(|t| t.name == "Vec")
                    || caller.module_uses.iter().any(|u| {
                        u.kind == ModuleUseKind::Type
                            && u.alias == "Vec"
                            && u.persistent_id != crate::prelude::VEC_ID
                    })
                {
                    return Err(refusal());
                }
                leaves = leaves.checked_add(1).ok_or_else(refusal)?;
                if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                    return Err(refusal());
                }
                let [element] = arguments.as_slice() else {
                    return Err(refusal());
                };
                if crate::vec_ops::ast_vec_element_is_admitted(element) || *element == Type::String
                {
                    continue;
                }
                let Type::Named { name, arguments } = element else {
                    return Err(refusal());
                };
                if !arguments.is_empty() {
                    return Err(refusal());
                }
                let id = identity(module, name, programs, remaining)?.ok_or_else(refusal)?;
                let target = authored.get(id).ok_or_else(refusal)?;
                let declaration = target.ty.ok_or_else(refusal)?;
                debit(
                    remaining,
                    match &declaration.kind {
                        TypeDeclarationKind::Record { fields } => fields.len() * 2,
                        _ => 1,
                    },
                )?;
                if !target.explicit
                    || !declaration.explicit_id
                    || declaration.extends.is_some()
                    || !super::type_rewrite::record_element(declaration)
                {
                    return Err(refusal());
                }
                if private(caller, target.module, id, remaining)? {
                    found(target.module, declaration, remaining)?;
                }
                continue;
            }
            if !arguments.is_empty() {
                return Err(refusal());
            }
            let id = id.ok_or_else(refusal)?;
            let target = authored.get(id).ok_or_else(refusal)?;
            let declaration = target.ty.ok_or_else(refusal)?;
            debit(remaining, frames.len())?;
            if !target.explicit
                || !declaration.explicit_id
                || !declaration.type_parameters.is_empty()
                || !declaration.invariants().is_empty()
                || declaration.extends.is_some()
                || !matches!(declaration.kind, TypeDeclarationKind::Record { .. })
                || frames.iter().any(|f| f.declaration.stable_id == id)
                || frames.len() == DEPTH
            {
                return Err(refusal());
            }
            if private(caller, target.module, id, remaining)? {
                found(target.module, declaration, remaining)?;
            }
            validate_declaration(declaration, &mut fields, remaining)?;
            frames.push(Frame {
                module: target.module,
                declaration,
                next: 0,
            });
        }
    }
    Ok(())
}
// Selection only: traverse borrowed declaration references to determine whether
// this particular root needs inference. Full shape validation follows separately.
fn needs_private<'a>(
    caller: &Program,
    module: &'a str,
    root: &'a TypeDeclaration,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    frames: &mut Vec<Frame<'a>>,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    frames.clear();
    frames.push(Frame {
        module,
        declaration: root,
        next: 0,
    });
    while let Some(frame) = frames.last_mut() {
        debit(work, 1)?;
        if let TypeDeclarationKind::Variant { cases } = &frame.declaration.kind {
            debit(work, cases.iter().map(|c| c.fields.len()).sum())?;
        }
        let child = field_type(frame.declaration, frame.next);
        frame.next += 1;
        let module = frame.module;
        let Some(Type::Named { name, arguments }) = child else {
            if child.is_none() {
                frames.pop();
            }
            continue;
        };
        let id = identity(module, name, programs, work)?;
        let (name, arguments) =
            if id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec") {
                let [Type::Named { name, arguments }] = arguments.as_slice() else {
                    continue;
                };
                (name, arguments)
            } else {
                (name, arguments)
            };
        if !arguments.is_empty() {
            continue;
        }
        let Some(id) = identity(module, name, programs, work)? else {
            continue;
        };
        let target = authored.get(id).ok_or_else(refusal)?;
        if private(caller, target.module, id, work)? {
            frames.clear();
            return Ok(true);
        }
        let Some(declaration) = target.ty else {
            continue;
        };
        debit(work, frames.len())?;
        if frames.iter().any(|f| f.declaration.stable_id == id) {
            continue;
        }
        if frames.len() == DEPTH {
            return Err(refusal());
        }
        frames.push(Frame {
            module: target.module,
            declaration,
            next: 0,
        });
    }
    Ok(false)
}

fn private(
    caller: &Program,
    module: &str,
    id: &str,
    remaining: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    *remaining = remaining
        .checked_sub(caller.module_uses.len() + 1)
        .ok_or_else(refusal)?;
    Ok(private_without_work(caller, module, id))
}
fn private_without_work(caller: &Program, module: &str, id: &str) -> bool {
    caller.module != module
        && !caller
            .module_uses
            .iter()
            .any(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id)
}
fn field_type(declaration: &TypeDeclaration, index: usize) -> Option<&Type> {
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } => fields.get(index).map(|f| &f.ty),
        TypeDeclarationKind::Variant { cases } => cases
            .iter()
            .flat_map(|c| &c.fields)
            .nth(index)
            .map(|f| &f.ty),
        TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => None,
    }
}
// Borrow the persistent identity; no cloned lookup key or hidden prelude walk.
pub(super) fn identity<'a>(
    module: &str,
    name: &str,
    programs: &'a [Program],
    remaining: &mut usize,
) -> Result<Option<&'a str>, Vec<Diagnostic>> {
    *remaining = remaining.checked_sub(programs.len()).ok_or_else(refusal)?;
    let provider = programs
        .iter()
        .find(|p| p.module == module)
        .ok_or_else(refusal)?;
    *remaining = remaining
        .checked_sub(provider.types.len() + provider.module_uses.len())
        .ok_or_else(refusal)?;
    Ok(provider
        .types
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.stable_id.as_str())
        .or_else(|| {
            provider
                .module_uses
                .iter()
                .find(|u| u.kind == ModuleUseKind::Type && u.alias == name)
                .map(|u| u.persistent_id.as_str())
        }))
}

#[cfg(test)]
#[path = "private_layout/tests.rs"]
mod tests;
