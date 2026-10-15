//! Presentation-only provenance for invocation-private layout declarations.
//!
//! The Workspace Semantic Graph appends copied provider declarations under
//! minted private names so a function-only signature root can be checked
//! without publishing the provider type in the caller. Those names are not
//! source and must never reach a diagnostic, where an agent could treat them
//! as usable spellings or repairs. The graph lends a [`PrivateNames`] view of
//! its held closure to the one verification of the exact synthetic program
//! those copies were appended to, through that verification's [`TypeTable`].
//! The view borrows everything it presents: binding it allocates nothing, and
//! provider context is read only while a diagnostic is constructed.
//! Lookup still resolves only by the synthetic name: this scope changes how a
//! diagnostic names a declaration, never which declarations resolve. An
//! authored declaration with any spelling, including a colliding one, is not
//! one of the appended copies and keeps its ordinary authored rendering.

use crate::ast::program_source::ProgramSource;
use crate::ast::{Program, Span, Type, TypeDeclaration};
use crate::diagnostic::Diagnostic;
use crate::source_verify::diagnostics::error;
use crate::source_verify::type_table::TypeTable;

/// The held closure that appended the private copies, in append order.
pub(crate) trait Providers {
    /// The provider module and provider declaration of the `index`th copy.
    fn provider(&self, index: usize) -> Option<(&str, &TypeDeclaration)>;
}

/// A borrowed binding of the copies at `program.types()[base..]` to the closure
/// that appended them. Only the graph constructs one, after checking that
/// every copy is the exact appended declaration the closure describes.
#[derive(Clone, Copy)]
pub(crate) struct PrivateNames<'s> {
    program: &'s dyn ProgramSource,
    caller: &'s Program,
    base: usize,
    providers: &'s dyn Providers,
}

impl<'s> PrivateNames<'s> {
    pub(crate) fn new(
        program: &'s dyn ProgramSource,
        caller: &'s Program,
        base: usize,
        providers: &'s dyn Providers,
    ) -> Self {
        Self {
            program,
            caller,
            base,
            providers,
        }
    }
}

struct Provider<'s> {
    module: &'s str,
    declaration: &'s TypeDeclaration,
}

fn provider<'s>(
    program: &dyn ProgramSource,
    types: &TypeTable<'s>,
    name: &str,
) -> Option<(Provider<'s>, &'s Program)> {
    let names = types.private?;
    if !names.program.subject().same(program.subject()) {
        return None;
    }
    let declaration = types.declaration(name)?;
    let index = names
        .program
        .types()
        .suffix(names.base)?
        .iter()
        .position(|copy| std::ptr::eq(copy, declaration))?;
    let (module, declaration) = names.providers.provider(index)?;
    Some((
        Provider {
            module,
            declaration,
        },
        names.caller,
    ))
}

fn label(provider: &Provider<'_>) -> String {
    format!(
        "{} [unimported @id(\"{}\") from {}]",
        provider.declaration.name, provider.declaration.stable_id, provider.module
    )
}

/// Whether `ty` names a private layout copy anywhere in its payload. Such a
/// type has no source spelling, so no help may offer it as one.
pub(super) fn is_private(program: &dyn ProgramSource, types: &TypeTable<'_>, ty: &Type) -> bool {
    if !types
        .private
        .is_some_and(|names| names.program.subject().same(program.subject()))
    {
        return false;
    }
    let mut pending = vec![ty];
    while let Some(node) = pending.pop() {
        match node {
            Type::Named { name, arguments } => {
                if provider(program, types, name).is_some() {
                    return true;
                }
                pending.extend(arguments);
            }
            Type::Function { parameters, result } => {
                pending.extend(parameters);
                pending.push(result);
            }
            _ => {}
        }
    }
    false
}

/// How a diagnostic names the bare declared type `name` when it is a private
/// copy of this exact program; `None` when `name` is its own source spelling.
pub(super) fn private_label(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    name: &str,
) -> Option<String> {
    provider(program, types, name).map(|(provider, _)| label(&provider))
}

/// `ty` as a diagnostic names it. Outside a bound private scope this is the
/// exact ordinary `Display` rendering.
pub(super) fn type_name(program: &dyn ProgramSource, types: &TypeTable<'_>, ty: &Type) -> String {
    if !is_private(program, types, ty) {
        return ty.to_string();
    }
    let mut presented = ty.clone();
    let mut pending = vec![&mut presented];
    while let Some(node) = pending.pop() {
        match node {
            Type::Named { name, arguments } => {
                if let Some((provider, _)) = provider(program, types, name) {
                    *name = label(&provider);
                }
                pending.extend(arguments.iter_mut());
            }
            Type::Function { parameters, result } => {
                pending.extend(parameters.iter_mut());
                pending.push(result);
            }
            _ => {}
        }
    }
    presented.to_string()
}

/// The `SPX-M101` missing-case diagnostic. An authored or explicitly imported
/// variant keeps its copyable `Type::Case` witness; a private layout names
/// only the case, with its provider context and, when the provider supports
/// it, the ordinary explicit import that would make its cases nameable.
pub(super) fn missing_case(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    variant_name: &str,
    case_name: &str,
    payload: bool,
    span: Span,
) -> Diagnostic {
    let body = if payload { "{ .. }" } else { "{}" };
    let Some((provider, caller)) = provider(program, types, variant_name) else {
        return error(
            program,
            "SPX-M101",
            format!("non-exhaustive match; missing case `{variant_name}::{case_name} {body}`"),
            span,
        );
    };
    let declaration = provider.declaration;
    let diagnostic = error(
        program,
        "SPX-M101",
        format!(
            "non-exhaustive match; missing case `{case_name} {body}` of provider variant `{}` \
             (@id(\"{}\") from {}), which this module has not imported",
            declaration.name, declaration.stable_id, provider.module
        ),
        span,
    );
    if !declaration.explicit_id {
        return diagnostic;
    }
    // The provider name is an import alias only when the authored caller has
    // no import, type or function already spelling it.
    let spelled = caller
        .module_uses
        .iter()
        .any(|u| u.alias == declaration.name)
        || caller.types.iter().any(|t| t.name == declaration.name)
        || caller.functions.iter().any(|f| f.name == declaration.name);
    if spelled {
        diagnostic.with_help(format!(
            "to name its cases, import @id(\"{}\") from {} explicitly under an alias \
             this module does not already use",
            declaration.stable_id, provider.module
        ))
    } else {
        diagnostic.with_help(format!(
            "to name its cases, import the provider variant explicitly: \
             `use type @id(\"{}\") from {} as {};`",
            declaration.stable_id, provider.module, declaration.name
        ))
    }
}

#[cfg(test)]
#[path = "private_names/tests.rs"]
mod tests;
