//! Independent additive internal-record profile; public Bytes ABIs remain frozen.
#[cfg(test)]
use super::{hir_error, owned_collection_record, substitute_type, ResolvedProgram};
use super::{DeclarationIndex, DeclarationKind, ResolvedType};
use std::borrow::Cow;
use std::collections::BTreeSet;
pub(crate) mod paid;
#[cfg(test)]
pub(crate) mod saved;

/// Ordinary internal record runtime only. Frozen Project selectors use
/// `admitted` so a new composition cannot widen their private ABI silently.
pub(crate) fn runtime_admitted(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    admitted(root, declarations) || super::owned_collection_record::admitted(root, declarations)
}

pub(crate) fn admitted(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    if !matches!(root, ResolvedType::Nominal { .. }) {
        return false;
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType, usize),
        Leave,
    }
    let mut pending = vec![Frame::Enter(root, 1)];
    let mut active = Vec::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut text = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ty, _) if crate::map_ops::is_collection(ty) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String, _) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::Bytes, _) => {
                leaves += 1;
            }
            Frame::Enter(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::Char
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => {}
            Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return false;
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = ty
                else {
                    unreachable!()
                };
                if declarations.has_record_invariant(declaration)
                    || !arguments.is_empty()
                    || declarations
                        .type_parameters(declaration)
                        .is_none_or(|p| !p.is_empty())
                    || declarations
                        .declaration(declaration)
                        .is_none_or(|d| d.kind != DeclarationKind::Record)
                {
                    return false;
                }
                let Some(declared) = declarations.record_fields(declaration) else {
                    return false;
                };
                // Same complete structural identity; only the active path
                // participates in cycle detection, in the original order.
                if active.contains(&ty) {
                    return false;
                }
                active.push(ty);
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return false;
                }
                pending.push(Frame::Leave);
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(&field.ty, depth + 1)),
                );
            }
            Frame::Leave => {
                active.pop();
            }
            Frame::Enter(_, _) => return false,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return false;
        }
    }
    text
}

/// Select a String-bearing record from exact declaration fields. This is a
/// separate walk from admission: hostile fields cannot hide behind a failed
/// or truncated admission traversal.
pub(crate) fn contains_string(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    if !matches!(root, ResolvedType::Nominal { declaration, .. }
        if declarations.declaration(declaration).is_some_and(|item| item.kind == DeclarationKind::Record))
    {
        return false;
    }
    let mut pending = vec![Cow::Borrowed(root)];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if ty.as_ref() == &ResolvedType::String || crate::map_ops::is_collection(&ty) {
            return true;
        }
        if visited.contains(&ty) {
            continue;
        }
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty.as_ref()
        {
            if let Some(fields) = declarations.record_fields(declaration) {
                for field in fields {
                    // Leaf and zero-argument nominal substitution is identity.
                    // Parameter and compound templates retain the exact fallible
                    // substitute_type route, including foreign-owner refusal.
                    let identity = match &field.ty {
                        ResolvedType::Nominal { arguments, .. } => arguments.is_empty(),
                        ResolvedType::Function { .. } | ResolvedType::TypeParameter { .. } => false,
                        ResolvedType::Never
                        | ResolvedType::OnceFunction
                        | ResolvedType::OnceFunctionI64
                        | ResolvedType::OnceFunctionI64Pair
                        | ResolvedType::MutFunctionI64
                        | ResolvedType::Unit
                        | ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::Char
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::ArrayU8(_)
                        | ResolvedType::F32
                        | ResolvedType::F64
                        | ResolvedType::Bool
                        | ResolvedType::String
                        | ResolvedType::Bytes
                        | ResolvedType::Str
                        | ResolvedType::SliceU8
                        | ResolvedType::StringMap => true,
                    };
                    if identity {
                        pending.push(Cow::Borrowed(&field.ty));
                    } else if let Ok(ty) = super::substitute_type(&field.ty, declaration, arguments)
                    {
                        pending.push(Cow::Owned(ty));
                    }
                }
            }
        }
        // Move owned substitutions once; borrowed roots/fields stay references.
        visited.insert(ty);
    }
    false
}

#[cfg(test)]
pub(crate) fn validate_use(
    root: &ResolvedType,
    declarations: &DeclarationIndex,
) -> Result<(), crate::diagnostic::Diagnostic> {
    if contains_string(root, declarations) && !runtime_admitted(root, declarations) {
        return Err(super::hir_error(
            "String-bearing executable record is outside the bounded owned-text profile",
        ));
    }
    Ok(())
}

/// Additive owning-record selection for runtime String helpers. The ordinary
/// scalar signature traversal stays unchanged.
pub(crate) fn program_uses_strings(program: &super::ResolvedProgram) -> bool {
    program.types.iter().any(|declaration| {
        admitted(
            &ResolvedType::Nominal {
                declaration: declaration.id.clone(),
                arguments: Vec::new(),
            },
            &program.declarations,
        )
    })
}

// Complete pre-change owned traversal, independent of the production views.
#[cfg(test)]
pub(crate) mod oracle {
    use super::*;
    /// Ordinary internal record runtime only. Frozen Project selectors use
    /// `admitted` so a new composition cannot widen their private ABI silently.
    pub(crate) fn runtime_admitted(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
        admitted(root, declarations)
            || crate::hir::owned_collection_record::admitted(root, declarations)
    }

    pub(crate) fn admitted(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
        if !matches!(root, ResolvedType::Nominal { .. }) {
            return false;
        }
        enum Frame {
            Enter(ResolvedType, usize),
            Leave(String),
        }
        let mut pending = vec![Frame::Enter(root.clone(), 1)];
        let mut active = BTreeSet::new();
        let mut fields = 0usize;
        let mut leaves = 0usize;
        let mut text = false;
        while let Some(frame) = pending.pop() {
            match frame {
                Frame::Enter(ty, _) if crate::map_ops::is_collection(&ty) => {
                    text = true;
                    leaves += 1;
                }
                Frame::Enter(ResolvedType::String, _) => {
                    text = true;
                    leaves += 1;
                }
                Frame::Enter(ResolvedType::Bytes, _) => {
                    leaves += 1;
                }
                Frame::Enter(
                    ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::Char
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool,
                    _,
                ) => {}
                Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                    if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                        return false;
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = &ty
                    else {
                        unreachable!()
                    };
                    if declarations
                        .declaration(&crate::hir::DeclarationId::new(format!(
                            "{declaration}#invariant"
                        )))
                        .is_some()
                        || !arguments.is_empty()
                        || declarations
                            .type_parameters(declaration)
                            .is_none_or(|p| !p.is_empty())
                        || declarations
                            .declaration(declaration)
                            .is_none_or(|d| d.kind != DeclarationKind::Record)
                    {
                        return false;
                    }
                    let Some(declared) = declarations.record_fields(declaration) else {
                        return false;
                    };
                    let identity = ty.identity_key();
                    if !active.insert(identity.clone()) {
                        return false;
                    }
                    fields += declared.len();
                    if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                        return false;
                    }
                    pending.push(Frame::Leave(identity));
                    pending.extend(
                        declared
                            .iter()
                            .rev()
                            .map(|field| Frame::Enter(field.ty.clone(), depth + 1)),
                    );
                }
                Frame::Leave(identity) => {
                    active.remove(&identity);
                }
                Frame::Enter(_, _) => return false,
            }
            if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                return false;
            }
        }
        text
    }

    /// Select a String-bearing record from exact declaration fields. This is a
    /// separate walk from admission: hostile fields cannot hide behind a failed
    /// or truncated admission traversal.
    pub(crate) fn contains_string(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
        if !matches!(root, ResolvedType::Nominal { declaration, .. }
        if declarations.declaration(declaration).is_some_and(|item| item.kind == DeclarationKind::Record))
        {
            return false;
        }
        let mut pending = vec![root.clone()];
        let mut visited = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            if ty == ResolvedType::String || crate::map_ops::is_collection(&ty) {
                return true;
            }
            if !visited.insert(ty.clone()) {
                continue;
            }
            if let ResolvedType::Nominal {
                declaration,
                arguments,
            } = ty
            {
                if let Some(fields) = declarations.record_fields(&declaration) {
                    for field in fields {
                        if let Ok(ty) =
                            crate::hir::substitute_type(&field.ty, &declaration, &arguments)
                        {
                            pending.push(ty);
                        }
                    }
                }
            }
        }
        false
    }

    pub(crate) fn validate_use(
        root: &ResolvedType,
        declarations: &DeclarationIndex,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        if contains_string(root, declarations) && !runtime_admitted(root, declarations) {
            return Err(crate::hir::hir_error(
                "String-bearing executable record is outside the bounded owned-text profile",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::DeclarationId;

    fn fixture() -> DeclarationIndex {
        let source = crate::parse(
            r#"module text.classifier;
@id("text.row") record Row { @id("text.row.field") field: i64, }
@id("text.box") record Box<T> { @id("text.box.field") field: T, }
@id("text.main") fn main() -> i64 { 0 }
"#,
            std::path::Path::new("text-classifier.spx"),
        )
        .unwrap();
        crate::hir::resolve(&source).unwrap().declarations.clone()
    }
    fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
        ResolvedType::Nominal {
            declaration: DeclarationId::new(id),
            arguments,
        }
    }
    fn parity(index: &DeclarationIndex, ty: &ResolvedType) {
        assert_eq!(admitted(ty, index), oracle::admitted(ty, index));
        assert_eq!(
            contains_string(ty, index),
            oracle::contains_string(ty, index)
        );
        let verdict = |result: Result<(), crate::diagnostic::Diagnostic>| {
            result.map_err(|error| (error.code, error.message))
        };
        assert_eq!(
            verdict(validate_use(ty, index)),
            verdict(oracle::validate_use(ty, index))
        );
    }
    #[test]
    fn borrowed_text_classifiers_preserve_limits_cycles_and_substitution_errors() {
        let mut index = fixture();
        let id = DeclarationId::new("text.row");
        let field = index.record_fields[&id][0].clone();
        let root = nominal("text.row", vec![]);
        for count in [0, 1, 256, 257, 4096, 4097] {
            let mut fields = vec![field.clone(); count];
            for f in &mut fields {
                f.ty = ResolvedType::I64;
            }
            if let Some(f) = fields.first_mut() {
                f.ty = ResolvedType::String;
            }
            index.record_fields.insert(id.clone(), fields);
            parity(&index, &root);
            assert_eq!(admitted(&root, &index), count != 0 && count <= 4096);
        }
        for count in [256, 257] {
            let mut fields = vec![field.clone(); count];
            for f in &mut fields {
                f.ty = ResolvedType::String;
            }
            index.record_fields.insert(id.clone(), fields);
            parity(&index, &root);
            assert_eq!(admitted(&root, &index), count == 256);
        }
        for depth in [64, 65] {
            for level in 0..depth {
                let level_id = DeclarationId::new(format!("text.depth.{level}"));
                let mut declaration = index.declarations[&id].clone();
                declaration.id = level_id.clone();
                index.declarations.insert(level_id.clone(), declaration);
                index.type_parameters.insert(level_id.clone(), vec![]);
                let mut f = field.clone();
                f.ty = if level + 1 == depth {
                    ResolvedType::String
                } else {
                    nominal(&format!("text.depth.{}", level + 1), vec![])
                };
                index.record_fields.insert(level_id, vec![f]);
            }
            let ty = nominal("text.depth.0", vec![]);
            parity(&index, &ty);
            assert_eq!(admitted(&ty, &index), depth == 64);
        }
        let mut recursive = field.clone();
        recursive.ty = root.clone();
        let mut text = field.clone();
        text.ty = ResolvedType::String;
        index
            .record_fields
            .insert(id.clone(), vec![recursive, text]);
        parity(&index, &root);
        assert!(contains_string(&root, &index));
        assert!(!admitted(&root, &index));
        for ty in [
            ResolvedType::String,
            ResolvedType::Bytes,
            root.clone(),
            nominal("missing.α:0:", vec![]),
            nominal("text.box", vec![ResolvedType::String]),
            nominal("text.box", vec![]),
        ] {
            parity(&index, &ty);
        }
        // Repeated siblings are not active cycles; the leave frame retires
        // precisely one ancestor before the next sibling is examined.
        let mut leaf = field.clone();
        leaf.ty = ResolvedType::String;
        index.record_fields.insert(id.clone(), vec![leaf]);
        let mut parent = field.clone();
        parent.ty = root.clone();
        let parent_id = DeclarationId::new("text.parent");
        let mut declaration = index.declarations[&id].clone();
        declaration.id = parent_id.clone();
        index.declarations.insert(parent_id.clone(), declaration);
        index.type_parameters.insert(parent_id.clone(), vec![]);
        index
            .record_fields
            .insert(parent_id, vec![parent.clone(), parent]);
        let siblings = nominal("text.parent", vec![]);
        parity(&index, &siblings);
        assert!(admitted(&siblings, &index));
        let invariant_id = DeclarationId::new("text.row#invariant");
        let mut invariant = index.declarations[&id].clone();
        invariant.id = invariant_id.clone();
        index.declarations.insert(invariant_id.clone(), invariant);
        parity(&index, &root);
        assert!(!admitted(&root, &index));
        index.declarations.remove(&invariant_id);
        for ty in [
            ResolvedType::TypeParameter {
                owner: DeclarationId::new("foreign"),
                index: 0,
            },
            nominal(
                "text.box",
                vec![ResolvedType::TypeParameter {
                    owner: id.clone(),
                    index: 0,
                }],
            ),
            ResolvedType::Function {
                parameters: vec![ResolvedType::String],
                result: Box::new(ResolvedType::Bool),
            },
        ] {
            let mut f = field.clone();
            f.ty = ty;
            index.record_fields.insert(id.clone(), vec![f]);
            parity(&index, &root);
        }
    }
    #[test]
    fn borrowed_text_classifiers_reduce_actual_allocations_and_settle_drop() {
        let mut index = fixture();
        let id = DeclarationId::new("text.row");
        index.record_fields.get_mut(&id).unwrap()[0].ty = ResolvedType::String;
        let root = nominal("text.row", vec![]);
        let (old, old_metrics) = crate::test_allocation_observer::measure(|| {
            (
                oracle::admitted(&root, &index),
                oracle::contains_string(&root, &index),
            )
        });
        let (actual, metrics) = crate::test_allocation_observer::measure(|| {
            (admitted(&root, &index), contains_string(&root, &index))
        });
        assert_eq!(actual, (true, true));
        assert_eq!(actual, old);
        for m in [metrics, old_metrics] {
            assert!(!m.invalid);
            assert_eq!(m.live, 0);
            assert_eq!(m.allocated, m.freed);
        }
        assert!(metrics.allocations < old_metrics.allocations);
        assert!(metrics.allocated < old_metrics.allocated);
    }
}
