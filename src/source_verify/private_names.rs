//! Presentation-only provenance for invocation-private layout declarations.
//!
//! The Workspace Semantic Graph appends copied provider declarations under
//! minted private names so a function-only signature root can be checked
//! without publishing the provider type in the caller. Those names are not
//! source and must never reach a diagnostic, where an agent could treat them
//! as usable spellings or repairs. The graph binds this scope to the exact
//! synthetic program and the exact copied declaration addresses it appended.
//! Lookup still resolves only by the synthetic name: this scope changes how a
//! diagnostic names a declaration, never which declarations resolve. An
//! authored declaration with any spelling, including a colliding one, is not
//! one of the bound addresses and keeps its ordinary authored rendering.

use crate::ast::{Program, Span, Type, TypeDeclaration};
use crate::diagnostic::Diagnostic;
use crate::source_verify::diagnostics::error;
use crate::source_verify::type_table::TypeTable;
use std::cell::RefCell;

/// The authenticated provider context of one copied private declaration.
pub(crate) struct ProviderName {
    /// Address of the copied declaration inside the bound synthetic program.
    pub(crate) declaration: usize,
    pub(crate) module: String,
    pub(crate) id: String,
    /// The provider's own declared name; context only, never a caller name.
    pub(crate) name: String,
    /// The provider declaration has an explicit persistent identity, so an
    /// ordinary `use type @id(..)` import can name it.
    pub(crate) explicit: bool,
    /// No caller import, type or function already spells `name`, so that
    /// import may use the provider's declared name as its alias.
    pub(crate) name_free: bool,
}

struct Active {
    program: usize,
    names: Vec<ProviderName>,
}

std::thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

/// Run `operation` with `names` bound to the exact `program` it verifies.
pub(crate) fn with_provider_names<T>(
    program: &Program,
    names: Vec<ProviderName>,
    operation: impl FnOnce() -> T,
) -> T {
    if names.is_empty() {
        return operation();
    }
    struct Restore(Option<Active>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            ACTIVE.with(|active| *active.borrow_mut() = previous);
        }
    }
    let restore = Restore(ACTIVE.with(|active| {
        active.borrow_mut().replace(Active {
            program: program as *const Program as usize,
            names,
        })
    }));
    let result = operation();
    drop(restore);
    result
}

fn provider<R>(
    program: &Program,
    types: &TypeTable<'_>,
    name: &str,
    present: impl FnOnce(&ProviderName) -> R,
) -> Option<R> {
    let declaration = types.declaration(name)? as *const TypeDeclaration as usize;
    ACTIVE.with(|active| {
        let active = active.borrow();
        let active = active.as_ref()?;
        if active.program != program as *const Program as usize {
            return None;
        }
        active
            .names
            .iter()
            .find(|candidate| candidate.declaration == declaration)
            .map(present)
    })
}

fn label(provider: &ProviderName) -> String {
    format!(
        "{} [unimported @id(\"{}\") from {}]",
        provider.name, provider.id, provider.module
    )
}

/// `ty` as a diagnostic names it. Outside a bound private scope this is the
/// exact ordinary `Display` rendering.
pub(super) fn type_name(program: &Program, types: &TypeTable<'_>, ty: &Type) -> String {
    let bound = ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_some_and(|active| active.program == program as *const Program as usize)
    });
    if !bound {
        return ty.to_string();
    }
    let mut presented = None::<Type>;
    let mut pending = vec![ty];
    while let Some(node) = pending.pop() {
        match node {
            Type::Named { name, arguments } => {
                if provider(program, types, name, |_| ()).is_some() {
                    presented = Some(ty.clone());
                    break;
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
    let Some(mut presented) = presented else {
        return ty.to_string();
    };
    let mut pending = vec![&mut presented];
    while let Some(node) = pending.pop() {
        match node {
            Type::Named { name, arguments } => {
                if let Some(text) = provider(program, types, name, label) {
                    *name = text;
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
    program: &Program,
    types: &TypeTable<'_>,
    variant_name: &str,
    case_name: &str,
    payload: bool,
    span: Span,
) -> Diagnostic {
    let body = if payload { "{ .. }" } else { "{}" };
    provider(program, types, variant_name, |provider| {
        let diagnostic = error(
            program,
            "SPX-M101",
            format!(
                "non-exhaustive match; missing case `{case_name} {body}` of provider variant `{}` \
                 (@id(\"{}\") from {}), which this module has not imported",
                provider.name, provider.id, provider.module
            ),
            span,
        );
        match (provider.explicit, provider.name_free) {
            (true, true) => diagnostic.with_help(format!(
                "to name its cases, import the provider variant explicitly: \
                 `use type @id(\"{}\") from {} as {};`",
                provider.id, provider.module, provider.name
            )),
            (true, false) => diagnostic.with_help(format!(
                "to name its cases, import @id(\"{}\") from {} explicitly under an alias \
                 this module does not already use",
                provider.id, provider.module
            )),
            (false, _) => diagnostic,
        }
    })
    .unwrap_or_else(|| {
        error(
            program,
            "SPX-M101",
            format!("non-exhaustive match; missing case `{variant_name}::{case_name} {body}`"),
            span,
        )
    })
}
