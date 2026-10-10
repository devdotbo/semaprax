//! Proof over actual provider bodies, never over the generated prototype.
use super::*;
use crate::ast::{ModuleUseKind, Program};
use crate::workspace_graph::{AuthoredDeclaration, AuthoredKind};
use std::collections::BTreeMap;

fn provider<'a>(
    function: &'a Function,
    module: &str,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<&'a Program> {
    let fact = authored.get(function.stable_id.as_str())?;
    if !fact.explicit
        || fact.kind != AuthoredKind::Function
        || fact.module != module
        || !std::ptr::eq(fact.function?, function)
    {
        return None;
    }
    let mut owners = programs.iter().filter(|program| program.module == module);
    let owner = owners.next()?;
    if owners.next().is_some()
        || owner
            .functions
            .iter()
            .filter(|candidate| candidate.stable_id == function.stable_id)
            .count()
            != 1
    {
        return None;
    }
    if !owner
        .functions
        .iter()
        .any(|candidate| std::ptr::eq(candidate, function))
    {
        return None;
    }
    Some(owner)
}
fn lookup<'a>(
    owner: &'a Program,
    name: &str,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<(&'a Program, &'a Function)> {
    let mut local = owner
        .functions
        .iter()
        .filter(|function| function.name == name);
    if let Some(function) = local.next() {
        if local.next().is_some() {
            return None;
        }
        return Some((
            provider(function, &owner.module, authored, programs)?,
            function,
        ));
    }
    let mut imports = owner
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function && import.alias == name);
    let import = imports.next()?;
    if imports.next().is_some() {
        return None;
    }
    let fact = authored.get(import.persistent_id.as_str())?;
    if fact.module != import.target_module {
        return None;
    }
    let function = fact.function?;
    Some((
        provider(function, fact.module, authored, programs)?,
        function,
    ))
}
fn record<'a>(
    owner: &Program,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<&'a TypeDeclaration> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let defining = programs
        .iter()
        .find(|program| program.module == owner.module)?;
    let local = defining
        .types
        .iter()
        .find(|declaration| declaration.name == *name);
    let fact = if let Some(declaration) = local {
        authored.get(declaration.stable_id.as_str())?
    } else {
        let mut imports = defining
            .module_uses
            .iter()
            .filter(|import| import.kind == ModuleUseKind::Type && import.alias == *name);
        let import = imports.next()?;
        if imports.next().is_some() {
            return None;
        }
        let fact = authored.get(import.persistent_id.as_str())?;
        if fact.module != import.target_module {
            return None;
        }
        fact
    };
    let declaration = fact.ty?;
    if !fact.explicit
        || fact.kind != AuthoredKind::Type
        || !declaration.type_parameters.is_empty()
        || !matches!(declaration.kind, TypeDeclarationKind::Record { .. })
    {
        return None;
    }
    let mut owners = programs
        .iter()
        .filter(|program| program.module == fact.module);
    let provider = owners.next()?;
    if owners.next().is_some()
        || provider
            .types
            .iter()
            .filter(|item| item.stable_id == declaration.stable_id)
            .count()
            != 1
        || !provider
            .types
            .iter()
            .any(|item| std::ptr::eq(item, declaration))
    {
        return None;
    }
    Some(declaration)
}
fn parameter<'a>(
    owner: &Program,
    mode: ParamMode,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> bool {
    match mode {
        ParamMode::Own => *ty == Type::Bytes,
        ParamMode::Value => {
            crate::source_verify::is_scalar_source_type(ty)
                || crate::loop_calls::ast_copy_variant(owner, ty)
                || record(owner, ty, authored, programs)
                    .is_some_and(crate::source_verify::copy_record_declaration_admitted)
        }
        ParamMode::Borrow => {
            crate::loop_calls::ast_param_admitted(owner, mode, ty)
                || record(owner, ty, authored, programs).is_some()
        }
        ParamMode::Shared => false,
    }
}

pub(super) fn authorized<'a>(
    function: &'a Function,
    module: &str,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> bool {
    if function.return_type != Type::Bytes
        || function
            .params
            .iter()
            .filter(|param| param.mode == ParamMode::Own && param.ty == Type::Bytes)
            .count()
            != 1
    {
        return false;
    }
    let Some(owner) = provider(function, module, authored, programs) else {
        return false;
    };
    let Some(lookup_work) = programs.iter().try_fold(programs.len(), |work, program| {
        work.checked_add(program.functions.len())?
            .checked_add(program.types.len())?
            .checked_add(program.module_uses.len())
    }) else {
        return false;
    };
    crate::loop_calls::byte_writer::source_nonallocating_with(
        owner,
        function,
        |owner, name| lookup(owner, name, authored, programs),
        |owner, mode, ty| parameter(owner, mode, ty, authored, programs),
        |owner, ty| {
            crate::source_verify::is_scalar_source_type(ty)
                || crate::loop_calls::ast_copy_variant(owner, ty)
                || record(owner, ty, authored, programs)
                    .is_some_and(crate::source_verify::copy_record_declaration_admitted)
        },
        lookup_work,
    )
}
