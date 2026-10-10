//! Bounded, iterative retention of the checked nominal types a function
//! signature or body actually reaches, moved out of `workspace_graph.rs` to
//! keep that module under its recorded size budget (see
//! `tests/module-size-budget.tsv`). Moved verbatim from the root; only the
//! `use` imports below are new.

use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;
use crate::hir;

use super::{
    active_builder_limit, graph_error, limit_error, reserve_builder_structure, CheckedValueNode,
    MAX_CHECKED_VALUE_DEPTH, MAX_CHECKED_VALUE_VISITS, MAX_DECLARATIONS,
};

pub(super) const NOMINAL_KEY_SCRATCH_BYTES: usize = 256;

pub(super) fn retain_checked_value_types(
    root: CheckedValueNode<'_>,
    declarations: &hir::DeclarationIndex,
    retained: &mut BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>,
    visits: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    // One cursor per active ancestor: wide statement/pattern lists cannot
    // allocate an unbounded sibling queue. This fixed scratch stack is not
    // retained; its peak storage is charged once by the inventory entry point.
    let mut stack = [None; MAX_CHECKED_VALUE_DEPTH + 1];
    stack[0] = Some((root, 0usize));
    let mut depth = 0usize;
    loop {
        let (node, next) = stack[depth].expect("active checked value cursor");
        if next == 0 {
            if *visits >= MAX_CHECKED_VALUE_VISITS {
                return Err(vec![limit_error(
                    "checked_value_visits",
                    MAX_CHECKED_VALUE_VISITS,
                )]);
            }
            *visits += 1;
            if let Some(ty) = node.ty() {
                retain_checked_nominal_type(ty, declarations, retained)?;
            }
        }
        if let Some(child) = node.child(next) {
            if depth == MAX_CHECKED_VALUE_DEPTH {
                return Err(vec![limit_error(
                    "checked_value_depth",
                    MAX_CHECKED_VALUE_DEPTH,
                )]);
            }
            stack[depth] = Some((node, next + 1));
            depth += 1;
            stack[depth] = Some((child, 0));
        } else if depth == 0 {
            break;
        } else {
            stack[depth] = None;
            depth -= 1;
        }
    }
    Ok(())
}

pub(super) fn retain_checked_nominal_type(
    ty: &hir::ResolvedType,
    declarations: &hir::DeclarationIndex,
    retained: &mut BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>,
) -> Result<(), Vec<Diagnostic>> {
    let hir::ResolvedType::Nominal { declaration, .. } = ty else {
        return Ok(());
    };
    // Most application records have no type arguments. Probe their exact
    // canonical key on bounded stack storage before allocating a duplicate
    // String (and identity_key's traversal carriers) for every body use.
    // Oversized identities and generic instances retain the ordinary path.
    if let hir::ResolvedType::Nominal { arguments, .. } = ty {
        if arguments.is_empty() {
            use std::io::Write;
            let mut storage = [0u8; NOMINAL_KEY_SCRATCH_BYTES];
            let mut cursor = std::io::Cursor::new(storage.as_mut_slice());
            if write!(cursor, "nominal:{}:{}:0:", declaration.as_str().len(), declaration).is_ok() {
                let length = cursor.position() as usize;
                let key = std::str::from_utf8(&storage[..length])
                    .expect("identity key is composed of UTF-8 strings");
                if retained.contains_key(key) {
                    return Ok(());
                }
            }
        }
    }
    let key = ty.identity_key();
    if retained.contains_key(&key) {
        return Ok(());
    }
    if retained.len() >= MAX_DECLARATIONS {
        return Err(vec![limit_error("declarations", MAX_DECLARATIONS)]);
    }
    let kind = declarations
        .declaration(declaration)
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "checked value nominal declaration is absent",
            )]
        })?
        .kind;
    let facts = declarations.type_facts(ty).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "checked value type facts are absent",
        )]
    })?;
    let base = if retained.is_empty() {
        std::mem::size_of::<BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>>()
    } else {
        0
    };
    let bytes = base
        .checked_add(
            std::mem::size_of::<(String, hir::DeclarationKind, hir::TypeFacts)>()
                + 8 * std::mem::size_of::<usize>(),
        )
        .and_then(|bytes| bytes.checked_add(key.capacity()))
        .and_then(|bytes| bytes.checked_add(facts.layout_key.capacity()))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(bytes)?;
    retained.insert(key, (kind, facts));
    Ok(())
}
