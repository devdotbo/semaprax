//! Internal ordinary calls over explicitly imported resource-free byte records.
//! Public Project descriptors and scalar linker signatures remain independent.
use super::diagnostics::PROJECT_SIGNATURE_HELP;
use super::*;
mod text_collections;

pub(super) fn admitted_with_layout(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    layout: &super::expected_projection::private_layout::LayoutClosure<'_>,
) -> bool {
    match text_collections::admitted(caller, target, authored, programs, layout) {
        Ok(true) => return true,
        Err(()) => return false,
        Ok(false) => {}
    }
    let Some(function) = target.function else {
        return false;
    };
    if !function.type_parameters.is_empty() {
        return false;
    }
    let record = |ty: &Type| record_slot(target.module, ty, caller, authored, programs);
    let uses_record = record(&function.return_type)
        || function
            .params
            .iter()
            .any(|parameter| record(&parameter.ty));
    let copy_result = function.params.iter().any(|parameter| {
        (parameter.mode == ParamMode::Own && parameter.ty == Type::Bytes)
            || (matches!(parameter.mode, ParamMode::Own | ParamMode::Borrow)
                && record(&parameter.ty))
    }) && (record_kind(
        target.module,
        &function.return_type,
        caller,
        authored,
        programs,
    ) == Some(false)
        || fieldless_variant_result(
            target.module,
            &function.return_type,
            caller,
            authored,
            programs,
        ));
    uses_record
        && (copy_result
            || scalar(&function.return_type)
            || function.return_type == Type::Bytes
            || record(&function.return_type))
        && function.params.iter().all(|parameter| {
            (parameter.mode == ParamMode::Value && scalar(&parameter.ty))
                || (parameter.mode == ParamMode::Own && parameter.ty == Type::Bytes)
                || (parameter.mode == ParamMode::Borrow
                    && matches!(parameter.ty, Type::Str | Type::SliceU8))
                || (matches!(parameter.mode, ParamMode::Own | ParamMode::Borrow)
                    && record(&parameter.ty))
        })
}
pub(super) fn text_record_import(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    text_collections::record_import(caller, target, authored, programs)
}
/// A closed fieldless variant is a Copy result: it carries only its case tag.
/// Keep the existing direct type-import requirement and exclude payload and
/// resource variants from this owned-record function-import lane.
fn fieldless_variant_result(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    let Some(id) = resolve_type_id(module, name, programs) else {
        return false;
    };
    let Some(target) = authored.get(id.as_str()) else {
        return false;
    };
    let Some(TypeDeclaration {
        kind: TypeDeclarationKind::Variant { cases },
        ..
    }) = target.ty
    else {
        return false;
    };
    !cases.is_empty()
        && cases.iter().all(|case| case.fields.is_empty())
        && signature_type_is_admitted(module, ty, caller, authored, programs, &mut BTreeSet::new())
}
fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}
#[cfg(test)]
thread_local! {
    static LEGACY_RECORD_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
pub(in crate::workspace_graph) fn with_legacy_record_scans<T>(
    operation: impl FnOnce() -> T,
) -> (T, usize) {
    LEGACY_RECORD_SCANS.with(|count| {
        let previous = count.replace(0);
        let result = operation();
        let observed = count.replace(previous);
        (result, observed)
    })
}

fn record_slot(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    record_kind(module, ty, caller, authored, programs) == Some(true)
}
fn record_kind(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Option<bool> {
    #[cfg(test)]
    LEGACY_RECORD_SCANS.with(|count| count.set(count.get().checked_add(1).unwrap()));
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let id = resolve_type_id(module, name, programs)?;
    let target = authored.get(id.as_str())?;
    let declaration = target.ty?;
    let owns_bytes = record_shape(
        target.module,
        declaration,
        authored,
        programs,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    signature_type_is_admitted(module, ty, caller, authored, programs, &mut BTreeSet::new())
        .then_some(owns_bytes)
}
/// Re-derive the complete explicit record closure. No resources, views, generic
/// substitutions, variants, classes, or callable storage enter this lane.
fn record_shape(
    module: &str,
    declaration: &TypeDeclaration,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
    memo: &mut BTreeMap<String, bool>,
) -> Option<bool> {
    if let Some(result) = memo.get(&declaration.stable_id) {
        return Some(*result);
    }
    if visiting.len() >= MAX_CHECKED_VALUE_DEPTH {
        return None;
    }
    if !declaration.explicit_id
        || !declaration.type_parameters.is_empty()
        || !visiting.insert(declaration.stable_id.clone())
    {
        return None;
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    let mut owns_bytes = false;
    for field in fields {
        if field.ty == Type::Bytes {
            owns_bytes = true;
        } else if !scalar(&field.ty) {
            let Type::Named { name, arguments } = &field.ty else {
                return None;
            };
            if !arguments.is_empty() {
                return None;
            }
            let id = resolve_type_id(module, name, programs)?;
            let target = authored.get(id.as_str())?;
            owns_bytes |= record_shape(
                target.module,
                target.ty?,
                authored,
                programs,
                visiting,
                memo,
            )?;
        }
    }
    visiting.remove(&declaration.stable_id);
    memo.insert(declaration.stable_id.clone(), owns_bytes);
    Some(owns_bytes)
}

/// Diagnose the first absent nominal signature identity from authenticated
/// declarations. This is help only: it grants no import or type authority.
fn missing_signature_import(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
) -> Option<String> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if visiting.len() >= MAX_CHECKED_VALUE_DEPTH {
        return None;
    }
    // Diagnostic discovery only: the existing admission route independently
    // authenticates the builtin vector and its declared element layout.
    // A authored nominal shadow or another generic never supplies this hint.
    if name == "Vec"
        && arguments.len() == 1
        && resolve_type_id(module, name, programs).is_none()
        && matches!(&arguments[0], Type::Named { arguments, .. } if arguments.is_empty())
    {
        return missing_signature_import(
            module,
            &arguments[0],
            caller,
            authored,
            programs,
            visiting,
        );
    }
    if !arguments.is_empty() {
        return None;
    }
    let id = resolve_type_id(module, name, programs)?;
    if !visiting.insert(id.clone()) {
        return None;
    }
    let target = authored.get(id.as_str())?;
    let declaration = target.ty?;
    if !target.explicit || !declaration.explicit_id {
        return None;
    }
    if !caller
        .module_uses
        .iter()
        .any(|item| item.kind == ModuleUseKind::Type && item.persistent_id == id)
    {
        return Some(crate::bounded_output::budgeted_format(format_args!(
            "missing direct nominal type import: use type @id(\"{id}\") from {} as {};",
            target.module, declaration.name
        )));
    }
    let mut inspect = |field: &crate::ast::FieldDeclaration| {
        missing_signature_import(
            target.module,
            &field.ty,
            caller,
            authored,
            programs,
            visiting,
        )
    };
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } => fields.iter().find_map(&mut inspect),
        TypeDeclarationKind::Variant { cases } => cases
            .iter()
            .flat_map(|case| &case.fields)
            .find_map(&mut inspect),
        _ => None,
    }
}

pub(super) fn type_refusal(
    caller: &Program,
    module_use: &ModuleUse,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    let diagnostic = use_error(
        caller,
        module_use,
        "type target must be an admitted nongeneric value type or flat generic record template without borrowed or nested storage",
    );
    let declaration = target.ty.expect("type target carries a type");
    let ty = Type::Named {
        name: declaration.name.clone(),
        arguments: Vec::new(),
    };
    let diagnostic = match missing_signature_import(
        target.module,
        &ty,
        caller,
        authored,
        programs,
        &mut BTreeSet::new(),
    ) {
        Some(help) => diagnostic.with_help(help),
        None => diagnostic,
    };
    Err(vec![diagnostic])
}

// Closed internal Reader inspections retain a scalar result; no view or owner
// can escape through this signature. The defining body is verified separately.
fn reader_inspection_signature(function: &Function) -> bool {
    function.type_parameters.is_empty()
        && function.effects.is_empty()
        && function.yields.is_none()
        && function.follows.is_none()
        && matches!(function.params.as_slice(), [param] if param.mode == ParamMode::Borrow && crate::stdin_stream_ops::ast_is_reader(&param.ty))
        && matches!(
            function.return_type,
            Type::I64
                | Type::I32
                | Type::Char
                | Type::U8
                | Type::Usize
                | Type::F32
                | Type::F64
                | Type::Bool
        )
}

pub(super) fn validate_imported_function<'a>(
    caller: &Program,
    module_use: &ModuleUse,
    target: &AuthoredDeclaration<'_>,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    layouts: &mut Option<expected_projection::private_layout::LayoutClosure<'a>>,
) -> Result<(), Vec<Diagnostic>> {
    validate_imported_function_with_registry(
        caller, module_use, target, authored, programs, layouts, None,
    )
}
pub(super) fn validate_imported_function_with_registry<'a>(
    caller: &Program,
    module_use: &ModuleUse,
    target: &AuthoredDeclaration<'_>,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    layouts: &mut Option<expected_projection::private_layout::LayoutClosure<'a>>,
    registry: Option<&expected_projection::private_layout::registry::LayoutRegistry<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let held;
    if registry.is_none() && layouts.is_none() {
        *layouts = Some(expected_projection::private_layout::LayoutClosure::derive(
            caller, authored, programs,
        )?);
    }
    let layout = if let Some(registry) = registry {
        held = registry.get(caller, authored, programs)?;
        &*held
    } else {
        layouts.as_ref().expect("initialized caller layout")
    };
    let function = target.function.expect("function target carries a function");
    let transparent_vec_wrapper = programs
        .iter()
        .find(|program| program.module == target.module)
        .is_some_and(|program| crate::vec_ops::source_wrapper(program, function).is_some());
    let transparent_box_wrapper = programs
        .iter()
        .find(|program| program.module == target.module)
        .is_some_and(|program| crate::box_ops::source_wrapper(program, function).is_some());
    if crate::stdin_stream_ops::ast_forward_signature(function)
        || reader_inspection_signature(function)
        || transparent_vec_wrapper
        || transparent_box_wrapper
        || admitted_with_layout(caller, target, authored, programs, layout)
    {
        return Ok(());
    }
    if expected_projection::byte_writer_import_authorized(
        function,
        target.module,
        authored,
        programs,
    )? {
        return Ok(());
    }
    let byte_parameter = package::admitted_byte_parameter;
    let borrowed_copy_vec_parameter = |param: &crate::ast::Param| {
        param.mode == ParamMode::Borrow && crate::vec_ops::ast_copy_vec(&param.ty)
    };
    let has_byte_parameter = function.params.iter().any(byte_parameter);
    let scalar_return = matches!(
        function.return_type,
        Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
    );
    let refusal = |message: &str, scalar_help: bool| {
        let diagnostic = use_error(caller, module_use, message);
        let mut visiting = BTreeSet::new();
        let missing = function
            .params
            .iter()
            .map(|parameter| &parameter.ty)
            .chain(std::iter::once(&function.return_type))
            .find_map(|ty| {
                missing_signature_import(
                    target.module,
                    ty,
                    caller,
                    authored,
                    programs,
                    &mut visiting,
                )
            });
        if let Some(help) = missing {
            diagnostic.with_help(help)
        } else if scalar_help {
            diagnostic.with_help(PROJECT_SIGNATURE_HELP)
        } else {
            diagnostic
        }
    };
    if !function.type_parameters.is_empty()
        || function.params.iter().any(|param| {
            param.mode != ParamMode::Value
                && !byte_parameter(param)
                && !borrowed_copy_vec_parameter(param)
                && !(param.mode == ParamMode::Borrow && param.ty == Type::Str)
        })
        || (has_byte_parameter && !scalar_return)
    {
        return Err(vec![refusal(package::import_profile_refusal(), false)]);
    }
    for param in &function.params {
        if byte_parameter(param) || borrowed_copy_vec_parameter(param) {
            continue;
        }
        if !signature_type_is_admitted(
            target.module,
            &param.ty,
            caller,
            authored,
            programs,
            &mut BTreeSet::new(),
        ) {
            return Err(vec![refusal(
                "function signature leaves the admitted scalar/Copy workspace domain",
                true,
            )]);
        }
    }
    let ty = &function.return_type;
    {
        if !signature_type_is_admitted(
            target.module,
            ty,
            caller,
            authored,
            programs,
            &mut BTreeSet::new(),
        ) {
            return Err(vec![refusal(
                "function signature leaves the admitted scalar/Copy workspace domain",
                true,
            )]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
