//! Select Vec only through exact authored import identities and nominal fields.
//! Fixed charged cursor backing avoids an unbounded recursive source walk.
use super::*;
use crate::ast::FieldDeclaration;
const SLOTS: usize = 514;
const WORK: usize = 1_000_000;
struct Cursor<'a> {
    pending: [Option<(&'a Program, &'a Type)>; SLOTS],
    seen: [Option<(&'a Program, &'a TypeDeclaration)>; SLOTS],
    pending_len: usize,
    seen_len: usize,
    work: usize,
    fields: usize,
}
impl<'a> Cursor<'a> {
    fn push(&mut self, owner: &'a Program, ty: &'a Type) -> bool {
        if self.pending_len == SLOTS {
            return false;
        }
        self.pending[self.pending_len] = Some((owner, ty));
        self.pending_len += 1;
        true
    }
    fn step(&mut self) -> bool {
        self.work += 1;
        self.work <= WORK
    }
    fn fields(&mut self, owner: &'a Program, fields: &'a [FieldDeclaration]) -> bool {
        self.fields = self.fields.saturating_add(fields.len());
        self.fields <= crate::cleanup::MAX_CLEANUP_VISITED_FIELDS
            && fields.iter().all(|field| self.push(owner, &field.ty))
    }
    fn declaration(&mut self, owner: &'a Program, declaration: &'a TypeDeclaration) -> bool {
        for index in 0..self.seen_len {
            if !self.step() {
                return false;
            }
            let (previous_owner, previous) = self.seen[index].unwrap();
            if std::ptr::eq(previous_owner, owner) && std::ptr::eq(previous, declaration) {
                return true;
            }
        }
        if self.seen_len == SLOTS {
            return false;
        }
        self.seen[self.seen_len] = Some((owner, declaration));
        self.seen_len += 1;
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                self.fields(owner, fields)
            }
            TypeDeclarationKind::Variant { cases } => {
                cases.iter().all(|case| self.fields(owner, &case.fields))
            }
            TypeDeclarationKind::Resource { .. } => true,
        }
    }
}
fn contains<'a>(
    owner: &'a Program,
    root: Option<&'a Type>,
    declaration: Option<&'a TypeDeclaration>,
    programs: &'a [Program],
) -> bool {
    if root.is_some_and(|ty| !matches!(ty, Type::Named { .. })) {
        return false;
    }
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Cursor<'a>>()) {
        return false;
    }
    let mut cursor = Cursor {
        pending: [None; SLOTS],
        seen: [None; SLOTS],
        pending_len: 0,
        seen_len: 0,
        work: 0,
        fields: 0,
    };
    if let Some(ty) = root {
        if !cursor.push(owner, ty) {
            return false;
        }
    }
    if let Some(declaration) = declaration {
        if !cursor.declaration(owner, declaration) {
            return false;
        }
    }
    while cursor.pending_len != 0 {
        cursor.pending_len -= 1;
        let (owner, ty) = cursor.pending[cursor.pending_len].take().unwrap();
        if !cursor.step() {
            return false;
        }
        let Type::Named { name, arguments } = ty else {
            continue;
        };
        if name == "Vec" {
            return true;
        }
        for argument in arguments {
            if !cursor.push(owner, argument) {
                return false;
            }
        }
        if !arguments.is_empty() {
            continue;
        }
        let mut target = None;
        for candidate in &owner.types {
            if !cursor.step() {
                return false;
            }
            if candidate.name == *name {
                target = Some((owner, candidate));
                break;
            }
        }
        if target.is_none() {
            for import in &owner.module_uses {
                if !cursor.step() {
                    return false;
                }
                if import.kind != ModuleUseKind::Type || import.alias != *name {
                    continue;
                }
                for provider in programs {
                    if !cursor.step() {
                        return false;
                    }
                    if provider.module != import.target_module {
                        continue;
                    }
                    for candidate in &provider.types {
                        if !cursor.step() {
                            return false;
                        }
                        if candidate.stable_id == import.persistent_id {
                            target = Some((provider, candidate));
                            break;
                        }
                    }
                    break;
                }
                break;
            }
        }
        if let Some((owner, declaration)) = target {
            if !cursor.declaration(owner, declaration) {
                return false;
            }
        }
    }
    false
}
pub(super) fn module_uses(source: &Program, programs: &[Program]) -> bool {
    source.module_uses.iter().any(|import| {
        let Some(owner) = programs
            .iter()
            .find(|program| program.module == import.target_module)
        else {
            return false;
        };
        match import.kind {
            ModuleUseKind::Function => owner
                .functions
                .iter()
                .filter(|function| function.stable_id == import.persistent_id)
                .any(|function| {
                    contains(owner, Some(&function.return_type), None, programs)
                        || function
                            .params
                            .iter()
                            .any(|param| contains(owner, Some(&param.ty), None, programs))
                }),
            ModuleUseKind::Type => owner
                .types
                .iter()
                .filter(|declaration| declaration.stable_id == import.persistent_id)
                .any(|declaration| contains(owner, None, Some(declaration), programs)),
            ModuleUseKind::Protocol => false,
        }
    })
}
