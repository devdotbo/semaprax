//! Rare source-derived signatures for module-local primitive lexical replay.
//! These are captured from the authoritative AST, never from a cleanup schema.
use super::*;
use crate::ast::{ParamMode, Type};
mod body_digest;
pub(in crate::workspace_graph) mod records;

pub(in crate::workspace_graph) struct Inventory {
    pub(in crate::workspace_graph) functions: Vec<FunctionFact>,
    pub(in crate::workspace_graph) records: Option<Box<records::Records>>,
    source_revision: [u8; 32],
}
pub(in crate::workspace_graph) struct FunctionFact {
    pub(in crate::workspace_graph) id: hir::DeclarationId,
    pub(in crate::workspace_graph) explicit: bool,
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) params: Vec<ParameterFact>,
    pub(in crate::workspace_graph) result: hir::ResolvedType,
    body: [u8; 32],
}
pub(in crate::workspace_graph) struct ParameterFact {
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) ownership: hir::OwnershipMode,
    pub(in crate::workspace_graph) ty: hir::ResolvedType,
}

fn ty(ty: &Type) -> Option<hir::ResolvedType> {
    use hir::ResolvedType as R;
    Some(match ty {
        Type::I64 => R::I64,
        Type::I32 => R::I32,
        Type::Char => R::Char,
        Type::U8 => R::U8,
        Type::Usize => R::Usize,
        Type::ArrayU8(size) => R::ArrayU8(*size),
        Type::F32 => R::F32,
        Type::F64 => R::F64,
        Type::Bool => R::Bool,
        Type::String => R::String,
        Type::Bytes => R::Bytes,
        Type::Str => R::Str,
        Type::SliceU8 => R::SliceU8,
        Type::StringMap => R::StringMap,
        Type::OnceFunction => R::OnceFunction,
        Type::OnceFunctionI64 => R::OnceFunctionI64,
        Type::OnceFunctionI64Pair => R::OnceFunctionI64Pair,
        Type::MutFunctionI64 => R::MutFunctionI64,
        Type::Never | Type::Named { .. } | Type::Function { .. } => return None,
    })
}
fn ownership(param: &crate::ast::Param, ty: &hir::ResolvedType) -> Option<hir::OwnershipMode> {
    use hir::{OwnershipMode as O, ResolvedType as R};
    if param.mode == ParamMode::Shared {
        return None;
    }
    Some(
        if param.mode == ParamMode::Borrow && matches!(ty, R::Bytes | R::StringMap) {
            O::Borrow
        } else if matches!(
            ty,
            R::Bytes
                | R::String
                | R::StringMap
                | R::OnceFunction
                | R::OnceFunctionI64
                | R::OnceFunctionI64Pair
        ) {
            O::Own
        } else if matches!(ty, R::Str | R::SliceU8) {
            if param.mode != ParamMode::Borrow {
                return None;
            }
            O::Borrow
        } else {
            match param.mode {
                ParamMode::Value => O::Value,
                ParamMode::Own => O::Own,
                ParamMode::Borrow => O::Borrow,
                ParamMode::Shared => return None,
            }
        },
    )
}

/// Unsupported shapes keep the retained lexical route closed. They do not
/// change ordinary source checking, retention or direct cleanup replay.
pub(super) fn capture(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    authenticated_sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    if !program.module_uses.is_empty()
        || !program.agents.is_empty()
        || !program.interfaces.is_empty()
        || !program.protocols.is_empty()
        || !program.implementations.is_empty()
        || !program.permits.is_empty()
    {
        return Ok(None);
    }
    let mut work = WORK;
    if !records::preflight(program, resolved, &mut work)? {
        return Ok(None);
    }
    for function in &program.functions {
        work = work
            .checked_sub(function.params.len().checked_add(1).ok_or_else(fail)?)
            .ok_or_else(fail)?;
        // Named-type lookups scan the bounded local declaration inventory.
        work = work
            .checked_sub(
                function
                    .params
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(program.types.len()))
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        if !function.type_parameters.is_empty()
            || !function.effects.is_empty()
            || function.yields.is_some()
            || function.follows.is_some()
            || !records::source_type_admitted(program, &function.return_type)
        {
            return Ok(None);
        }
        for param in &function.params {
            if !records::source_type_admitted(program, &param.ty) {
                return Ok(None);
            }
            if !matches!(param.ty, Type::Named { .. })
                && ownership(param, &ty(&param.ty).expect("primitive")).is_none()
            {
                return Ok(None);
            }
        }
    }
    // This factory is reachable only while the checked authoritative AST and
    // its checked resolved program are both held. Freeze a complete semantic
    // typed-body guard; never accept an expected digest from a candidate.
    let Some(sources) = authenticated_sources else {
        return Ok(None);
    };
    work.checked_sub(sources.len()).ok_or_else(fail)?;
    let mut current_sources = sources.iter().filter(|source| source.path == program.path);
    let source = current_sources.next().ok_or_else(fail)?;
    if current_sources.next().is_some() {
        return Err(fail());
    }
    reserve_builder_structure(body_digest::scratch_bytes())?;
    let source_revision = body_digest::source(&program.path, &program.module, &source.source);
    let mut body_work = WORK;
    for source in &program.functions {
        body_work = body_work
            .checked_sub(resolved.functions.len())
            .ok_or_else(fail)?;
        let origin = authored.get(source.stable_id.as_str()).ok_or_else(fail)?;
        if origin.path != program.path
            || origin.module != program.module
            || origin.function.is_none()
        {
            return Err(fail());
        }
        let mut matches = resolved
            .functions
            .iter()
            .filter(|function| function.id.as_str() == source.stable_id);
        let function = matches.next().ok_or_else(fail)?;
        if matches.next().is_some() {
            return Err(fail());
        }
        work = work
            .checked_sub(
                source
                    .params
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(program.types.len()))
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        if !records::source_type_matches(program, &source.return_type, &function.return_type)
            || source.params.len() != function.params.len()
            || source
                .params
                .iter()
                .zip(&function.params)
                .any(|(source, current)| {
                    !records::source_type_matches(program, &source.ty, &current.ty)
                        || if matches!(source.ty, Type::Named { .. }) {
                            match source.mode {
                                ParamMode::Own => current.ownership != hir::OwnershipMode::Own,
                                ParamMode::Borrow => {
                                    current.ownership != hir::OwnershipMode::Borrow
                                }
                                ParamMode::Value => current.ownership != hir::OwnershipMode::Value,
                                ParamMode::Shared => true,
                            }
                        } else {
                            ownership(source, &current.ty) != Some(current.ownership)
                        }
                })
        {
            return Err(fail());
        }
        if body_digest::typed(function, &source_revision).is_none() {
            return Ok(None);
        }
    }
    reserve_builder_structure(
        std::mem::size_of::<Inventory>()
            .checked_add(
                program
                    .functions
                    .len()
                    .checked_mul(std::mem::size_of::<FunctionFact>())
                    .ok_or_else(capacity)?,
            )
            .ok_or_else(capacity)?,
    )?;
    let records = records::capture(program, resolved, &mut work)?;
    let mut functions = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        reserve_builder_structure(
            function
                .params
                .len()
                .checked_mul(std::mem::size_of::<ParameterFact>())
                .ok_or_else(capacity)?,
        )?;
        let mut params = Vec::with_capacity(function.params.len());
        body_work = body_work
            .checked_sub(resolved.functions.len())
            .ok_or_else(fail)?;
        let current = resolved
            .functions
            .iter()
            .find(|current| current.id.as_str() == function.stable_id)
            .ok_or_else(fail)?;
        for (param, current) in function.params.iter().zip(&current.params) {
            let resolved = records::clone_type(&current.ty)?;
            params.push(ParameterFact {
                name: crate::bounded_output::budgeted_clone(&param.name),
                ownership: current.ownership,
                ty: resolved,
            });
        }
        reserve_builder_structure(function.stable_id.len())?;
        functions.push(FunctionFact {
            id: hir::DeclarationId::new(function.stable_id.as_str()),
            explicit: function.explicit_id,
            name: crate::bounded_output::budgeted_clone(&function.name),
            params,
            result: records::clone_type(&current.return_type)?,
            body: body_digest::typed(current, &source_revision).ok_or_else(fail)?,
        });
    }
    if crate::bounded_output::active_refused() {
        return Err(capacity());
    }
    let inventory = Inventory {
        functions,
        records,
        source_revision,
    };
    // Validate the complete retained backing arithmetic after the independently
    // reserved exact-capacity carriers have been constructed.
    inventory.owned_capacity_bytes().ok_or_else(capacity)?;
    Ok(Some(Box::new(inventory)))
}
impl Inventory {
    pub(in crate::workspace_graph) fn owned_capacity_bytes(&self) -> Option<usize> {
        self.functions
            .iter()
            .try_fold(
                self.functions
                    .capacity()
                    .checked_mul(std::mem::size_of::<FunctionFact>())?,
                |total, function| {
                    let params = function.params.iter().try_fold(
                        function
                            .params
                            .capacity()
                            .checked_mul(std::mem::size_of::<ParameterFact>())?,
                        |sum, param| {
                            sum.checked_add(param.name.capacity())?
                                .checked_add(match &param.ty {
                                    hir::ResolvedType::Nominal { declaration, .. } => {
                                        declaration.as_str().len()
                                    }
                                    _ => 0,
                                })
                        },
                    )?;
                    total
                        .checked_add(function.id.as_str().len())?
                        .checked_add(function.name.capacity())?
                        .checked_add(params)?
                        .checked_add(match &function.result {
                            hir::ResolvedType::Nominal { declaration, .. } => {
                                declaration.as_str().len()
                            }
                            _ => 0,
                        })
                },
            )?
            .checked_add(
                self.records
                    .as_ref()
                    .map_or(Some(0), |records| records.owned_capacity_bytes())?,
            )
    }
}

pub(in crate::workspace_graph) fn body_matches(
    inventory: &Inventory,
    source: &FunctionFact,
    function: &hir::ResolvedFunction,
) -> bool {
    body_digest::typed(function, &inventory.source_revision) == Some(source.body)
}
pub(in crate::workspace_graph) fn digest_scratch_bytes() -> usize {
    body_digest::scratch_bytes()
}
