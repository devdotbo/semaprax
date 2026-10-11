//! Defining-source facts for demanded pure scalar helpers, including providers
//! with no literal inventory. Facts alone grant no replay authority.
use super::*;
use crate::hir::{
    OwnershipMode, ResolvedExprKind, ResolvedFunction, ResolvedStatement, ResolvedType,
};
use primitive::body_digest;
const DEPTH: usize = 514;
const WORK: usize = 1_048_576;
pub(in crate::workspace_graph) struct Inventory {
    pub(in crate::workspace_graph) functions: Vec<Function>,
    revision: [u8; 32],
    pub(in crate::workspace_graph) capture_work: usize,
}
pub(in crate::workspace_graph) struct Function {
    pub(in crate::workspace_graph) id: hir::DeclarationId,
    pub(in crate::workspace_graph) explicit: bool,
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) params: Vec<Parameter>,
    pub(in crate::workspace_graph) result: ResolvedType,
    digest: [u8; 32],
}
pub(in crate::workspace_graph) struct Parameter {
    pub(in crate::workspace_graph) id: hir::ValueId,
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) ty: ResolvedType,
}
fn debit(work: &mut usize, units: usize) -> Result<(), Vec<Diagnostic>> {
    *work = work.checked_sub(units).ok_or_else(fail)?;
    Ok(())
}
/// Pure still includes checked arithmetic and failing pre/postconditions.
/// No builtin provider, lexical exits, mutable borrow or carrier is inferred.
pub(in crate::workspace_graph) fn pure(
    function: &ResolvedFunction,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    if !function.effects.is_empty()
        || function.yields.is_some()
        || !hir::is_scalar_resolved_type(&function.return_type)
        || function
            .params
            .iter()
            .any(|p| p.ownership != OwnershipMode::Value || !hir::is_scalar_resolved_type(&p.ty))
        || crate::hir::lexical_exit::requires(function).map_err(|e| vec![e])?
    {
        return Ok(false);
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
            debit(work, 1)?;
            let mut cursor = stack[depth - 1].take().ok_or_else(fail)?;
            let expr = cursor.expression();
            if !hir::is_scalar_resolved_type(&expr.ty) || expr.ownership != OwnershipMode::Value {
                return Ok(false);
            }
            match &expr.kind {
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::Unary { .. }
                | ResolvedExprKind::Binary { .. }
                | ResolvedExprKind::If { .. } => {}
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {}
                ResolvedExprKind::Call {
                    instance,
                    type_arguments,
                    ..
                } if instance.is_none() && type_arguments.is_empty() => {}
                ResolvedExprKind::Block { statements, .. } => {
                    debit(work, statements.len())?;
                    if statements.iter().any(|s|!matches!(s,ResolvedStatement::Let{binding,..}|ResolvedStatement::Assign{binding,field:None,..} if binding.ownership==OwnershipMode::Value&&hir::is_scalar_resolved_type(&binding.ty))) {return Ok(false);}
                }
                _ => return Ok(false),
            }
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Ok(false);
                }
                stack[depth - 1] = Some(cursor);
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(true)
}
pub(super) fn capture(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    // Demand is merely selection; each defining source is independently held.
    if !programs.iter().any(|p| {
        p.module_uses.iter().any(|u| {
            u.kind == crate::ast::ModuleUseKind::Function && u.target_module == program.module
        })
    }) {
        return Ok(None);
    }
    let Some(sources) = sources else {
        return Ok(None);
    };
    let mut work = WORK;
    debit(&mut work, sources.len())?;
    let mut matches = sources.iter().filter(|s| s.path == program.path);
    let source = matches.next().ok_or_else(fail)?;
    if matches.next().is_some() {
        return Err(fail());
    }
    reserve_builder_structure(
        body_digest::scratch_bytes()
            .checked_add(
                DEPTH
                    .checked_mul(std::mem::size_of::<
                        Option<hir::lexical_exit::ChildCursor<'_>>,
                    >())
                    .ok_or_else(capacity)?,
            )
            .ok_or_else(capacity)?,
    )?;
    debit(
        &mut work,
        source
            .source
            .len()
            .checked_add(program.path.len())
            .and_then(|n| n.checked_add(program.module.len()))
            .and_then(|n| n.checked_add(24 + b"semaprax.retained-primitive-source.v1\0".len()))
            .ok_or_else(fail)?,
    )?;
    let revision = body_digest::source(&program.path, &program.module, &source.source);
    let mut count = 0usize;
    for source in &program.functions {
        debit(&mut work, resolved.functions.len())?;
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == source.stable_id)
            .ok_or_else(fail)?;
        if source.explicit_id
            && source.type_parameters.is_empty()
            && source.follows.is_none()
            && source
                .params
                .iter()
                .all(|p| p.mode == crate::ast::ParamMode::Value)
            && pure(function, &mut work)?
        {
            count = count.checked_add(1).ok_or_else(capacity)?;
        }
    }
    if count == 0 {
        return Ok(None);
    }
    reserve_builder_structure(
        std::mem::size_of::<Inventory>()
            .checked_add(
                count
                    .checked_mul(std::mem::size_of::<Function>())
                    .ok_or_else(capacity)?,
            )
            .ok_or_else(capacity)?,
    )?;
    let mut functions = Vec::with_capacity(count);
    if functions.capacity() != count {
        return Err(capacity());
    }
    for source in &program.functions {
        debit(&mut work, resolved.functions.len())?;
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == source.stable_id)
            .ok_or_else(fail)?;
        if !source.explicit_id
            || !source.type_parameters.is_empty()
            || source.follows.is_some()
            || source
                .params
                .iter()
                .any(|p| p.mode != crate::ast::ParamMode::Value)
            || !pure(function, &mut work)?
        {
            continue;
        }
        let origin = authored.get(source.stable_id.as_str()).ok_or_else(fail)?;
        if origin.module != program.module
            || origin.path != program.path
            || !origin.explicit
            || !origin.function.is_some_and(|f| std::ptr::eq(f, source))
            || function.name != source.name
            || source.params.len() != function.params.len()
            || primitive::ty(&source.return_type).as_ref() != Some(&function.return_type)
            || source
                .params
                .iter()
                .zip(&function.params)
                .any(|(source, current)| {
                    source.name != current.name
                        || primitive::ty(&source.ty).as_ref() != Some(&current.ty)
                })
        {
            return Err(fail());
        }
        reserve_builder_structure(
            source
                .stable_id
                .len()
                .checked_add(source.name.len())
                .and_then(|n| {
                    n.checked_add(
                        function
                            .params
                            .len()
                            .checked_mul(std::mem::size_of::<Parameter>())?,
                    )
                })
                .ok_or_else(capacity)?,
        )?;
        let mut params = Vec::with_capacity(function.params.len());
        if params.capacity() != function.params.len() {
            return Err(capacity());
        }
        for param in &function.params {
            reserve_builder_structure(
                param
                    .id
                    .as_str()
                    .len()
                    .checked_add(param.name.len())
                    .ok_or_else(capacity)?,
            )?;
            params.push(Parameter {
                id: param.id.clone(),
                name: param.name.clone(),
                ty: param.ty.clone(),
            });
        }
        let (digest, spent) = body_digest::typed_with_work(function, &revision).ok_or_else(fail)?;
        debit(&mut work, spent)?;
        functions.push(Function {
            id: function.id.clone(),
            explicit: source.explicit_id,
            name: function.name.clone(),
            params,
            result: function.return_type.clone(),
            digest,
        });
    }
    Ok(Some(Box::new(Inventory {
        functions,
        revision,
        capture_work: WORK.checked_sub(work).ok_or_else(fail)?,
    })))
}
pub(in crate::workspace_graph) fn match_work(
    inventory: &Inventory,
    fact: &Function,
    function: &ResolvedFunction,
) -> Option<usize> {
    let (digest, spent) = body_digest::typed_with_work(function, &inventory.revision)?;
    (digest == fact.digest).then_some(spent)
}
