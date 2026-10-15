//! Exact borrowed type-fact replay. Nominal spelling alone grants no admission.
use super::*;

fn identity(out: &mut impl Write, ty: &ResolvedType, depth: usize) -> fmt::Result {
    if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
        return Err(fmt::Error);
    }
    let scalar = match ty {
        ResolvedType::I64 => "i64",
        ResolvedType::I32 => "i32",
        ResolvedType::U8 => "u8",
        ResolvedType::Usize => "usize",
        ResolvedType::Char => "char",
        ResolvedType::Bool => "bool",
        ResolvedType::F32 => "f32",
        ResolvedType::F64 => "f64",
        ResolvedType::String => "string",
        ResolvedType::Bytes => "bytes",
        ResolvedType::ArrayU8(length) => return write!(out, "array:u8:{length}"),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            write!(
                out,
                "nominal:{}:{}:{}:",
                declaration.as_str().len(),
                declaration,
                arguments.len()
            )?;
            for argument in arguments {
                let mut size = Size(0);
                identity(&mut size, argument, depth + 1)?;
                write!(out, "{}:", size.0)?;
                identity(out, argument, depth + 1)?;
            }
            return Ok(());
        }
        _ => return Err(fmt::Error),
    };
    out.write_str(scalar)
}
struct Size(usize);
impl Write for Size {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}
fn compiler(
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    id: &str,
    kind: hir::DeclarationKind,
    owner: Option<&str>,
) -> bool {
    facts.get(id).is_some_and(|fact| {
        fact.kind == kind
            && fact.origin == hir::IdentityOrigin::CompilerOwned
            && fact.owner.as_deref() == owner
            && fact.path.is_none()
            && fact.module.is_none()
    })
}

// A child's length is framing only: its complete contents are independently
// replayed against the authenticated field type below.
fn child_layout<'a>(layout: &mut MatchText<'a>) -> Option<&'a str> {
    let (size, rest) = layout.0.split_once(':')?;
    let length = size.parse::<usize>().ok()?;
    if size.is_empty()
        || !size.bytes().all(|b| b.is_ascii_digit())
        || (size.len() > 1 && size.starts_with('0'))
    {
        return None;
    }
    let child = rest.get(..length)?;
    layout.0 = rest.get(length..)?;
    Some(child)
}

fn replay(
    ty: &ResolvedType,
    text: &str,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    depth: usize,
    work: &mut usize,
) -> Option<(bool, bool)> {
    if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
        return None;
    }
    *work = work.checked_sub(1)?;
    if let Some(key) = scalar_layout(ty) {
        return (text == key).then_some((true, false));
    }
    match ty {
        ResolvedType::ArrayU8(length) => {
            return matches_text(text, format_args!("array:u8:{length}")).then_some((true, false))
        }
        ResolvedType::String => return (text == "owned:string").then_some((false, true)),
        ResolvedType::Bytes => return (text == "owned:bytes").then_some((false, true)),
        _ => {}
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    if declaration.as_str() == crate::prelude::VEC_ID {
        if !compiler(
            facts,
            crate::prelude::VEC_ID,
            hir::DeclarationKind::Record,
            None,
        ) {
            return None;
        }
        let [element] = arguments.as_slice() else {
            return None;
        };
        let prefix = if crate::vec_ops::resolved_vec_element_is_admitted(element) {
            "vec"
        } else if *element == ResolvedType::String {
            "declared-vector-only"
        } else {
            vector_record(element, modules, facts)?
        };
        let mut layout = MatchText(text);
        write!(layout, "{prefix}:").ok()?;
        identity(&mut layout, element, 0).ok()?;
        return layout.0.is_empty().then_some((false, true));
    }
    if declaration.as_str() == crate::prelude::OPTION_ID {
        use crate::prelude::{OPTION_ID, OPTION_NONE_ID, OPTION_SOME_ID, OPTION_SOME_VALUE_ID};
        if !compiler(facts, OPTION_ID, hir::DeclarationKind::Variant, None)
            || !compiler(
                facts,
                OPTION_NONE_ID,
                hir::DeclarationKind::VariantCase,
                Some(OPTION_ID),
            )
            || !compiler(
                facts,
                OPTION_SOME_ID,
                hir::DeclarationKind::VariantCase,
                Some(OPTION_ID),
            )
            || !compiler(
                facts,
                OPTION_SOME_VALUE_ID,
                hir::DeclarationKind::CaseField,
                Some(OPTION_SOME_ID),
            )
        {
            return None;
        }
        let [argument] = arguments.as_slice() else {
            return None;
        };
        if !matches!(
            argument,
            ResolvedType::I64 | ResolvedType::Bool | ResolvedType::U8
        ) {
            return None;
        }
        let key = scalar_layout(argument)?;
        return matches_text(
            text,
            format_args!(
                "variant:{}:{}:2:{}:{}:0:{}:{}:1:{}:{}:{}:{}",
                OPTION_ID.len(),
                OPTION_ID,
                OPTION_NONE_ID.len(),
                OPTION_NONE_ID,
                OPTION_SOME_ID.len(),
                OPTION_SOME_ID,
                OPTION_SOME_VALUE_ID.len(),
                OPTION_SOME_VALUE_ID,
                key.len(),
                key
            ),
        )
        .then_some((true, false));
    }
    if !arguments.is_empty() {
        return None;
    }
    let origin = facts.get(declaration.as_str())?;
    if origin.kind != hir::DeclarationKind::Record
        || origin.origin != hir::IdentityOrigin::Explicit
        || origin.owner.is_some()
    {
        return None;
    }
    let mut records = modules
        .iter()
        .flat_map(|module| module.types.iter().map(move |record| (module, record)))
        .filter(|(_, record)| record.id == *declaration);
    let (owner, record) = records.next()?;
    if records.next().is_some()
        || origin.path.as_deref() != Some(owner.path.as_str())
        || origin.module.as_deref() != Some(owner.module.as_str())
        || !record.type_parameters.is_empty()
        || modules
            .iter()
            .filter(|module| module.path == owner.path || module.module == owner.module)
            .count()
            != 1
    {
        return None;
    }
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return None;
    };
    if fields.len() > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
        return None;
    }
    let mut layout = MatchText(text);
    write!(
        layout,
        "record:{}:{}:{}:",
        declaration.as_str().len(),
        declaration,
        fields.len()
    )
    .ok()?;
    let mut copy = true;
    let mut drop = false;
    for (position, field) in fields.iter().enumerate() {
        let origin = facts.get(field.id.as_str())?;
        if origin.kind != hir::DeclarationKind::Field
            || origin.owner.as_deref() != Some(declaration.as_str())
            || origin.path.as_deref() != Some(owner.path.as_str())
            || origin.module.as_deref() != Some(owner.module.as_str())
            || fields[..position]
                .iter()
                .any(|previous| previous.id == field.id)
        {
            return None;
        }
        write!(layout, "{}:{}:", field.id.as_str().len(), field.id).ok()?;
        let (field_copy, field_drop) = replay(
            &field.ty,
            child_layout(&mut layout)?,
            modules,
            facts,
            depth + 1,
            work,
        )?;
        if field_drop && origin.origin != hir::IdentityOrigin::Explicit {
            return None;
        }
        copy &= field_copy;
        drop |= field_drop;
    }
    layout.0.is_empty().then_some((copy, drop))
}

fn vector_record(
    ty: &ResolvedType,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Option<&'static str> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let origin = facts.get(declaration.as_str())?;
    if origin.kind != hir::DeclarationKind::Record
        || origin.origin != hir::IdentityOrigin::Explicit
        || origin.owner.is_some()
    {
        return None;
    }
    let mut records = modules
        .iter()
        .flat_map(|module| module.types.iter().map(move |record| (module, record)))
        .filter(|(_, record)| record.id == *declaration);
    let (owner, record) = records.next()?;
    if records.next().is_some()
        || origin.path.as_deref() != Some(owner.path.as_str())
        || origin.module.as_deref() != Some(owner.module.as_str())
        || !record.type_parameters.is_empty()
        || modules
            .iter()
            .filter(|module| module.path == owner.path || module.module == owner.module)
            .count()
            != 1
    {
        return None;
    }
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return None;
    };
    if !(1..=hir::copy_record_collection::MAX_FIELDS).contains(&fields.len()) {
        return None;
    }
    let mut owned = 0usize;
    let mut strings = 0usize;
    let mut bytes = 0usize;
    let mut explicit = true;
    for (position, field) in fields.iter().enumerate() {
        let origin = facts.get(field.id.as_str())?;
        if origin.kind != hir::DeclarationKind::Field
            || origin.owner.as_deref() != Some(declaration.as_str())
            || origin.path.as_deref() != Some(owner.path.as_str())
            || origin.module.as_deref() != Some(owner.module.as_str())
            || fields[..position]
                .iter()
                .any(|previous| previous.id == field.id)
        {
            return None;
        }
        explicit &= origin.origin == hir::IdentityOrigin::Explicit;
        if matches!(field.ty, ResolvedType::String | ResolvedType::Bytes) {
            owned += 1;
            strings += usize::from(field.ty == ResolvedType::String);
            bytes += usize::from(field.ty == ResolvedType::Bytes);
        } else {
            if !crate::vec_ops::resolved_element_is_admitted(&field.ty) {
                return None;
            }
            scalar_layout(&field.ty)?;
        }
    }
    // Replay the *same priority* as DeclarationIndex::owned_builtin_facts.
    // These layout tokens describe checked storage, not runtime admission.
    // The declared one-String family precedes the owned-leaf family there.
    if owned == 0 || (bytes == 2 && strings == 0 && fields.len() == 3) {
        Some("vec")
    } else if strings == 1 && bytes == 0 {
        Some("declared-vector-only")
    } else if explicit
        && fields.len() <= hir::owned_leaf_collection::MAX_FIELDS
        && owned <= hir::owned_leaf_collection::MAX_OWNED_FIELDS
    {
        Some("owned-leaf-vector-v1")
    } else {
        None
    }
}

fn checked(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    ty: &ResolvedType,
) -> Option<(bool, bool)> {
    let ResolvedType::Nominal { declaration, .. } = ty else {
        return None;
    };
    let (kind, checked) = module.signature_types.iter().find_map(|(key, value)| {
        let mut matcher = MatchText(key);
        (identity(&mut matcher, ty, 0).is_ok() && matcher.0.is_empty()).then_some(value)
    })?;
    if facts.get(declaration.as_str())?.kind != *kind || checked.contains_resource || !checked.sized
    {
        return None;
    }
    let mut work = crate::cleanup::MAX_CLEANUP_VISITED_FIELDS;
    let (copy, drop) = replay(ty, &checked.layout_key, modules, facts, 0, &mut work)?;
    (checked.copy == copy && checked.needs_drop == drop).then_some((copy, drop))
}
pub(super) fn copy(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    ty: &ResolvedType,
) -> bool {
    hir::is_scalar_resolved_type(ty)
        || matches!(ty, ResolvedType::ArrayU8(_))
        || checked(module, modules, facts, ty) == Some((true, false))
}
pub(super) fn parameter(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    mode: OwnershipMode,
    ty: &ResolvedType,
) -> bool {
    match mode {
        OwnershipMode::Own => *ty == ResolvedType::Bytes,
        OwnershipMode::Value => copy(module, modules, facts, ty),
        OwnershipMode::Borrow => {
            matches!(ty, ResolvedType::SliceU8 | ResolvedType::Str)
                || checked(module, modules, facts, ty).is_some()
        }
        OwnershipMode::Shared => false,
    }
}
