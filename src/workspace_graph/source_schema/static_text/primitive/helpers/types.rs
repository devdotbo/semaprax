//! Borrowed defining-module record closure. IDs discover a candidate; source
//! records, current member pointers and namespace facts authenticate it.
use super::*;
type Cursor<'a> = Option<hir::lexical_exit::ChildCursor<'a>>;

pub(super) fn discover<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    index: &mut hir::DeclarationIndex,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Vec<&'a ResolvedTypeDeclaration>, Vec<Diagnostic>> {
    resources
        .reserve_array::<Cursor<'_>>(anchor, DEPTH, "retained type body cursor")
        .map_err(|_| fail())?;
    let mut types = Vec::new();
    for function in &module.functions {
        receive(
            &function.return_type,
            module,
            modules,
            facts,
            index,
            resources,
            anchor,
            &mut types,
        )?;
        for parameter in &function.params {
            receive(
                &parameter.ty,
                module,
                modules,
                facts,
                index,
                resources,
                anchor,
                &mut types,
            )?;
        }
        let mut stack = [None; DEPTH];
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
            let mut depth = 1;
            while depth != 0 {
                charge(resources, anchor, 1)?;
                let mut cursor = stack[depth - 1].take().ok_or_else(fail)?;
                if !cursor.is_started() {
                    receive(
                        &cursor.expression().ty,
                        module,
                        modules,
                        facts,
                        index,
                        resources,
                        anchor,
                        &mut types,
                    )?;
                }
                if let Some(child) = cursor.next() {
                    if depth == DEPTH {
                        return Err(fail());
                    }
                    stack[depth - 1] = Some(cursor);
                    stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                    depth += 1;
                } else {
                    depth -= 1;
                }
            }
        }
    }
    Ok(types)
}
#[allow(
    clippy::too_many_arguments,
    reason = "keep the source namespace and single preparation separate from discovery"
)]
fn receive<'a>(
    ty: &hir::ResolvedType,
    local: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    index: &mut hir::DeclarationIndex,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    held: &mut Vec<&'a ResolvedTypeDeclaration>,
) -> Result<(), Vec<Diagnostic>> {
    charge(resources, anchor, 1)?;
    let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    charge(resources, anchor, declaration.as_str().len())?;
    if declaration.as_str() == crate::prelude::VEC_ID {
        let [element] = arguments.as_slice() else {
            return Err(fail());
        };
        if matches!(element, hir::ResolvedType::Nominal { arguments, .. } if !arguments.is_empty())
        {
            return Err(fail());
        }
        return receive(
            element, local, modules, facts, index, resources, anchor, held,
        );
    }
    if !arguments.is_empty() {
        return Err(fail());
    }
    let mut selected = None;
    for module in modules {
        for current in &module.types {
            charge(
                resources,
                anchor,
                current
                    .id
                    .as_str()
                    .len()
                    .checked_add(declaration.as_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            if current.id == *declaration && selected.replace((module, current)).is_some() {
                return Err(fail());
            }
        }
    }
    let (module, current) = selected.ok_or_else(fail)?;
    if std::ptr::eq(module, local) {
        return Ok(());
    }
    super::super::current_module(module, modules, resources, anchor)?;
    let mut provider_held = false;
    for old in held.iter() {
        charge(resources, anchor, 1)?;
        if std::ptr::eq(*old, current) {
            return Ok(());
        }
        for member in &module.types {
            charge(resources, anchor, 1)?;
            if std::ptr::eq(*old, member) {
                provider_held = true;
            }
        }
    }
    let source = module.record_closure.as_deref().ok_or_else(fail)?;
    charge(resources, anchor, 33)?;
    if module.record_source_revision != Some(source.revision()) {
        return Err(fail());
    }
    if !provider_held {
        charge(resources, anchor, source.capture_work)?;
        resources
            .reserve_array::<u8>(
                anchor,
                source.owned_capacity_bytes().ok_or_else(fail)?,
                "held defining record source facts",
            )
            .map_err(|_| fail())?;
    }
    super::super::records::prepared::authenticate_provider(
        module,
        &source.records,
        declaration,
        facts,
        index,
        resources,
        anchor,
    )?;
    if held.len() == held.capacity() {
        let target = held.len().checked_add(1).ok_or_else(fail)?;
        charge(resources, anchor, held.len())?;
        resources
            .reserve_array::<&ResolvedTypeDeclaration>(
                anchor,
                target,
                "borrowed defining type directory",
            )
            .map_err(|_| fail())?;
        held.reserve_exact(1);
        if held.capacity() != target {
            return Err(fail());
        }
    }
    charge(resources, anchor, 1)?;
    held.push(current);
    Ok(())
}
